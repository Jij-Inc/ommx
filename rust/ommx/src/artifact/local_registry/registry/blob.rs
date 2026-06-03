use crate::artifact::digest::{sha256_digest, validate_digest};
use anyhow::{ensure, Context, Result};
use filetime::FileTime;
use oci_spec::image::Digest;
use std::{
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    str::FromStr,
    time::SystemTime,
};
use uuid::Uuid;

const BLOB_STORE_LOCK_FILE_NAME: &str = ".lock";

#[derive(Debug, Clone)]
pub struct BlobRecord {
    pub digest: Digest,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

#[derive(Debug, Clone)]
pub enum DeleteBlobOutcome {
    Deleted(BlobRecord),
    Kept(BlobRecord),
    Missing,
}

#[derive(Debug, Clone)]
pub struct FileBlobStore {
    root: PathBuf,
}

impl FileBlobStore {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root)
            .with_context(|| format!("Failed to create blob store {}", root.display()))?;
        Ok(Self { root })
    }

    pub fn put_bytes(&self, bytes: &[u8]) -> Result<Digest> {
        let digest = Digest::from_str(&sha256_digest(bytes)).context("Failed to parse digest")?;
        let path = self.path_for_digest(&digest)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        if path.exists() {
            let _lock = self.lock_store()?;
            if path.exists() {
                verify_existing_blob(&path, bytes, digest.as_ref())?;
                touch_existing_blob(&path, digest.as_ref())?;
                return Ok(digest);
            }
        }

        let temp_path = self.write_temp_blob(bytes, &digest)?;
        let _lock = self.lock_store()?;
        self.publish_temp_blob(&temp_path, bytes, &digest, &path)?;
        Ok(digest)
    }

    pub fn read_bytes(&self, digest: &Digest) -> Result<Vec<u8>> {
        let path = self.path_for_digest(digest)?;
        let bytes =
            fs::read(&path).with_context(|| format!("Failed to read blob {}", path.display()))?;
        ensure!(
            sha256_digest(&bytes) == digest.as_ref(),
            "Blob digest verification failed for {digest}"
        );
        Ok(bytes)
    }

    pub fn exists(&self, digest: &Digest) -> Result<bool> {
        Ok(self.path_for_digest(digest)?.exists())
    }

    pub fn size(&self, digest: &Digest) -> Result<u64> {
        let path = self.path_for_digest(digest)?;
        Ok(fs::metadata(&path)
            .with_context(|| format!("Failed to read blob metadata {}", path.display()))?
            .len())
    }

    pub fn touch_blob(&self, digest: &Digest) -> Result<()> {
        let _lock = self.lock_store()?;
        let path = self.path_for_digest(digest)?;
        let metadata = fs::metadata(&path)
            .with_context(|| format!("Failed to read blob metadata {}", path.display()))?;
        ensure!(
            metadata.is_file(),
            "Blob path is not a file for {digest}: {}",
            path.display()
        );
        touch_existing_blob(&path, digest.as_ref())
    }

    pub fn list_blobs(&self) -> Result<Vec<BlobRecord>> {
        let mut out = Vec::new();
        if !self.root.exists() {
            return Ok(out);
        }
        for algorithm_entry in fs::read_dir(&self.root)
            .with_context(|| format!("Failed to list blob store {}", self.root.display()))?
        {
            let algorithm_entry = algorithm_entry?;
            let algorithm_path = algorithm_entry.path();
            if !algorithm_path.is_dir() {
                continue;
            }
            let Some(algorithm) = algorithm_path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            for blob_entry in fs::read_dir(&algorithm_path).with_context(|| {
                format!("Failed to list blob directory {}", algorithm_path.display())
            })? {
                let blob_entry = blob_entry?;
                let blob_path = blob_entry.path();
                let metadata = match blob_entry.metadata() {
                    Ok(metadata) => metadata,
                    Err(error) if error.kind() == ErrorKind::NotFound => continue,
                    Err(error) => {
                        return Err(error).with_context(|| {
                            format!("Failed to read blob metadata {}", blob_path.display())
                        });
                    }
                };
                if !metadata.is_file() {
                    continue;
                }
                let Some(encoded) = blob_path.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };
                let digest_string = format!("{algorithm}:{encoded}");
                if validate_digest(&digest_string).is_err() {
                    continue;
                }
                let digest = Digest::from_str(&digest_string)
                    .context("Failed to parse listed blob digest")?;
                out.push(BlobRecord {
                    digest,
                    size: metadata.len(),
                    modified: metadata.modified().ok(),
                });
            }
        }
        out.sort_by(|left, right| left.digest.as_ref().cmp(right.digest.as_ref()));
        Ok(out)
    }

    pub fn delete_blob_if_older_than(
        &self,
        digest: &Digest,
        cutoff: SystemTime,
    ) -> Result<DeleteBlobOutcome> {
        let _lock = self.lock_store()?;
        let path = self.path_for_digest(digest)?;
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Ok(DeleteBlobOutcome::Missing);
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Failed to read blob metadata {}", path.display()));
            }
        };
        ensure!(
            metadata.is_file(),
            "Blob path is not a file for {digest}: {}",
            path.display()
        );
        let record = BlobRecord {
            digest: digest.clone(),
            size: metadata.len(),
            modified: metadata.modified().ok(),
        };
        if !record.is_older_than(cutoff) {
            return Ok(DeleteBlobOutcome::Kept(record));
        }
        match fs::remove_file(&path) {
            Ok(()) => Ok(DeleteBlobOutcome::Deleted(record)),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(DeleteBlobOutcome::Missing),
            Err(error) => {
                Err(error).with_context(|| format!("Failed to delete blob {}", path.display()))
            }
        }
    }

    fn path_for_digest(&self, digest: &Digest) -> Result<PathBuf> {
        Ok(self
            .root
            .join(digest.algorithm().as_ref())
            .join(digest.digest()))
    }

    fn write_temp_blob(&self, bytes: &[u8], digest: &Digest) -> Result<PathBuf> {
        let temp_path = self.temp_path_for_digest(digest)?;
        let mut temp_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .with_context(|| format!("Failed to create temporary blob {}", temp_path.display()))?;
        temp_file
            .write_all(bytes)
            .with_context(|| format!("Failed to write temporary blob {}", temp_path.display()))?;
        temp_file
            .sync_all()
            .with_context(|| format!("Failed to sync temporary blob {}", temp_path.display()))?;
        drop(temp_file);
        Ok(temp_path)
    }

    fn publish_temp_blob(
        &self,
        temp_path: &Path,
        bytes: &[u8],
        digest: &Digest,
        path: &Path,
    ) -> Result<()> {
        match fs::hard_link(&temp_path, path) {
            Ok(()) => {
                let result = touch_existing_blob(path, digest.as_ref());
                let _ = fs::remove_file(&temp_path);
                result
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                let result = verify_existing_blob(path, bytes, digest.as_ref())
                    .and_then(|()| touch_existing_blob(path, digest.as_ref()));
                let _ = fs::remove_file(&temp_path);
                result
            }
            Err(error) => {
                let _ = fs::remove_file(&temp_path);
                Err(error).with_context(|| {
                    format!(
                        "Failed to publish blob {} from {} to {}",
                        digest.as_ref(),
                        temp_path.display(),
                        path.display()
                    )
                })
            }
        }
    }

    fn temp_path_for_digest(&self, digest: &Digest) -> Result<PathBuf> {
        let path = self.path_for_digest(digest)?;
        let encoded = path
            .file_name()
            .and_then(|name| name.to_str())
            .context("Blob digest path has no file name")?;
        Ok(path.with_file_name(format!(".{encoded}.{}.tmp", Uuid::new_v4())))
    }

    fn lock_store(&self) -> Result<FileBlobStoreLock> {
        let lock_path = self.root.join(BLOB_STORE_LOCK_FILE_NAME);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&lock_path)
            .with_context(|| format!("Failed to open blob store lock {}", lock_path.display()))?;
        file.lock()
            .with_context(|| format!("Failed to lock blob store {}", lock_path.display()))?;
        Ok(FileBlobStoreLock { file })
    }
}

impl BlobRecord {
    fn is_older_than(&self, cutoff: SystemTime) -> bool {
        self.modified.is_some_and(|modified| modified < cutoff)
    }
}

struct FileBlobStoreLock {
    file: File,
}

impl Drop for FileBlobStoreLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

fn verify_existing_blob(path: &Path, bytes: &[u8], digest: &str) -> Result<()> {
    let existing = fs::read(path)
        .with_context(|| format!("Failed to read existing blob {}", path.display()))?;
    ensure!(
        existing == bytes,
        "Existing blob has different bytes for digest {digest}"
    );
    Ok(())
}

fn touch_existing_blob(path: &Path, digest: &str) -> Result<()> {
    filetime::set_file_mtime(path, FileTime::now())
        .with_context(|| format!("Failed to touch existing blob {digest}"))
}
