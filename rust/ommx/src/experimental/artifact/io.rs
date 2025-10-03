//! I/O operations for Artifact (load, save)

use super::Artifact;
use crate::artifact::get_local_registry_path;
use anyhow::{bail, Context, Result};
use ocipkg::{
    image::{OciArchive, OciArchiveBuilder, OciDir, Remote},
    ImageName,
};
use std::{ops::DerefMut, path::Path};

impl Artifact {
    /// Load an artifact from local registry, pulling from remote if not found
    ///
    /// This method searches the local registry for the artifact in the following order:
    /// 1. Check for `.ommx` archive file
    /// 2. Check for OCI directory format
    /// 3. If neither exists, pull from remote and save as archive (default format)
    ///
    /// # Arguments
    ///
    /// * `image_name` - The image name to load
    ///
    /// # Returns
    ///
    /// Returns the loaded artifact, prioritizing archive format when both exist.
    pub fn load(image_name: &ImageName) -> Result<Self> {
        let base_path = get_local_registry_path(image_name);
        let archive_path = base_path.with_extension("ommx");
        let dir_path = &base_path;

        // Check for archive format first (preferred)
        if archive_path.exists() {
            log::debug!("Loading artifact from archive: {}", archive_path.display());
            match Self::from_oci_archive(&archive_path) {
                Ok(artifact) => return Ok(artifact),
                Err(e) => {
                    log::warn!(
                        "Failed to load archive at {}, trying directory format: {}",
                        archive_path.display(),
                        e
                    );
                }
            }
        }

        // Check for directory format (legacy support)
        if dir_path.exists() && dir_path.is_dir() {
            log::debug!("Loading artifact from directory: {}", dir_path.display());
            return Self::from_oci_dir(dir_path);
        }

        // Neither format exists locally, pull from remote
        log::info!("Artifact not found locally, pulling from remote: {image_name}");
        let mut remote_artifact = Self::from_remote(image_name.clone())?;

        // Save to local registry as archive (default format)
        remote_artifact.save_as_archive(&archive_path)?;

        // Load the saved archive
        Self::from_oci_archive(&archive_path)
    }

    /// Save the artifact as an OCI archive file
    ///
    /// This method converts the artifact to archive format and saves it to the specified path.
    ///
    /// # Arguments
    ///
    /// * `path` - The path where the archive should be saved (typically with `.ommx` extension)
    pub fn save_as_archive(&mut self, path: &Path) -> Result<()> {
        if path.exists() {
            bail!("Output file already exists: {}", path.display());
        }

        let image_name = self
            .image_name()
            .context("Cannot save artifact without image name")?;
        let image_name = ImageName::parse(&image_name)?;

        let builder = OciArchiveBuilder::new(path.to_path_buf(), image_name)?;

        match self {
            Self::Archive(a) => {
                ocipkg::image::copy(a.deref_mut() as &mut OciArchive, builder)?;
            }
            Self::Dir(a) => {
                ocipkg::image::copy(a.deref_mut() as &mut OciDir, builder)?;
            }
            Self::Remote(a) => {
                ocipkg::image::copy(a.deref_mut() as &mut Remote, builder)?;
            }
        }

        Ok(())
    }
}
