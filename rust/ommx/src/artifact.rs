//! Manage messages as container
//!
//! This module provides a unified Artifact API that dynamically manages different storage formats:
//! - OCI Archive format (`.ommx` files, default for new artifacts)
//! - OCI Directory format (legacy support)
//! - Remote registry references

mod annotations;
mod builder;
mod config;
mod io;
mod layers;
pub mod media_types;
#[cfg(test)]
mod tests;

pub use annotations::*;
pub use builder::Builder;
pub use config::*;

use anyhow::{bail, ensure, Context, Result};
use ocipkg::{
    image::{Image, OciArchive, OciArtifact, OciDir, Remote},
    oci_spec::image::Descriptor,
    ImageName,
};
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

/// OMMX Artifact with dynamic format handling
///
/// This enum replaces the parametric `Artifact<T: Image>` with a simpler API that
/// automatically manages different storage formats.
///
/// # Variants
///
/// - `Archive`: OCI archive format (`.ommx` file, default for new artifacts)
/// - `Dir`: OCI directory format (legacy support)
/// - `Remote`: Remote registry reference (transitions to Archive/Dir after pull)
pub enum Artifact {
    Archive(OciArtifact<OciArchive>),
    Dir(OciArtifact<OciDir>),
    Remote(OciArtifact<Remote>),
}

impl Artifact {
    /// Create an Artifact from an OCI archive file (`.ommx`)
    pub fn from_oci_archive(path: &Path) -> Result<Self> {
        let mut artifact = OciArtifact::from_oci_archive(path)?;
        Self::validate_artifact_type(&mut artifact)?;
        Ok(Self::Archive(artifact))
    }

    /// Create an Artifact from an OCI directory
    pub fn from_oci_dir(path: &Path) -> Result<Self> {
        let mut artifact = OciArtifact::from_oci_dir(path)?;
        Self::validate_artifact_type(&mut artifact)?;
        Ok(Self::Dir(artifact))
    }

    /// Create an Artifact from a remote registry
    pub fn from_remote(image_name: ImageName) -> Result<Self> {
        let artifact = OciArtifact::from_remote(image_name)?;
        Ok(Self::Remote(artifact))
    }

    /// Get the image name if available
    pub fn image_name(&mut self) -> Option<String> {
        match self {
            Self::Archive(a) => a.get_name().ok().map(|n| n.to_string()),
            Self::Dir(a) => a.get_name().ok().map(|n| n.to_string()),
            Self::Remote(a) => a.get_name().ok().map(|n| n.to_string()),
        }
    }

    /// Get manifest annotations
    pub fn annotations(&mut self) -> Result<std::collections::HashMap<String, String>> {
        let manifest = self.get_manifest()?;
        Ok(manifest.annotations().clone().unwrap_or_default())
    }

    /// Get layer descriptors
    pub fn layers(&mut self) -> Result<Vec<Descriptor>> {
        let manifest = self.get_manifest()?;
        Ok(manifest.layers().to_vec())
    }

    /// Validate that the artifact has the correct OMMX artifact type
    pub(crate) fn validate_artifact_type<T: Image>(artifact: &mut OciArtifact<T>) -> Result<()> {
        let manifest = artifact.get_manifest()?;
        let ty = manifest
            .artifact_type()
            .as_ref()
            .context("Not an OMMX Artifact")?;
        ensure!(
            *ty == media_types::v1_artifact(),
            "Not an OMMX Artifact: {}",
            ty
        );
        Ok(())
    }
}

/// Get all images stored in the local registry
pub fn get_images() -> Result<Vec<ImageName>> {
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
