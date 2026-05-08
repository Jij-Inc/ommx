use super::{
    annotations_json, import_legacy_local_registry, import_legacy_local_registry_ref,
    import_legacy_local_registry_ref_with_policy, import_legacy_local_registry_with_policy,
    now_rfc3339, FileBlobStore, LayerRecord, LegacyImportReport, LegacyOciDirRef, ManifestRecord,
    RefConflictPolicy, RefUpdate, SqliteIndexStore, BLOB_KIND_BLOB, BLOB_KIND_MANIFEST,
};
use crate::artifact::{PendingArtifactBlob, OCI_ARTIFACT_MANIFEST_MEDIA_TYPE};
use anyhow::{ensure, Context, Result};
use oci_spec::image::{ArtifactManifest, Descriptor, MediaType};
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

    pub fn import_legacy_ref(&self, image_name: &ImageName) -> Result<LegacyOciDirRef> {
        import_legacy_local_registry_ref(&self.index, &self.blobs, &self.root, image_name)
    }

    pub fn import_legacy_ref_with_policy(
        &self,
        image_name: &ImageName,
        policy: RefConflictPolicy,
    ) -> Result<LegacyOciDirRef> {
        import_legacy_local_registry_ref_with_policy(
            &self.index,
            &self.blobs,
            &self.root,
            image_name,
            policy,
        )
    }

    pub fn import_legacy_layout(&self) -> Result<LegacyImportReport> {
        import_legacy_local_registry(&self.index, &self.blobs, &self.root)
    }

    pub fn import_legacy_layout_with_policy(
        &self,
        policy: RefConflictPolicy,
    ) -> Result<LegacyImportReport> {
        import_legacy_local_registry_with_policy(&self.index, &self.blobs, &self.root, policy)
    }

    pub fn resolve_image_name(&self, image_name: &ImageName) -> Result<Option<String>> {
        self.index.resolve_image_name(image_name)
    }

    pub(crate) fn publish_artifact_manifest(
        &self,
        image_name: &ImageName,
        manifest: &ArtifactManifest,
        manifest_descriptor: &Descriptor,
        manifest_bytes: &[u8],
        blobs: &[PendingArtifactBlob],
        policy: RefConflictPolicy,
    ) -> Result<RefUpdate> {
        ensure!(
            manifest.media_type().as_ref() == OCI_ARTIFACT_MANIFEST_MEDIA_TYPE,
            "Manifest is not an OCI artifact manifest: {}",
            manifest.media_type()
        );
        ensure!(
            manifest_descriptor.media_type() == &MediaType::ArtifactManifest,
            "Manifest descriptor is not an OCI artifact manifest descriptor: {}",
            manifest_descriptor.media_type()
        );
        ensure!(
            manifest_descriptor.digest().to_string()
                == crate::artifact::sha256_digest(manifest_bytes),
            "Manifest descriptor digest does not match manifest bytes"
        );
        ensure!(
            manifest_descriptor.size() == manifest_bytes.len() as u64,
            "Manifest descriptor size does not match manifest bytes"
        );
        ensure!(
            manifest.blobs().len() == blobs.len(),
            "Manifest blob descriptor count does not match pending blob count"
        );
        for (manifest_blob, pending_blob) in manifest.blobs().iter().zip(blobs) {
            ensure!(
                manifest_blob == pending_blob.descriptor(),
                "Manifest blob descriptor does not match pending blob descriptor"
            );
        }

        let manifest_digest = manifest_descriptor.digest().to_string();
        if policy == RefConflictPolicy::KeepExisting {
            if let Some(existing_manifest_digest) = self.resolve_image_name(image_name)? {
                if existing_manifest_digest != manifest_digest.as_str() {
                    return Ok(RefUpdate::Conflicted {
                        existing_manifest_digest,
                        incoming_manifest_digest: manifest_digest,
                    });
                }
            }
        }

        for blob in blobs {
            self.put_artifact_blob(blob, BLOB_KIND_BLOB)?;
        }
        self.put_descriptor_bytes(manifest_descriptor, manifest_bytes, BLOB_KIND_MANIFEST)?;

        let layers = manifest
            .blobs()
            .iter()
            .enumerate()
            .map(|(position, layer)| -> Result<LayerRecord> {
                Ok(LayerRecord {
                    manifest_digest: manifest_digest.clone(),
                    position: u32::try_from(position)
                        .context("Layer position does not fit in u32")?,
                    digest: layer.digest().to_string(),
                    media_type: layer.media_type().to_string(),
                    size: layer.size(),
                    annotations_json: annotations_json(layer.annotations().as_ref())
                        .context("Failed to encode layer annotations")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        self.index.put_manifest(
            &ManifestRecord {
                digest: manifest_digest.clone(),
                media_type: manifest_descriptor.media_type().to_string(),
                size: manifest_descriptor.size(),
                subject_digest: manifest
                    .subject()
                    .as_ref()
                    .map(|subject| subject.digest().to_string()),
                annotations_json: annotations_json(manifest.annotations().as_ref())
                    .context("Failed to encode manifest annotations")?,
                created_at: now_rfc3339(),
            },
            &layers,
        )?;
        self.index
            .put_image_ref_with_policy(image_name, &manifest_digest, policy)
    }

    fn put_artifact_blob(&self, blob: &PendingArtifactBlob, kind: &str) -> Result<()> {
        self.put_descriptor_bytes(blob.descriptor(), blob.bytes(), kind)
    }

    fn put_descriptor_bytes(
        &self,
        descriptor: &Descriptor,
        bytes: &[u8],
        kind: &str,
    ) -> Result<()> {
        let mut record = self.blobs.put_bytes(bytes)?;
        ensure!(
            record.digest == descriptor.digest().to_string(),
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
