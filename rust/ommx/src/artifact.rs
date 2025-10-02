//! Manage messages as container
//!

mod annotations;
mod builder;
mod config;
pub mod media_types;
pub use annotations::*;
pub use builder::*;
pub use config::*;

use crate::v1;
use anyhow::{bail, ensure, Context, Result};
use ocipkg::{
    distribution::MediaType,
    image::{
        Image, OciArchive, OciArchiveBuilder, OciArtifact, OciDir, OciDirBuilder, Remote,
        RemoteBuilder,
    },
    oci_spec::image::{Descriptor, ImageManifest},
    Digest, ImageName,
};
use prost::Message;
use std::{env, path::PathBuf, sync::OnceLock};
use std::{
    ops::{Deref, DerefMut},
    path::Path,
};

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
pub fn get_image_dir(image_name: &ImageName) -> PathBuf {
    get_local_registry_root().join(image_name.as_path())
}

/// Get the artifact path (directory or archive file) for the given image name in the local registry
/// Returns the path to either an oci-dir directory or an oci-archive file
pub fn get_artifact_path(image_name: &ImageName) -> Option<PathBuf> {
    let root = get_local_registry_root();
    
    // First check for oci-archive format (new default)
    let archive_path = root.join(format!("{}.ommx", image_name.as_path().display()));
    if archive_path.exists() && archive_path.is_file() {
        return Some(archive_path);
    }
    
    // Fallback to oci-dir format (backward compatibility)
    let dir_path = get_image_dir(image_name);
    if dir_path.exists() && dir_path.is_dir() && dir_path.join("oci-layout").exists() {
        return Some(dir_path);
    }
    
    None
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
    let root = get_local_registry_root();
    let artifacts = gather_oci_dirs(root)?;
    artifacts.into_iter()
        .map(|artifact_path| {
            let relative = artifact_path
                .strip_prefix(root)
                .context("Failed to get relative path")?;
            
            // For archive files, we need to extract the image name differently
            if artifact_path.is_file() {
                // Try to extract image name from the archive metadata
                if let Ok(mut artifact) = Artifact::from_oci_archive(&artifact_path) {
                    if let Ok(name) = artifact.get_name() {
                        return Ok(name);
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

/// OMMX Artifact, an OCI Artifact of type [`application/org.ommx.v1.artifact`][media_types::v1_artifact]
pub struct Artifact<Base: Image>(OciArtifact<Base>);

impl<Base: Image> Deref for Artifact<Base> {
    type Target = OciArtifact<Base>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<Base: Image> DerefMut for Artifact<Base> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Artifact<OciArchive> {
    pub fn from_oci_archive(path: &Path) -> Result<Self> {
        let artifact = OciArtifact::from_oci_archive(path)?;
        Self::new(artifact)
    }

    pub fn push(&mut self) -> Result<Artifact<Remote>> {
        let name = self.get_name()?;
        log::info!("Pushing: {name}");
        let mut remote = RemoteBuilder::new(name)?;
        if let Ok((domain, username, password)) = auth_from_env() {
            remote.add_basic_auth(&domain, &username, &password);
        }
        let out = ocipkg::image::copy(self.0.deref_mut(), remote)?;
        Ok(Artifact(OciArtifact::new(out)))
    }

    pub fn load(&mut self) -> Result<()> {
        let image_name = self.get_name()?;
        let path = get_image_dir(&image_name);
        if path.exists() {
            log::trace!("Already exists in local registry: {}", path.display());
            return Ok(());
        }
        log::info!("Loading to local registry: {image_name}");
        ocipkg::image::copy(self.0.deref_mut(), OciDirBuilder::new(path, image_name)?)?;
        Ok(())
    }
}

impl Artifact<OciDir> {
    pub fn from_oci_dir(path: &Path) -> Result<Self> {
        let artifact = OciArtifact::from_oci_dir(path)?;
        Self::new(artifact)
    }

    pub fn push(&mut self) -> Result<Artifact<Remote>> {
        let name = self.get_name()?;
        log::info!("Pushing: {name}");
        let mut remote = RemoteBuilder::new(name)?;
        if let Ok((domain, username, password)) = auth_from_env() {
            remote.add_basic_auth(&domain, &username, &password);
        }
        let out = ocipkg::image::copy(self.0.deref_mut(), remote)?;
        Ok(Artifact(OciArtifact::new(out)))
    }

    pub fn save(&mut self, output: &Path) -> Result<()> {
        if output.exists() {
            bail!("Output file already exists: {}", output.display());
        }
        let builder = if let Ok(name) = self.get_name() {
            OciArchiveBuilder::new(output.to_path_buf(), name)?
        } else {
            OciArchiveBuilder::new_unnamed(output.to_path_buf())?
        };
        ocipkg::image::copy(self.0.deref_mut(), builder)?;
        Ok(())
    }
}

impl Artifact<Remote> {
    pub fn from_remote(image_name: ImageName) -> Result<Self> {
        let artifact = OciArtifact::from_remote(image_name)?;
        Self::new(artifact)
    }

    pub fn pull(&mut self) -> Result<Artifact<OciDir>> {
        let image_name = self.get_name()?;
        let path = get_image_dir(&image_name);
        if path.exists() {
            log::trace!("Already exists in local registry: {}", path.display());
            return Ok(Artifact(OciArtifact::from_oci_dir(&path)?));
        }
        log::info!("Pulling to local registry: {image_name}");
        if let Ok((domain, username, password)) = auth_from_env() {
            self.0.add_basic_auth(&domain, &username, &password);
        }
        let out = ocipkg::image::copy(self.0.deref_mut(), OciDirBuilder::new(path, image_name)?)?;
        Ok(Artifact(OciArtifact::new(out)))
    }
}

impl<Base: Image> Artifact<Base> {
    pub fn new(artifact: OciArtifact<Base>) -> Result<Self> {
        Ok(Self(artifact))
    }

    pub fn get_manifest(&mut self) -> Result<ImageManifest> {
        let manifest = self.0.get_manifest()?;
        let ty = manifest
            .artifact_type()
            .as_ref()
            .context("Not an OMMX Artifact")?;
        ensure!(
            *ty == media_types::v1_artifact(),
            "Not an OMMX Artifact: {}",
            ty
        );
        Ok(manifest)
    }

    pub fn get_config(&mut self) -> Result<Config> {
        let (_desc, blob) = self.0.get_config()?;
        let config = serde_json::from_slice(&blob)?;
        Ok(config)
    }

    pub fn get_layer_descriptors(&mut self, media_type: &MediaType) -> Result<Vec<Descriptor>> {
        let manifest = self.get_manifest()?;
        Ok(manifest
            .layers()
            .iter()
            .filter(|desc| desc.media_type() == media_type)
            .cloned()
            .collect())
    }

    pub fn get_layer(&mut self, digest: &Digest) -> Result<(Descriptor, Vec<u8>)> {
        for (desc, blob) in self.0.get_layers()? {
            if desc.digest() == &digest.to_string() {
                return Ok((desc, blob));
            }
        }
        bail!("Layer of digest {} not found", digest)
    }

    pub fn get_solution(&mut self, digest: &Digest) -> Result<(v1::State, SolutionAnnotations)> {
        let (desc, blob) = self.get_layer(digest)?;
        ensure!(
            desc.media_type() == &media_types::v1_solution(),
            "Layer {digest} is not an ommx.v1.Solution: {}",
            desc.media_type()
        );
        Ok((
            v1::State::decode(blob.as_slice())?,
            SolutionAnnotations::from_descriptor(&desc),
        ))
    }

    pub fn get_sample_set(
        &mut self,
        digest: &Digest,
    ) -> Result<(v1::SampleSet, SampleSetAnnotations)> {
        let (desc, blob) = self.get_layer(digest)?;
        ensure!(
            desc.media_type() == &media_types::v1_sample_set(),
            "Layer {digest} is not an ommx.v1.SampleSet: {}",
            desc.media_type()
        );
        Ok((
            v1::SampleSet::decode(blob.as_slice())?,
            SampleSetAnnotations::from_descriptor(&desc),
        ))
    }

    pub fn get_instance(&mut self, digest: &Digest) -> Result<(v1::Instance, InstanceAnnotations)> {
        let (desc, blob) = self.get_layer(digest)?;
        ensure!(
            desc.media_type() == &media_types::v1_instance(),
            "Layer {digest} is not an ommx.v1.Instance: {}",
            desc.media_type()
        );
        Ok((
            v1::Instance::decode(blob.as_slice())?,
            InstanceAnnotations::from_descriptor(&desc),
        ))
    }

    pub fn get_parametric_instance(
        &mut self,
        digest: &Digest,
    ) -> Result<(v1::ParametricInstance, ParametricInstanceAnnotations)> {
        let (desc, blob) = self.get_layer(digest)?;
        ensure!(
            desc.media_type() == &media_types::v1_parametric_instance(),
            "Layer {digest} is not an ommx.v1.ParametricInstance: {}",
            desc.media_type()
        );
        Ok((
            v1::ParametricInstance::decode(blob.as_slice())?,
            ParametricInstanceAnnotations::from_descriptor(&desc),
        ))
    }

    pub fn get_solutions(&mut self) -> Result<Vec<(Descriptor, v1::State)>> {
        let mut out = Vec::new();
        for (desc, blob) in self.0.get_layers()? {
            if desc.media_type() != &media_types::v1_solution() {
                continue;
            }
            let solution = v1::State::decode(blob.as_slice())?;
            out.push((desc, solution));
        }
        Ok(out)
    }

    pub fn get_instances(&mut self) -> Result<Vec<(Descriptor, v1::Instance)>> {
        let mut out = Vec::new();
        for (desc, blob) in self.0.get_layers()? {
            if desc.media_type() != &media_types::v1_instance() {
                continue;
            }
            let instance = v1::Instance::decode(blob.as_slice())?;
            out.push((desc, instance));
        }
        Ok(out)
    }
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
    fn test_get_artifact_path_none_when_not_exists() {
        // Test with a non-existent image name
        let image_name = ImageName::parse("test.local/nonexistent:v1").unwrap();
        let result = get_artifact_path(&image_name);
        
        // Should return None for non-existent artifacts
        assert!(result.is_none());
    }

    #[test]
    fn test_gather_artifacts_empty_dir() {
        let temp_dir = std::env::temp_dir().join(format!("ommx_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();
        let result = gather_artifacts(&temp_dir).unwrap();
        assert!(result.is_empty());
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test] 
    fn test_gather_artifacts_with_mock_oci_dir() {
        let temp_dir = std::env::temp_dir().join(format!("ommx_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();
        
        // Create a mock OCI directory structure
        let oci_dir = temp_dir.join("test-image");
        fs::create_dir_all(&oci_dir).unwrap();
        fs::write(oci_dir.join("oci-layout"), r#"{"imageLayoutVersion": "1.0.0"}"#).unwrap();
        
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
        fs::write(oci_dir.join("oci-layout"), r#"{"imageLayoutVersion": "1.0.0"}"#).unwrap();
        
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
