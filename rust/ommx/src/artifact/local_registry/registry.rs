use super::{
    annotations_json, import_legacy_local_registry, import_legacy_local_registry_ref,
    import_legacy_local_registry_ref_with_policy, import_legacy_local_registry_with_policy,
    now_rfc3339, BlobRecord, FileBlobStore, LayerRecord, LegacyImportReport, ManifestRecord,
    OciDirImport, RefConflictPolicy, RefUpdate, SqliteIndexStore, BLOB_KIND_BLOB, BLOB_KIND_CONFIG,
    BLOB_KIND_MANIFEST,
};
use crate::artifact::{media_types, ImageRef, StagedArtifactBlob};
use anyhow::{ensure, Context, Result};
use oci_spec::image::{Descriptor, ImageManifest, MediaType};
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

    pub fn import_legacy_ref(&self, image_name: &ImageRef) -> Result<OciDirImport> {
        import_legacy_local_registry_ref(&self.index, &self.blobs, &self.root, image_name)
    }

    pub fn import_legacy_ref_with_policy(
        &self,
        image_name: &ImageRef,
        policy: RefConflictPolicy,
    ) -> Result<OciDirImport> {
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

    pub fn resolve_image_name(&self, image_name: &ImageRef) -> Result<Option<String>> {
        self.index.resolve_image_name(image_name)
    }

    /// Synthesize a fresh anonymous image name keyed to this
    /// registry's `registry_id`. Format matches
    /// `LocalArtifactBuilder::new_anonymous` and the unnamed-archive
    /// import path: `<registry-id8>.ommx.local/anonymous:<timestamp>-<nonce>`.
    /// Each call returns a new name (the nonce differs); the structural
    /// predicates [`crate::artifact::is_anonymous_artifact_ref_name`]
    /// and [`crate::artifact::is_anonymous_artifact_tag`] match every
    /// name produced this way, so
    /// `ommx artifact prune-anonymous` cleans them uniformly.
    pub fn synthesize_anonymous_image_name(&self) -> Result<ImageRef> {
        let registry_id = self.index.registry_id()?;
        crate::artifact::anonymous_artifact_image_name(&registry_id)
    }

    /// List every SQLite ref whose `(name, reference)` matches the
    /// shape an anonymous artifact's image name would take:
    /// `<registry-id8>.ommx.local/anonymous` (8 lowercase hex chars
    /// prefix + suffix) for the name, and `YYYYMMDDTHHMMSS-<nonce>`
    /// (timestamp + 12-hex random suffix) for the reference. Both
    /// must match — a substring check on the suffix alone would
    /// over-match a human-pushed ref against a real mDNS host like
    /// `myhost.ommx.local/anonymous:v1`. Returned in
    /// `(name, reference)` order to match
    /// [`SqliteIndexStore::list_refs`].
    pub fn list_anonymous_artifact_refs(
        &self,
    ) -> Result<Vec<crate::artifact::local_registry::RefRecord>> {
        let all = self.index.list_refs(None)?;
        Ok(all
            .into_iter()
            .filter(|r| {
                crate::artifact::is_anonymous_artifact_ref_name(&r.name)
                    && crate::artifact::is_anonymous_artifact_tag(&r.reference)
            })
            .collect())
    }

    /// Bulk-delete every SQLite ref produced by
    /// [`crate::artifact::LocalArtifactBuilder::new_anonymous`].
    /// Returns the deleted records so callers (e.g. CLI
    /// `ommx artifact prune-anonymous`) can report what changed. The
    /// manifest / config / layer / blob CAS records the deleted refs
    /// pointed at are **not** touched; they become unreferenced rows
    /// reclaimable by a future GC sweep. This is intentional — the
    /// prune is cheap and the orphan reclamation is the slower /
    /// riskier operation.
    pub fn prune_anonymous_artifact_refs(
        &self,
    ) -> Result<Vec<crate::artifact::local_registry::RefRecord>> {
        let refs = self.list_anonymous_artifact_refs()?;
        for r in &refs {
            self.index.delete_ref(&r.name, &r.reference)?;
        }
        Ok(refs)
    }

    /// Publish a staged OCI Image Manifest bundle to the SQLite Local
    /// Registry. Callers must construct `manifest` and `manifest_descriptor`
    /// via [`crate::artifact::LocalArtifactBuilder`] or the import paths
    /// in `local_registry::import::*`, both of which produce an OCI
    /// Image Manifest with the OMMX `artifactType` field set. The
    /// publish path does not dispatch on manifest format — the SQLite
    /// Local Registry stores OCI Image Manifest exclusively.
    pub(crate) fn publish_artifact_manifest(
        &self,
        image_name: &ImageRef,
        manifest: &ImageManifest,
        manifest_descriptor: &Descriptor,
        manifest_bytes: &[u8],
        blobs: &[StagedArtifactBlob],
        policy: RefConflictPolicy,
    ) -> Result<RefUpdate> {
        ensure!(
            manifest_descriptor.media_type() == &MediaType::ImageManifest,
            "Manifest descriptor must be `{:?}`, got `{}`",
            MediaType::ImageManifest,
            manifest_descriptor.media_type(),
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
        let artifact_type = manifest
            .artifact_type()
            .as_ref()
            .context("Manifest does not carry the OMMX `artifactType` field")?;
        ensure!(
            artifact_type == &MediaType::Other(media_types::V1_ARTIFACT_MEDIA_TYPE.to_string()),
            "Manifest `artifactType` must be `{}`, got `{}`",
            media_types::V1_ARTIFACT_MEDIA_TYPE,
            artifact_type,
        );
        // OCI Image Manifest `blobs` = manifest layers + the `config`
        // descriptor (which is the OCI 1.1 empty config blob in OMMX's
        // builder). Callers stage all of these in `blobs[]`.
        let manifest_descriptor_count = manifest.layers().len() + 1;
        ensure!(
            manifest_descriptor_count == blobs.len(),
            "Manifest descriptor count ({manifest_descriptor_count}) does not match pending blob count ({})",
            blobs.len()
        );
        let staged_descriptors: Vec<&Descriptor> =
            blobs.iter().map(|blob| blob.descriptor()).collect();
        let descriptor_is_staged = |d: &Descriptor| staged_descriptors.contains(&d);
        ensure!(
            descriptor_is_staged(manifest.config()),
            "Manifest config descriptor is not staged for upload"
        );
        for layer in manifest.layers() {
            ensure!(
                descriptor_is_staged(layer),
                "Manifest layer descriptor is not staged for upload: {}",
                layer.digest()
            );
        }

        let manifest_digest = manifest_descriptor.digest().to_string();

        // Pre-check: under `KeepExisting`, return the conflict before
        // we waste any CAS writes. The atomic publish in stage 2
        // re-validates the same condition inside the SQLite
        // transaction, so concurrent racers can't slip through; this
        // is purely a fast path for the common single-writer case.
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

        // Stage 1: write CAS bytes (idempotent, outside any SQLite tx).
        // Stage 2: a single SQLite transaction covers all blob records
        // + manifest + ref so a crash or conflict can never leave
        // committed manifest / blob rows under a ref that wasn't
        // actually published.
        //
        // Tag the manifest's `config` descriptor with `BLOB_KIND_CONFIG`
        // (matching the OCI-dir import path) and everything else with
        // `BLOB_KIND_BLOB`. Without this dispatch the empty config blob
        // built by `LocalArtifactBuilder::stage` would be persisted as a
        // generic layer, diverging from imports of legacy v2 dirs and
        // breaking GC / query logic that filters on `kind`.
        let config_digest = manifest.config().digest();
        let mut blob_records = Vec::with_capacity(blobs.len() + 1);
        for blob in blobs {
            let kind = if blob.descriptor().digest() == config_digest {
                BLOB_KIND_CONFIG
            } else {
                BLOB_KIND_BLOB
            };
            blob_records.push(self.stage_blob_record(blob.descriptor(), blob.bytes(), kind)?);
        }
        blob_records.push(self.stage_blob_record(
            manifest_descriptor,
            manifest_bytes,
            BLOB_KIND_MANIFEST,
        )?);

        let layer_records = manifest
            .layers()
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
        let manifest_record = ManifestRecord {
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
        };

        let outcome = self.index.publish_artifact_atomic(
            &blob_records,
            &manifest_record,
            &layer_records,
            Some(image_name),
            policy,
        )?;
        outcome.ref_update.context(
            "publish_artifact_atomic returned no RefUpdate for an explicit image_name; \
             this is a bug",
        )
    }

    /// CAS-write a descriptor's bytes and produce a [`BlobRecord`] for
    /// the IndexStore. The DB row is *not* inserted here; the caller
    /// passes the records to [`SqliteIndexStore::publish_artifact_atomic`]
    /// so the inserts happen inside the publish transaction.
    fn stage_blob_record(
        &self,
        descriptor: &Descriptor,
        bytes: &[u8],
        kind: &str,
    ) -> Result<BlobRecord> {
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
        Ok(record)
    }
}
