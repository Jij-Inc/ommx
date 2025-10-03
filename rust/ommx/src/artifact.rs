//! Manage messages as container
//!

mod annotations;
mod config;
pub mod media_types;
pub use annotations::*;
pub use config::*;

use anyhow::{bail, Context, Result};
use ocipkg::ImageName;
use std::path::Path;
use std::{env, path::PathBuf, sync::OnceLock};

/// Global storage for the local registry root path
static LOCAL_REGISTRY_ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Set the root directory for OMMX local registry
///
/// See [`get_local_registry_root`] for details.
///
pub fn set_local_registry_root(path: impl Into<PathBuf>) -> Result<()> {
    let path = path.into();
    LOCAL_REGISTRY_ROOT.set(path.clone()).map_err(|path| {
        anyhow::anyhow!(
            "Local registry root has already been set: {}",
            path.display()
        )
    })?;
    log::info!("Local registry root set via API: {}", path.display());
    Ok(())
}

/// Get the root directory for OMMX local registry
///
/// - Once the root is set, it is immutable for the lifetime of the program.
/// - You can set it via [`set_local_registry_root`] function before calling this.
/// - If this is called without calling [`set_local_registry_root`],
///   - It will check the `OMMX_LOCAL_REGISTRY_ROOT` environment variable.
///   - If the environment variable is not set, it will use the default project data directory.
/// - The root directory is **NOT** created automatically by this function.
///
pub fn get_local_registry_root() -> &'static Path {
    LOCAL_REGISTRY_ROOT.get_or_init(|| {
        // Try environment variable first
        let path = if let Ok(custom_dir) = env::var("OMMX_LOCAL_REGISTRY_ROOT") {
            let path = PathBuf::from(custom_dir);
            log::info!(
                "Local registry root initialized from OMMX_LOCAL_REGISTRY_ROOT: {}",
                path.display()
            );
            path
        } else {
            let path = directories::ProjectDirs::from("org", "ommx", "ommx")
                .expect("Failed to get project directories")
                .data_dir()
                .to_path_buf();
            log::info!(
                "Local registry root initialized to default: {}",
                path.display()
            );
            path
        };
        path
    })
}

#[deprecated(note = "Use get_local_registry_root instead")]
pub fn data_dir() -> Result<PathBuf> {
    let path = get_local_registry_root().to_path_buf();
    if !path.exists() {
        std::fs::create_dir_all(&path)
            .with_context(|| format!("Failed to create data directory: {}", path.display()))?;
    }
    Ok(path)
}

/// Get the directory for the given image name in the local registry
///
/// # Deprecated
/// Use [`get_local_registry_path`] instead, which is format-agnostic and works with both oci-dir and oci-archive formats.
#[deprecated(
    since = "2.1.0",
    note = "Use get_local_registry_path instead for dual format support"
)]
pub fn get_image_dir(image_name: &ImageName) -> PathBuf {
    get_local_registry_root().join(image_name.as_path())
}

/// Get the base path for the given image name in the local registry
///
/// This returns the path where the artifact should be stored, without format-specific extensions.
/// The caller should check:
/// - If this path is a directory with oci-layout -> oci-dir format
/// - If "{path}.ommx" exists as a file -> oci-archive format
pub fn get_local_registry_path(image_name: &ImageName) -> PathBuf {
    get_local_registry_root().join(image_name.as_path())
}

#[deprecated(note = "Use get_image_dir instead")]
pub fn image_dir(image_name: &ImageName) -> Result<PathBuf> {
    #[allow(deprecated)]
    Ok(data_dir()?.join(image_name.as_path()))
}

pub fn ghcr(org: &str, repo: &str, name: &str, tag: &str) -> Result<ImageName> {
    ImageName::parse(&format!(
        "ghcr.io/{}/{}/{}:{}",
        org.to_lowercase(),
        repo.to_lowercase(),
        name.to_lowercase(),
        tag
    ))
}

fn gather_oci_dirs(dir: &Path) -> Result<Vec<PathBuf>> {
    gather_artifacts(dir)
}

fn gather_artifacts(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut artifacts = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if path.join("oci-layout").exists() {
                // OCI directory format
                artifacts.push(path);
            } else {
                let mut sub_artifacts = gather_artifacts(&path)?;
                artifacts.append(&mut sub_artifacts)
            }
        } else if path.is_file() {
            // Check if it's an OCI archive by trying to parse it
            // OCI archives typically have .ommx extension or can be detected by content
            if is_oci_archive(&path) {
                artifacts.push(path);
            }
        }
    }
    Ok(artifacts)
}

fn is_oci_archive(path: &Path) -> bool {
    // Check file extension first (most common case)
    if let Some(ext) = path.extension() {
        if ext == "ommx" || ext == "tar" {
            return true;
        }
    }

    // For files without recognized extensions, don't try to parse them
    // as this could be expensive and most files won't be OCI archives
    false
}

fn auth_from_env() -> Result<(String, String, String)> {
    if let (Ok(domain), Ok(username), Ok(password)) = (
        env::var("OMMX_BASIC_AUTH_DOMAIN"),
        env::var("OMMX_BASIC_AUTH_USERNAME"),
        env::var("OMMX_BASIC_AUTH_PASSWORD"),
    ) {
        log::info!(
            "Detect OMMX_BASIC_AUTH_DOMAIN, OMMX_BASIC_AUTH_USERNAME, OMMX_BASIC_AUTH_PASSWORD for authentication."
        );
        return Ok((domain, username, password));
    }
    bail!("No authentication information found in environment variables");
}

/// Get all images stored in the local registry
pub fn get_images() -> Result<Vec<ImageName>> {
    use crate::experimental::artifact::Artifact;

    let root = get_local_registry_root();
    let artifacts = gather_oci_dirs(root)?;
    artifacts
        .into_iter()
        .map(|artifact_path| {
            let relative = artifact_path
                .strip_prefix(root)
                .context("Failed to get relative path")?;

            // For archive files, we need to extract the image name differently
            if artifact_path.is_file() {
                // Try to extract image name from the archive metadata
                if let Ok(mut artifact) = Artifact::from_oci_archive(&artifact_path) {
                    if let Some(name) = artifact.image_name() {
                        return ImageName::parse(&name);
                    }
                }
                // Fallback: use the file path without extension as image name
                let path_without_ext = if let Some(stem) = relative.file_stem() {
                    relative.with_file_name(stem)
                } else {
                    relative.to_path_buf()
                };
                ImageName::from_path(&path_without_ext)
            } else {
                // Directory format - use path as before
                ImageName::from_path(relative)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_is_oci_archive_extension() {
        // Create temporary files to test extension-based detection
        let temp_dir = std::env::temp_dir().join(format!("ommx_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();
        let temp_ommx = temp_dir.join("test.ommx");
        let temp_txt = temp_dir.join("test.txt");

        fs::write(&temp_ommx, b"").unwrap();
        fs::write(&temp_txt, b"").unwrap();

        // Should return true for .ommx extension even if content is invalid
        assert!(is_oci_archive(&temp_ommx));
        // Should return false for .txt extension
        assert!(!is_oci_archive(&temp_txt));

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_gather_artifacts_with_mock_oci_dir() {
        let temp_dir = std::env::temp_dir().join(format!("ommx_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        // Create a mock OCI directory structure
        let oci_dir = temp_dir.join("test-image");
        fs::create_dir_all(&oci_dir).unwrap();
        fs::write(
            oci_dir.join("oci-layout"),
            r#"{"imageLayoutVersion": "1.0.0"}"#,
        )
        .unwrap();

        let result = gather_artifacts(&temp_dir).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], oci_dir);

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_gather_artifacts_with_mock_oci_archive() {
        let temp_dir = std::env::temp_dir().join(format!("ommx_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        // Create a mock OCI archive file (just an empty .ommx file)
        let archive_file = temp_dir.join("test-artifact.ommx");
        fs::write(&archive_file, b"").unwrap();

        let result = gather_artifacts(&temp_dir).unwrap();
        // Should find the .ommx file, even though it's not a valid OCI archive
        // because it has the right extension
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], archive_file);

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_gather_artifacts_mixed_formats() {
        let temp_dir = std::env::temp_dir().join(format!("ommx_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        // Create both OCI directory and archive
        let oci_dir = temp_dir.join("test-dir");
        fs::create_dir_all(&oci_dir).unwrap();
        fs::write(
            oci_dir.join("oci-layout"),
            r#"{"imageLayoutVersion": "1.0.0"}"#,
        )
        .unwrap();

        let archive_file = temp_dir.join("test-archive.ommx");
        fs::write(&archive_file, b"").unwrap();

        let result = gather_artifacts(&temp_dir).unwrap();
        assert_eq!(result.len(), 2);

        // Should find both formats
        assert!(result.contains(&oci_dir));
        assert!(result.contains(&archive_file));

        fs::remove_dir_all(&temp_dir).unwrap();
    }
}
