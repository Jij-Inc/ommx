use super::{
    import_legacy_local_registry_ref, migrate_legacy_local_registry,
    migrate_legacy_local_registry_with_policy, now_rfc3339, FileBlobStore, LayerRecord,
    LegacyMigrationReport, LegacyOciDirImport, ManifestRecord, RefConflictPolicy, RefUpdate,
    SqliteIndexStore, BLOB_KIND_CONFIG, BLOB_KIND_LAYER, BLOB_KIND_MANIFEST,
};
use crate::artifact::{ArtifactBlob, ArtifactDescriptor, BuiltArtifactManifest};
use anyhow::{ensure, Context, Result};
use ocipkg::ImageName;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct LocalRegistry {
    root: PathBuf,
    index: SqliteIndexStore,
    blobs: FileBlobStore,
}

impl LocalRegistry {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        let index = SqliteIndexStore::open_in_registry_root(&root)?;
        let blobs = FileBlobStore::open_in_registry_root(&root)?;
        Ok(Self { root, index, blobs })
    }

    pub fn open_default() -> Result<Self> {
        Self::open(crate::artifact::get_local_registry_root())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn index(&self) -> &SqliteIndexStore {
        &self.index
    }

    pub fn blobs(&self) -> &FileBlobStore {
        &self.blobs
    }

    pub fn import_legacy_ref(&self, image_name: &ImageName) -> Result<LegacyOciDirImport> {
        import_legacy_local_registry_ref(&self.index, &self.blobs, &self.root, image_name)
    }

    pub fn migrate_legacy_layout(&self) -> Result<LegacyMigrationReport> {
        migrate_legacy_local_registry(&self.index, &self.blobs, &self.root)
    }

    pub fn migrate_legacy_layout_with_policy(
        &self,
        policy: RefConflictPolicy,
    ) -> Result<LegacyMigrationReport> {
        migrate_legacy_local_registry_with_policy(&self.index, &self.blobs, &self.root, policy)
    }

    pub fn resolve_image_name(&self, image_name: &ImageName) -> Result<Option<String>> {
        self.index.resolve_image_name(image_name)
    }

    pub fn publish_built_manifest(
        &self,
        image_name: &ImageName,
        artifact: &BuiltArtifactManifest,
        policy: RefConflictPolicy,
    ) -> Result<RefUpdate> {
        let manifest_digest = artifact.manifest_descriptor().digest();
        if policy == RefConflictPolicy::KeepExisting {
            if let Some(existing_manifest_digest) = self.resolve_image_name(image_name)? {
                if existing_manifest_digest != manifest_digest {
                    return Ok(RefUpdate::Conflicted {
                        existing_manifest_digest,
                        incoming_manifest_digest: manifest_digest.to_string(),
                    });
                }
            }
        }

        self.put_artifact_blob(artifact.config(), BLOB_KIND_CONFIG)?;
        for layer in artifact.layers() {
            self.put_artifact_blob(layer, BLOB_KIND_LAYER)?;
        }
        self.put_descriptor_bytes(
            artifact.manifest_descriptor(),
            artifact.manifest_bytes(),
            BLOB_KIND_MANIFEST,
        )?;

        let layers = artifact
            .manifest()
            .layers()
            .iter()
            .enumerate()
            .map(|(position, layer)| -> Result<LayerRecord> {
                Ok(LayerRecord {
                    manifest_digest: manifest_digest.to_string(),
                    position: u32::try_from(position)
                        .context("Layer position does not fit in u32")?,
                    digest: layer.digest().to_string(),
                    media_type: layer.media_type().to_string(),
                    size: layer.size(),
                    annotations_json: serde_json::to_string(layer.annotations())
                        .context("Failed to encode layer annotations")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        self.index.put_manifest(
            &ManifestRecord {
                digest: manifest_digest.to_string(),
                media_type: artifact.manifest_descriptor().media_type().to_string(),
                size: artifact.manifest_descriptor().size(),
                subject_digest: artifact
                    .manifest()
                    .subject()
                    .map(|subject| subject.digest().to_string()),
                annotations_json: serde_json::to_string(artifact.manifest().annotations())
                    .context("Failed to encode manifest annotations")?,
                created_at: now_rfc3339(),
            },
            &layers,
        )?;
        self.index
            .put_image_ref_with_policy(image_name, manifest_digest, policy)
    }

    fn put_artifact_blob(&self, blob: &ArtifactBlob, kind: &str) -> Result<()> {
        self.put_descriptor_bytes(blob.descriptor(), blob.bytes(), kind)
    }

    fn put_descriptor_bytes(
        &self,
        descriptor: &ArtifactDescriptor,
        bytes: &[u8],
        kind: &str,
    ) -> Result<()> {
        let mut record = self.blobs.put_bytes(bytes)?;
        ensure!(
            record.digest == descriptor.digest(),
            "Descriptor digest mismatch: descriptor={}, actual={}",
            descriptor.digest(),
            record.digest
        );
        ensure!(
            record.size == descriptor.size(),
            "Descriptor size mismatch for {}: descriptor={}, actual={}",
            descriptor.digest(),
            descriptor.size(),
            record.size
        );
        record.media_type = Some(descriptor.media_type().to_string());
        record.kind = kind.to_string();
        self.index.put_blob(&record)
    }
}
