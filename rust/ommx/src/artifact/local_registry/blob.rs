use super::{sha256_digest, ValidatedDigest, FILE_BLOB_STORE_DIR_NAME};
use anyhow::{ensure, Context, Result};
use std::{
    fs,
    fs::OpenOptions,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

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

    pub fn put_bytes(&self, bytes: &[u8]) -> Result<String> {
        let digest = sha256_digest(bytes);
        let path = self.path_for_digest(&digest)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        if path.exists() {
            verify_existing_blob(&path, bytes, &digest)?;
        } else {
            self.write_blob_atomically(bytes, &digest, &path)?;
        }
        Ok(digest)
    }

    pub fn read_bytes(&self, digest: &str) -> Result<Vec<u8>> {
        let path = self.path_for_digest(digest)?;
        let bytes =
            fs::read(&path).with_context(|| format!("Failed to read blob {}", path.display()))?;
        ensure!(
            sha256_digest(&bytes) == digest,
            "Blob digest verification failed for {digest}"
        );
        Ok(bytes)
    }

    pub fn exists(&self, digest: &str) -> Result<bool> {
        Ok(self.path_for_digest(digest)?.exists())
    }

    pub fn path_for_digest(&self, digest: &str) -> Result<PathBuf> {
        let digest = ValidatedDigest::parse(digest)?;
        Ok(self.root.join(digest.algorithm()).join(digest.encoded()))
    }

    fn write_blob_atomically(&self, bytes: &[u8], digest: &str, path: &Path) -> Result<()> {
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
                verify_existing_blob(path, bytes, digest)
            }
            Err(error) => {
                let _ = fs::remove_file(&temp_path);
                Err(error).with_context(|| {
                    format!(
                        "Failed to publish blob {} from {} to {}",
                        digest,
                        temp_path.display(),
                        path.display()
                    )
                })
            }
        }
    }

    fn temp_path_for_digest(&self, digest: &str) -> Result<PathBuf> {
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
