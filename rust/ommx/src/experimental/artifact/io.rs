//! I/O operations for Artifact (load, save)

use super::Artifact;
use crate::artifact::get_local_registry_path;
use anyhow::{anyhow, bail, ensure, Context, Result};
use ocipkg::{
    image::{
        OciArchive, OciArchiveBuilder, OciArtifact, OciDir, OciDirBuilder, Remote, RemoteBuilder,
    },
    ImageName,
};
use std::{fs, ops::DerefMut, path::Path};

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

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create parent directory: {}", parent.display())
            })?;
        }

        if let Self::Remote(remote) = self {
            apply_basic_auth(remote);
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

        *self = Self::from_oci_archive(path)?;

        Ok(())
    }

    /// Save the artifact as an OCI directory
    pub fn save_as_dir(&mut self, path: &Path) -> Result<()> {
        if path.exists() {
            bail!("Output directory already exists: {}", path.display());
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create parent directory: {}", parent.display())
            })?;
        }

        if let Self::Remote(remote) = self {
            apply_basic_auth(remote);
        }

        let image_name = self
            .image_name()
            .context("Cannot save artifact without image name")?;
        let image_name = ImageName::parse(&image_name)?;

        let builder = OciDirBuilder::new(path.to_path_buf(), image_name)?;

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

        *self = Self::from_oci_dir(path)?;

        Ok(())
    }

    /// Save the artifact to the local registry (defaults to archive format)
    pub fn save(&mut self) -> Result<()> {
        let image_name = self
            .image_name()
            .context("Cannot save artifact without image name")?;
        let image_name = ImageName::parse(&image_name)?;
        let base_path = get_local_registry_path(&image_name);
        let archive_path = base_path.with_extension("ommx");

        if archive_path.exists() {
            log::debug!(
                "Archive already exists; reusing: {}",
                archive_path.display()
            );
            *self = Self::from_oci_archive(&archive_path)?;
            return Ok(());
        }

        if let Some(parent) = archive_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create registry directory: {}", parent.display())
            })?;
        }

        self.save_as_archive(&archive_path)
    }

    /// Pull the artifact from remote into the local registry (archive format)
    pub fn pull(&mut self) -> Result<()> {
        ensure!(
            matches!(self, Self::Remote(_)),
            "pull() is only supported for remote artifacts"
        );
        self.save()
    }

    /// Push the artifact to the remote registry
    pub fn push(&mut self) -> Result<()> {
        let image_name = self
            .image_name()
            .context("Cannot push artifact without image name")?;
        let image_name = ImageName::parse(&image_name)?;

        match self {
            Self::Archive(archive) => {
                push_to_remote(archive.deref_mut(), image_name)?;
            }
            Self::Dir(dir) => {
                push_to_remote(dir.deref_mut(), image_name)?;
            }
            Self::Remote(_) => {
                bail!("Cannot push a Remote artifact without local data");
            }
        }

        Ok(())
    }
}

fn push_to_remote<T>(image: &mut T, image_name: ImageName) -> Result<()>
where
    T: ocipkg::image::Image,
{
    let mut builder = RemoteBuilder::new(image_name.clone())?;
    if let Ok((domain, username, password)) = auth_from_env() {
        builder.add_basic_auth(&domain, &username, &password);
    }
    ocipkg::image::copy(image, builder)?;
    Ok(())
}

fn apply_basic_auth(remote: &mut OciArtifact<Remote>) {
    if let Ok((domain, username, password)) = auth_from_env() {
        remote.add_basic_auth(&domain, &username, &password);
    }
}

fn auth_from_env() -> Result<(String, String, String)> {
    if let (Ok(domain), Ok(username), Ok(password)) = (
        std::env::var("OMMX_BASIC_AUTH_DOMAIN"),
        std::env::var("OMMX_BASIC_AUTH_USERNAME"),
        std::env::var("OMMX_BASIC_AUTH_PASSWORD"),
    ) {
        log::info!("Using OMMX basic auth from environment for remote registry access");
        return Ok((domain, username, password));
    }
    Err(anyhow!("Basic auth credentials not configured"))
}
