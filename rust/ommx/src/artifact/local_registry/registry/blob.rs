use crate::artifact::digest::{sha256_digest, validate_digest};
use anyhow::{ensure, Context, Result};
use filetime::FileTime;
use oci_spec::image::Digest;
use sha2::{Digest as _, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Read, Write},
    path::{Path, PathBuf},
    str::FromStr,
    time::SystemTime,
};

const BLOB_STORE_LOCK_FILE_NAME: &str = ".lock";
const SHA256_ALGORITHM: &str = "sha256";

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
                verify_existing_blob(&path, &digest, bytes.len() as u64)?;
                touch_existing_blob(&path, digest.as_ref())?;
                return Ok(digest);
            }
        }

        let parent = path.parent().context("Blob path has no parent directory")?;
        let mut temp_blob = create_temp_blob(parent)?;
        temp_blob
            .file
            .write_all(bytes)
            .context("Failed to write temporary blob")?;
        temp_blob
            .file
            .as_file_mut()
            .sync_all()
            .context("Failed to sync temporary blob")?;
        let _lock = self.lock_store()?;
        self.publish_temp_blob(temp_blob, &digest, bytes.len() as u64, &path)?;
        Ok(digest)
    }

    pub fn put_reader(&self, mut reader: impl Read) -> Result<(Digest, u64)> {
        let algorithm_dir = self.root.join(SHA256_ALGORITHM);
        fs::create_dir_all(&algorithm_dir)
            .with_context(|| format!("Failed to create {}", algorithm_dir.display()))?;
        let mut temp_blob = create_temp_blob(&algorithm_dir)?;
        let mut hasher = Sha256::new();
        let mut size = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = match reader.read(&mut buffer) {
                Ok(read) => read,
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) => return Err(error).context("Failed to read blob input"),
            };
            if read == 0 {
                break;
            }
            temp_blob
                .file
                .write_all(&buffer[..read])
                .context("Failed to write temporary blob")?;
            hasher.update(&buffer[..read]);
            size = size
                .checked_add(read as u64)
                .context("Blob size exceeds u64")?;
        }
        temp_blob
            .file
            .as_file_mut()
            .sync_all()
            .context("Failed to sync temporary blob")?;

        let digest = Digest::from_str(&format!(
            "{SHA256_ALGORITHM}:{}",
            encode_hex(&hasher.finalize())
        ))
        .context("Failed to parse digest")?;
        let path = self.path_for_digest(&digest)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        let _lock = self.lock_store()?;
        self.publish_temp_blob(temp_blob, &digest, size, &path)?;
        Ok((digest, size))
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

    fn publish_temp_blob(
        &self,
        temp_blob: TempBlob,
        digest: &Digest,
        size: u64,
        path: &Path,
    ) -> Result<()> {
        if path.exists() {
            verify_existing_blob(path, digest, size)?;
            touch_existing_blob(path, digest.as_ref())?;
            return Ok(());
        }
        let TempBlob {
            file,
            #[cfg(unix)]
            publish_permissions,
        } = temp_blob;
        match file.persist_noclobber(path) {
            Ok(published_file) => {
                #[cfg(unix)]
                if let Err(error) = published_file
                    .set_permissions(publish_permissions)
                    .with_context(|| {
                        format!("Failed to set final blob permissions on {}", path.display())
                    })
                {
                    use std::os::unix::fs::PermissionsExt;

                    let _ = published_file.set_permissions(fs::Permissions::from_mode(0o600));
                    drop(published_file);
                    let _ = fs::remove_file(path);
                    return Err(error);
                }
                drop(published_file);
                touch_existing_blob(path, digest.as_ref())
            }
            Err(error) if error.error.kind() == ErrorKind::AlreadyExists => {
                verify_existing_blob(path, digest, size)?;
                touch_existing_blob(path, digest.as_ref())
            }
            Err(error) => {
                let source_path = error.file.path().to_path_buf();
                Err(error.error).with_context(|| {
                    format!(
                        "Failed to publish blob {} from {} to {}",
                        digest.as_ref(),
                        source_path.display(),
                        path.display()
                    )
                })
            }
        }
    }

    fn lock_store(&self) -> Result<FileBlobStoreLock> {
        let lock_path = self.root.join(BLOB_STORE_LOCK_FILE_NAME);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
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

fn verify_existing_blob(path: &Path, digest: &Digest, expected_size: u64) -> Result<()> {
    let mut file = File::open(path)
        .with_context(|| format!("Failed to open existing blob {}", path.display()))?;
    let actual_size = file
        .metadata()
        .with_context(|| format!("Failed to inspect existing blob {}", path.display()))?
        .len();
    ensure!(
        actual_size == expected_size,
        "Existing blob has wrong size for {digest}: expected={expected_size}, actual={actual_size}"
    );
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("Failed to hash existing blob {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual = format!("sha256:{}", encode_hex(&hasher.finalize()));
    ensure!(
        actual == digest.as_ref(),
        "Existing blob digest mismatch for {digest}"
    );
    Ok(())
}

struct TempBlob {
    file: tempfile::NamedTempFile,
    #[cfg(unix)]
    publish_permissions: fs::Permissions,
}

fn create_temp_blob(directory: &Path) -> Result<TempBlob> {
    let file = tempfile::Builder::new()
        .prefix(".blob-")
        .suffix(".tmp")
        .tempfile_in(directory)
        .with_context(|| format!("Failed to create temporary blob in {}", directory.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permission_probe = tempfile::Builder::new()
            .prefix(".blob-permission-")
            .permissions(fs::Permissions::from_mode(0o666))
            .tempfile_in(directory)
            .with_context(|| {
                format!(
                    "Failed to create blob permission probe in {}",
                    directory.display()
                )
            })?;
        let publish_permissions = permission_probe
            .as_file()
            .metadata()
            .context("Failed to inspect blob permission probe")?
            .permissions();
        drop(permission_probe);
        Ok(TempBlob {
            file,
            publish_permissions,
        })
    }
    #[cfg(not(unix))]
    Ok(TempBlob { file })
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn touch_existing_blob(path: &Path, digest: &str) -> Result<()> {
    filetime::set_file_mtime(path, FileTime::now())
        .with_context(|| format!("Failed to touch existing blob {digest}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Error};
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    #[cfg(unix)]
    use std::{sync::mpsc, thread, time::Duration};

    #[cfg(unix)]
    #[test]
    fn put_bytes_reuses_existing_blob_without_creating_temp_file() {
        let root = tempfile::tempdir().unwrap();
        let store = FileBlobStore::new(root.path()).unwrap();
        let payload = b"already stored";
        let expected = store.put_bytes(payload).unwrap();

        let original_permissions = fs::metadata(root.path()).unwrap().permissions();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o555)).unwrap();
        let result = store.put_bytes(payload);
        fs::set_permissions(root.path(), original_permissions).unwrap();

        assert_eq!(result.unwrap(), expected);
    }

    #[cfg(unix)]
    #[test]
    fn put_reader_preserves_blob_permissions_and_uses_algorithm_directory() {
        let root = tempfile::tempdir().unwrap();
        let store = FileBlobStore::new(root.path()).unwrap();
        store
            .put_bytes(b"create lock and algorithm directory")
            .unwrap();

        let algorithm_dir = root.path().join(SHA256_ALGORITHM);
        let reference_path = algorithm_dir.join("reference-permissions");
        drop(
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&reference_path)
                .unwrap(),
        );
        let expected_mode = fs::metadata(&reference_path).unwrap().permissions().mode() & 0o777;
        fs::remove_file(&reference_path).unwrap();

        let original_permissions = fs::metadata(root.path()).unwrap().permissions();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o555)).unwrap();
        let payload = b"reader-backed payload";
        let result = store.put_reader(Cursor::new(payload));
        fs::set_permissions(root.path(), original_permissions).unwrap();

        let (digest, size) = result.unwrap();
        assert_eq!(size, payload.len() as u64);
        let path = store.path_for_digest(&digest).unwrap();
        let actual_mode = fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(actual_mode, expected_mode);
        assert_eq!(store.read_bytes(&digest).unwrap(), payload);
    }

    #[cfg(unix)]
    struct FailingAfterBlockReader {
        first_chunk: Option<&'static [u8]>,
        blocked: mpsc::Sender<()>,
        resume: mpsc::Receiver<()>,
    }

    #[cfg(unix)]
    impl Read for FailingAfterBlockReader {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            if let Some(bytes) = self.first_chunk.take() {
                buffer[..bytes.len()].copy_from_slice(bytes);
                return Ok(bytes.len());
            }
            self.blocked.send(()).unwrap();
            self.resume.recv().unwrap();
            Err(Error::other("reader failed after blocking"))
        }
    }

    #[cfg(unix)]
    #[test]
    fn partial_reader_temp_blob_stays_private_and_is_removed_on_error() {
        let root = tempfile::tempdir().unwrap();
        let store = FileBlobStore::new(root.path()).unwrap();
        let (blocked_tx, blocked_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let reader = FailingAfterBlockReader {
            first_chunk: Some(b"private partial payload"),
            blocked: blocked_tx,
            resume: resume_rx,
        };
        let writer_store = store.clone();
        let writer = thread::spawn(move || writer_store.put_reader(reader));

        blocked_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("reader did not block after writing its first chunk");
        let algorithm_dir = root.path().join(SHA256_ALGORITHM);
        let temp_paths = fs::read_dir(&algorithm_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(".blob-") && name.ends_with(".tmp"))
            })
            .collect::<Vec<_>>();
        assert_eq!(temp_paths.len(), 1);
        let mode = fs::metadata(&temp_paths[0]).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        resume_tx.send(()).unwrap();
        let error = writer
            .join()
            .unwrap()
            .expect_err("reader error must abort the blob write");
        assert!(error.to_string().contains("Failed to read blob input"));
        assert!(fs::read_dir(algorithm_dir).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with(".blob-") && name.ends_with(".tmp"))));
    }

    struct InterruptedOnce<R> {
        inner: R,
        interrupted: bool,
    }

    impl<R: Read> Read for InterruptedOnce<R> {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            if !self.interrupted {
                self.interrupted = true;
                return Err(Error::from(ErrorKind::Interrupted));
            }
            self.inner.read(buffer)
        }
    }

    #[test]
    fn put_reader_retries_interrupted_reads() {
        let root = tempfile::tempdir().unwrap();
        let store = FileBlobStore::new(root.path()).unwrap();
        let payload = b"payload after an interrupted read";
        let reader = InterruptedOnce {
            inner: Cursor::new(payload),
            interrupted: false,
        };

        let (digest, size) = store.put_reader(reader).unwrap();

        assert_eq!(size, payload.len() as u64);
        assert_eq!(store.read_bytes(&digest).unwrap(), payload);
    }

    #[cfg(windows)]
    #[test]
    fn published_reader_blob_is_not_marked_temporary() {
        use std::os::windows::fs::MetadataExt;

        const FILE_ATTRIBUTE_TEMPORARY: u32 = 0x100;

        let temp = tempfile::tempdir().unwrap();
        let store = FileBlobStore::new(temp.path()).unwrap();
        let (digest, _) = store.put_reader(Cursor::new(b"windows payload")).unwrap();
        let path = store.path_for_digest(&digest).unwrap();
        let attributes = fs::metadata(path).unwrap().file_attributes();

        assert_eq!(attributes & FILE_ATTRIBUTE_TEMPORARY, 0);
    }
}
