use super::{sha256_digest, validate_digest, FILE_BLOB_STORE_DIR_NAME};
use anyhow::{ensure, Context, Result};
use filetime::FileTime;
use oci_spec::image::Digest;
use std::{
    fs,
    fs::OpenOptions,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    str::FromStr,
    time::SystemTime,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct BlobRecord {
    pub digest: Digest,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

#[derive(Debug, Clone)]
pub struct FileBlobStore {
    root: PathBuf,
}

impl FileBlobStore {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root)
            .with_context(|| format!("Failed to create blob store {}", root.display()))?;
        Ok(Self { root })
    }

    pub fn open_in_registry_root(root: impl AsRef<Path>) -> Result<Self> {
        Self::open(root.as_ref().join(FILE_BLOB_STORE_DIR_NAME))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn put_bytes(&self, bytes: &[u8]) -> Result<Digest> {
        let digest = Digest::from_str(&sha256_digest(bytes)).context("Failed to parse digest")?;
        let path = self.path_for_digest(&digest)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        if path.exists() {
            verify_existing_blob(&path, bytes, digest.as_ref())?;
            touch_existing_blob(&path, digest.as_ref())?;
        } else {
            self.write_blob_atomically(bytes, &digest, &path)?;
        }
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

    pub fn blob_record(&self, digest: &Digest) -> Result<Option<BlobRecord>> {
        let path = self.path_for_digest(digest)?;
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Failed to read blob metadata {}", path.display()));
            }
        };
        Ok(Some(BlobRecord {
            digest: digest.clone(),
            size: metadata.len(),
            modified: metadata.modified().ok(),
        }))
    }

    pub(crate) fn touch_blob(&self, digest: &Digest) -> Result<()> {
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

    pub fn delete_blob(&self, digest: &Digest) -> Result<bool> {
        let path = self.path_for_digest(digest)?;
        match fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
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

    fn write_blob_atomically(&self, bytes: &[u8], digest: &Digest, path: &Path) -> Result<()> {
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

        match fs::hard_link(&temp_path, path) {
            Ok(()) => {
                let _ = fs::remove_file(&temp_path);
                Ok(())
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                let _ = fs::remove_file(&temp_path);
                verify_existing_blob(path, bytes, digest.as_ref())
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
