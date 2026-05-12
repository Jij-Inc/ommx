use super::*;
use crate::artifact::{
    media_types, LocalArtifact, LocalArtifactBuilder, LocalManifest, OCI_IMAGE_MANIFEST_MEDIA_TYPE,
};
use anyhow::{Context, Result};
use oci_spec::image::MediaType;
use ocipkg::ImageName;
use ocipkg::{
    image::{ImageBuilder, OciDirBuilder},
    oci_spec::image::{Descriptor, DescriptorBuilder, ImageManifestBuilder},
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

/// Build a tiny single-layer artifact in a fresh temp SQLite registry,
/// save it to `archive_path` via the v3 native save writer, and drop
/// the temp registry. Used by archive-import tests that need a `.ommx`
/// file on disk without polluting the test's main registry.
fn save_test_archive(
    archive_path: &Path,
    image_name: ImageName,
    layer_bytes: Vec<u8>,
) -> Result<()> {
    let sender_dir = tempfile::tempdir()?;
    let sender_registry = Arc::new(LocalRegistry::open(sender_dir.path())?);
    let mut builder = LocalArtifactBuilder::new(image_name);
    builder.add_layer_bytes(
        MediaType::Other(media_types::V1_INSTANCE_MEDIA_TYPE.into()),
        layer_bytes,
        HashMap::new(),
    )?;
    let local = builder.build_in_registry(sender_registry, RefConflictPolicy::Replace)?;
    local.save(archive_path)?;
    Ok(())
}

#[test]
fn file_blob_store_round_trip() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = FileBlobStore::open(dir.path().join("blobs"))?;
    let record = store.put_bytes(b"hello")?;

    assert_eq!(
        record.digest,
        "sha256:2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );
    assert!(store.exists(&record.digest)?);
    assert_eq!(store.read_bytes(&record.digest)?, b"hello");
    assert!(store.path_for_digest("sha256:../../outside").is_err());
    assert!(store.exists("sha256:../../outside").is_err());
    Ok(())
}

#[test]
fn sqlite_index_store_round_trip() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = SqliteIndexStore::open(dir.path().join(SQLITE_INDEX_FILE_NAME))?;
    assert_eq!(store.schema_version()?, 1);

    let manifest_digest = sha256_digest(b"manifest");
    let layer_digest = sha256_digest(b"layer");

    store.put_blob(&BlobRecord {
        digest: manifest_digest.clone(),
        size: b"manifest".len() as u64,
        media_type: Some("application/vnd.oci.image.manifest.v1+json".to_string()),
        storage_uri: "blobs/sha256/manifest".to_string(),
        kind: BLOB_KIND_MANIFEST.to_string(),
        last_verified_at: None,
    })?;
    store.put_blob(&BlobRecord {
        digest: layer_digest.clone(),
        size: b"layer".len() as u64,
        media_type: Some("application/octet-stream".to_string()),
        storage_uri: "blobs/sha256/layer".to_string(),
        kind: BLOB_KIND_LAYER.to_string(),
        last_verified_at: None,
    })?;

    let manifest = ManifestRecord {
        digest: manifest_digest.clone(),
        media_type: "application/vnd.oci.image.manifest.v1+json".to_string(),
        size: b"manifest".len() as u64,
        subject_digest: None,
        annotations_json: "{}".to_string(),
        created_at: now_rfc3339(),
    };
    let layers = [LayerRecord {
        manifest_digest: manifest_digest.clone(),
        position: 0,
        digest: layer_digest.clone(),
        media_type: "application/octet-stream".to_string(),
        size: b"layer".len() as u64,
        annotations_json: "{}".to_string(),
    }];
    store.put_manifest(&manifest, &layers)?;
    store.put_ref("example.com/ommx/experiment", "latest", &manifest_digest)?;

    assert_eq!(store.get_blob(&layer_digest)?.unwrap().kind, "layer");
    assert_eq!(
        store.get_manifest(&manifest_digest)?.unwrap().media_type,
        "application/vnd.oci.image.manifest.v1+json"
    );
    let stored_layers = store.get_layers(&manifest_digest)?;
    assert_eq!(stored_layers, layers);
    assert_eq!(
        store.resolve_ref("example.com/ommx/experiment", "latest")?,
        Some(manifest_digest.clone())
    );
    let refs = store.list_refs(Some("example.com/ommx"))?;
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].reference, "latest");

    store.put_ref("example.com/foo_bar/experiment", "latest", &manifest_digest)?;
    store.put_ref("example.com/fooXbar/experiment", "latest", &manifest_digest)?;
    let refs = store.list_refs(Some("example.com/foo_bar"))?;
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].name, "example.com/foo_bar/experiment");
    Ok(())
}

#[test]
fn concurrent_keep_existing_ref_publish_keeps_one_digest() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let root = dir.path().join("registry-v3");
    let index_store = SqliteIndexStore::open_in_registry_root(&root)?;
    let blob_store = FileBlobStore::open_in_registry_root(&root)?;
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:race")?;
    let first_digest = put_test_manifest(&index_store, &blob_store, b"first-manifest")?;
    let second_digest = put_test_manifest(&index_store, &blob_store, b"second-manifest")?;
    assert_ne!(first_digest, second_digest);

    let handles: Vec<_> = [first_digest.clone(), second_digest.clone()]
        .into_iter()
        .map(|manifest_digest| {
            let root = root.clone();
            let image_name = image_name.clone();
            std::thread::spawn(move || -> Result<RefUpdate> {
                let index_store = SqliteIndexStore::open_in_registry_root(root)?;
                index_store.put_image_ref_with_policy(
                    &image_name,
                    &manifest_digest,
                    RefConflictPolicy::KeepExisting,
                )
            })
        })
        .collect();

    let updates: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("ref publisher thread panicked"))
        .collect::<Result<_>>()?;

    assert_eq!(
        updates
            .iter()
            .filter(|update| matches!(update, RefUpdate::Inserted))
            .count(),
        1
    );
    assert_eq!(
        updates
            .iter()
            .filter(|update| matches!(update, RefUpdate::Conflicted { .. }))
            .count(),
        1
    );
    let final_digest = index_store
        .resolve_image_name(&image_name)?
        .context("Ref was not published")?;
    assert!(final_digest == first_digest || final_digest == second_digest);
    Ok(())
}

#[test]
fn imports_oci_dir_into_sqlite_registry_preserving_image_manifest() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let legacy_dir = dir.path().join("legacy");
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:v1")?;
    let layer = build_test_oci_dir(legacy_dir.clone(), image_name.clone())?;
    // The legacy import path must preserve the manifest digest and bytes,
    // so capture the legacy digest up front and assert identity is intact.
    let expected_digest = oci_dir_ref(&legacy_dir)?.manifest_digest;

    let registry_root = dir.path().join("registry-v3");
    let index_store = SqliteIndexStore::open_in_registry_root(&registry_root)?;
    let blob_store = FileBlobStore::open_in_registry_root(&registry_root)?;

    // Snapshot the original legacy manifest bytes so we can assert
    // byte-for-byte equality with what ends up in the v3 BlobStore.
    let (legacy_algorithm, legacy_encoded) = expected_digest
        .split_once(':')
        .expect("manifest digest is `algorithm:encoded`");
    let legacy_manifest_bytes = std::fs::read(
        legacy_dir
            .join("blobs")
            .join(legacy_algorithm)
            .join(legacy_encoded),
    )?;

    let imported = import_oci_dir(&index_store, &blob_store, &legacy_dir)?;

    assert_eq!(imported.image_name, Some(image_name.clone()));
    assert_eq!(imported.manifest_digest, expected_digest);
    assert_eq!(
        index_store.resolve_image_name(&image_name)?,
        Some(imported.manifest_digest.clone())
    );
    assert!(blob_store.exists(&imported.manifest_digest)?);
    assert!(blob_store.exists(layer.digest().as_ref())?);

    // Strict identity: the manifest bytes the v3 BlobStore returns must
    // be exactly the bytes that lived in the legacy OCI dir. Digest
    // equality already implies this for SHA-256, but a direct check
    // catches any future regression where import accidentally rebuilds
    // / re-serialises the manifest.
    assert_eq!(
        blob_store.read_bytes(&imported.manifest_digest)?,
        legacy_manifest_bytes
    );

    let manifest = index_store
        .get_manifest(&imported.manifest_digest)?
        .context("Imported manifest is missing")?;
    assert_eq!(manifest.media_type, OCI_IMAGE_MANIFEST_MEDIA_TYPE);
    let manifest_blob = index_store
        .get_blob(&manifest.digest)?
        .context("Imported manifest blob is missing")?;
    assert_eq!(manifest_blob.kind, BLOB_KIND_MANIFEST);
    let layers = index_store.get_layers(&imported.manifest_digest)?;
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].digest, layer.digest().to_string());
    let layer_blob = index_store
        .get_blob(&layers[0].digest)?
        .context("Imported layer blob is missing")?;
    assert_eq!(layer_blob.kind, BLOB_KIND_BLOB);

    let artifact = LocalArtifact::open_in_registry(
        Arc::new(LocalRegistry::open(&registry_root)?),
        image_name,
    )?;
    // LocalArtifact must dispatch on the stored manifest media type and
    // surface the legacy Image Manifest's layer descriptors through the
    // common LocalManifest view.
    assert_eq!(
        artifact.get_manifest()?.media_type(),
        OCI_IMAGE_MANIFEST_MEDIA_TYPE
    );
    assert_eq!(artifact.layers()?, vec![layer.clone()]);
    assert_eq!(artifact.get_blob(layer.digest().as_ref())?, b"instance");
    Ok(())
}

#[test]
fn imports_legacy_local_registry_explicitly() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let legacy_registry_root = dir.path().join("legacy-registry");
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:v2")?;
    let legacy_dir = legacy_local_registry_path(&legacy_registry_root, &image_name);
    build_test_oci_dir(legacy_dir, image_name.clone())?;

    let registry_root = dir.path().join("registry-v3");
    let index_store = SqliteIndexStore::open_in_registry_root(&registry_root)?;
    let blob_store = FileBlobStore::open_in_registry_root(&registry_root)?;

    assert!(index_store.resolve_image_name(&image_name)?.is_none());
    let report = import_legacy_local_registry(&index_store, &blob_store, &legacy_registry_root)?;
    assert_eq!(
        report,
        LegacyImportReport {
            scanned_dirs: 1,
            imported_dirs: 1,
            verified_dirs: 0,
            conflicted_dirs: 0,
            replaced_refs: 0
        }
    );
    let imported_digest = index_store
        .resolve_image_name(&image_name)?
        .context("Legacy local registry ref was not imported")?;
    assert_eq!(
        import_legacy_local_registry(&index_store, &blob_store, &legacy_registry_root)?,
        LegacyImportReport {
            scanned_dirs: 1,
            imported_dirs: 0,
            verified_dirs: 1,
            conflicted_dirs: 0,
            replaced_refs: 0
        }
    );
    assert!(blob_store.exists(&imported_digest)?);
    Ok(())
}

#[test]
fn import_legacy_local_registry_keeps_existing_ref_on_conflict() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let legacy_registry_root = dir.path().join("legacy-registry");
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:conflict")?;
    let legacy_dir = legacy_local_registry_path(&legacy_registry_root, &image_name);
    build_test_oci_dir(legacy_dir.clone(), image_name.clone())?;
    let legacy_manifest_digest = oci_dir_ref(&legacy_dir)?.manifest_digest;

    let registry_root = dir.path().join("registry-v3");
    let index_store = SqliteIndexStore::open_in_registry_root(&registry_root)?;
    let blob_store = FileBlobStore::open_in_registry_root(&registry_root)?;
    let existing_digest =
        put_test_manifest_ref(&index_store, &blob_store, &image_name, b"existing-manifest")?;
    assert_ne!(existing_digest, legacy_manifest_digest);

    let report = import_legacy_local_registry(&index_store, &blob_store, &legacy_registry_root)?;
    assert_eq!(
        report,
        LegacyImportReport {
            scanned_dirs: 1,
            imported_dirs: 0,
            verified_dirs: 0,
            conflicted_dirs: 1,
            replaced_refs: 0
        }
    );
    assert_eq!(
        index_store.resolve_image_name(&image_name)?,
        Some(existing_digest)
    );
    assert!(!blob_store.exists(&legacy_manifest_digest)?);
    Ok(())
}

#[test]
fn import_legacy_local_registry_replaces_existing_ref_when_requested() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let legacy_registry_root = dir.path().join("legacy-registry");
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:replace")?;
    let legacy_dir = legacy_local_registry_path(&legacy_registry_root, &image_name);
    build_test_oci_dir(legacy_dir.clone(), image_name.clone())?;
    let legacy_manifest_digest = oci_dir_ref(&legacy_dir)?.manifest_digest;

    let registry_root = dir.path().join("registry-v3");
    let index_store = SqliteIndexStore::open_in_registry_root(&registry_root)?;
    let blob_store = FileBlobStore::open_in_registry_root(&registry_root)?;
    let existing_digest =
        put_test_manifest_ref(&index_store, &blob_store, &image_name, b"existing-manifest")?;
    assert_ne!(existing_digest, legacy_manifest_digest);

    let report = import_legacy_local_registry_with_policy(
        &index_store,
        &blob_store,
        &legacy_registry_root,
        RefConflictPolicy::Replace,
    )?;
    assert_eq!(
        report,
        LegacyImportReport {
            scanned_dirs: 1,
            imported_dirs: 0,
            verified_dirs: 0,
            conflicted_dirs: 0,
            replaced_refs: 1
        }
    );
    assert_eq!(
        index_store.resolve_image_name(&image_name)?,
        Some(legacy_manifest_digest.clone())
    );
    assert!(blob_store.exists(&legacy_manifest_digest)?);
    Ok(())
}

#[test]
fn local_registry_imports_legacy_refs_when_requested() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:v3")?;
    let legacy_dir = legacy_local_registry_path(dir.path(), &image_name);
    build_test_oci_dir(legacy_dir, image_name.clone())?;

    let registry = LocalRegistry::open(dir.path())?;
    assert!(registry.resolve_image_name(&image_name)?.is_none());
    assert_eq!(
        registry.import_legacy_layout()?,
        LegacyImportReport {
            scanned_dirs: 1,
            imported_dirs: 1,
            verified_dirs: 0,
            conflicted_dirs: 0,
            replaced_refs: 0
        }
    );
    let imported_digest = registry
        .resolve_image_name(&image_name)?
        .context("Legacy local registry ref was not imported")?;
    assert!(registry.blobs().exists(&imported_digest)?);
    assert!(registry.index().get_manifest(&imported_digest)?.is_some());
    Ok(())
}

#[test]
fn local_registry_builds_native_image_manifest_with_artifact_type() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = Arc::new(LocalRegistry::open(dir.path())?);
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:built")?;

    let artifact = build_test_local_artifact(&registry, &image_name, b"instance")?;

    let manifest_digest = registry
        .resolve_image_name(&image_name)?
        .context("Published ref is missing")?;
    assert_eq!(manifest_digest, artifact.manifest_digest());
    let manifest_bytes = registry.blobs().read_bytes(&manifest_digest)?;
    let manifest: oci_spec::image::ImageManifest = serde_json::from_slice(&manifest_bytes)?;
    let layer = manifest
        .layers()
        .first()
        .context("Published layer is missing")?;

    let manifest_record = registry
        .index()
        .get_manifest(&manifest_digest)?
        .context("Published manifest is missing")?;
    assert_eq!(manifest_record.media_type, OCI_IMAGE_MANIFEST_MEDIA_TYPE);
    assert_eq!(manifest_record.size, manifest_bytes.len() as u64);
    // Manifest's own `mediaType` field is left unset to match the v2 /
    // ArchiveArtifactBuilder shape; the IndexStore record's `media_type`
    // column carries the format for query / dispatch purposes.
    assert_eq!(manifest.media_type().as_ref(), None);
    assert_eq!(
        manifest.artifact_type().as_ref(),
        Some(&MediaType::Other(
            media_types::V1_ARTIFACT_MEDIA_TYPE.to_string()
        ))
    );
    // Empty config descriptor matches what `ocipkg::OciArtifactBuilder::new`
    // produces by default; OMMX v2 SDK output and v3 SQLite registry agree.
    assert_eq!(manifest.config().media_type(), &MediaType::EmptyJSON);
    assert_eq!(
        manifest.config().digest().to_string(),
        media_types::OCI_EMPTY_CONFIG_DIGEST
    );
    assert_eq!(artifact.layers()?, manifest.layers().to_vec());
    assert_eq!(
        artifact.get_manifest()?.media_type(),
        OCI_IMAGE_MANIFEST_MEDIA_TYPE
    );
    assert_eq!(
        artifact.get_manifest()?.artifact_type(),
        &MediaType::Other(media_types::V1_ARTIFACT_MEDIA_TYPE.to_string())
    );

    let layers = registry.index().get_layers(&manifest_digest)?;
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].digest, layer.digest().to_string());
    assert_eq!(layers[0].media_type, media_types::V1_INSTANCE_MEDIA_TYPE);
    assert_eq!(artifact.get_blob(layer.digest().as_ref())?, b"instance");

    // Empty config blob must be readable from the registry and persisted
    // with `BLOB_KIND_CONFIG`, matching the OCI dir import path. A
    // mis-classified config blob (e.g. recorded as `BLOB_KIND_BLOB`)
    // would break GC reachability analysis and queries that filter by
    // blob kind.
    assert_eq!(
        artifact.get_blob(media_types::OCI_EMPTY_CONFIG_DIGEST)?,
        media_types::OCI_EMPTY_CONFIG_BYTES
    );
    let config_record = registry
        .index()
        .get_blob(media_types::OCI_EMPTY_CONFIG_DIGEST)?
        .context("Empty config blob record is missing")?;
    assert_eq!(config_record.kind, BLOB_KIND_CONFIG);
    Ok(())
}

#[test]
fn local_registry_build_keep_existing_skips_conflicting_manifest() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = Arc::new(LocalRegistry::open(dir.path())?);
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:keep")?;
    let first = build_test_local_artifact(&registry, &image_name, b"first")?;
    let (second, second_blob) = new_test_local_artifact_builder(image_name.clone(), b"second")?;

    let error = second
        .build_in_registry(registry.clone(), RefConflictPolicy::KeepExisting)
        .expect_err("conflicting local registry ref should fail");
    assert!(error.to_string().contains("already points to"));
    assert_eq!(
        registry.resolve_image_name(&image_name)?,
        Some(first.manifest_digest().to_string())
    );
    assert!(!registry.blobs().exists(second_blob.digest().as_ref())?);
    Ok(())
}

#[test]
fn concurrent_legacy_imports_are_idempotent() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let root = dir.path().to_path_buf();
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:parallel")?;
    let legacy_dir = legacy_local_registry_path(&root, &image_name);
    build_test_oci_dir(legacy_dir, image_name.clone())?;

    let handles: Vec<_> = (0..2)
        .map(|_| {
            let root = root.clone();
            std::thread::spawn(move || -> Result<LegacyImportReport> {
                let registry = LocalRegistry::open(root)?;
                registry.import_legacy_layout()
            })
        })
        .collect();

    let reports: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("import thread panicked"))
        .collect::<Result<_>>()?;

    assert_eq!(
        reports
            .iter()
            .map(|report| report.scanned_dirs)
            .sum::<usize>(),
        2
    );
    assert_eq!(
        reports
            .iter()
            .map(|report| report.imported_dirs)
            .sum::<usize>(),
        1
    );
    assert_eq!(
        reports
            .iter()
            .map(|report| report.verified_dirs)
            .sum::<usize>(),
        1
    );
    assert_eq!(
        reports
            .iter()
            .map(|report| report.conflicted_dirs)
            .sum::<usize>(),
        0
    );

    let registry = LocalRegistry::open(&root)?;
    let imported_digest = registry
        .resolve_image_name(&image_name)?
        .context("Legacy local registry ref was not imported")?;
    assert!(registry.blobs().exists(&imported_digest)?);
    Ok(())
}

#[test]
fn import_oci_dir_with_policy_surfaces_ref_update() -> Result<()> {
    // First import returns Inserted; an idempotent re-import returns
    // Unchanged. Both come back through the public OciDirImport.
    let dir = tempfile::tempdir()?;
    let legacy_dir = dir.path().join("legacy");
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:ru1")?;
    build_test_oci_dir(legacy_dir.clone(), image_name.clone())?;

    let registry_root = dir.path().join("registry-v3");
    let index_store = SqliteIndexStore::open_in_registry_root(&registry_root)?;
    let blob_store = FileBlobStore::open_in_registry_root(&registry_root)?;

    let first = import_oci_dir_with_policy(
        &index_store,
        &blob_store,
        &legacy_dir,
        RefConflictPolicy::KeepExisting,
    )?;
    assert_eq!(first.image_name.as_ref(), Some(&image_name));
    assert!(matches!(first.ref_update, Some(RefUpdate::Inserted)));

    let second = import_oci_dir_with_policy(
        &index_store,
        &blob_store,
        &legacy_dir,
        RefConflictPolicy::KeepExisting,
    )?;
    assert_eq!(second.manifest_digest, first.manifest_digest);
    assert!(matches!(second.ref_update, Some(RefUpdate::Unchanged)));
    Ok(())
}

#[test]
fn local_registry_import_legacy_ref_with_policy_replaces_existing() -> Result<()> {
    // Exercises the `_with_policy` variant on `LocalRegistry::import_legacy_ref*`
    // (added so the per-image import has the same Replace / KeepExisting
    // surface as the batch one). With Replace the ref ends up at the
    // legacy digest and the outcome is Replaced { previous }.
    let dir = tempfile::tempdir()?;
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:rwp")?;
    let legacy_dir = legacy_local_registry_path(dir.path(), &image_name);
    build_test_oci_dir(legacy_dir.clone(), image_name.clone())?;
    let legacy_manifest_digest = oci_dir_ref(&legacy_dir)?.manifest_digest;

    let registry = LocalRegistry::open(dir.path())?;
    let existing_digest = put_test_manifest_ref(
        registry.index(),
        registry.blobs(),
        &image_name,
        b"prior-manifest",
    )?;
    assert_ne!(existing_digest, legacy_manifest_digest);

    let import = registry.import_legacy_ref_with_policy(&image_name, RefConflictPolicy::Replace)?;
    assert_eq!(import.manifest_digest, legacy_manifest_digest);
    assert!(matches!(
        import.ref_update,
        Some(RefUpdate::Replaced { ref previous_manifest_digest })
            if previous_manifest_digest == &existing_digest
    ));
    assert_eq!(
        registry.resolve_image_name(&image_name)?,
        Some(legacy_manifest_digest)
    );
    Ok(())
}

#[test]
fn local_artifact_caches_manifest_across_clones() -> Result<()> {
    // get_manifest() memoises into an Arc<OnceLock<LocalManifest>>; the
    // same artifact handed out via Clone shares the cell. A reference
    // returned by one handle and another reference returned by its
    // clone must point at the same cached value.
    let dir = tempfile::tempdir()?;
    let registry = Arc::new(LocalRegistry::open(dir.path())?);
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:cache")?;

    let artifact = build_test_local_artifact(&registry, &image_name, b"cache-test")?;
    let m1 = artifact.get_manifest()? as *const LocalManifest;
    let m2 = artifact.get_manifest()? as *const LocalManifest;
    assert_eq!(m1, m2, "second get_manifest() must reuse the cached value");

    let cloned = artifact.clone();
    let m3 = cloned.get_manifest()? as *const LocalManifest;
    assert_eq!(
        m1, m3,
        "clones must share the OnceLock cell, not produce a separate parse"
    );
    Ok(())
}

#[test]
fn local_artifact_subject_round_trips() -> Result<()> {
    // LocalArtifact::subject() goes through the cached LocalManifest
    // and surfaces the Descriptor that LocalArtifactBuilder set via
    // `set_subject`. None when no subject is set.
    let dir = tempfile::tempdir()?;
    let registry = Arc::new(LocalRegistry::open(dir.path())?);
    let plain_image = ImageName::parse("ghcr.io/jij-inc/ommx/demo:plain")?;
    let plain = build_test_local_artifact(&registry, &plain_image, b"no-subject")?;
    assert_eq!(plain.subject()?, None);

    let subject_descriptor = oci_spec::image::DescriptorBuilder::default()
        .media_type(MediaType::ImageManifest)
        .digest(oci_spec::image::Digest::from_str(&sha256_digest(
            b"parent-manifest-bytes",
        ))?)
        .size(b"parent-manifest-bytes".len() as u64)
        .build()?;

    let child_image = ImageName::parse("ghcr.io/jij-inc/ommx/demo:child")?;
    let mut builder = LocalArtifactBuilder::new(child_image.clone());
    builder.add_layer_bytes(
        MediaType::Other(media_types::V1_INSTANCE_MEDIA_TYPE.to_string()),
        b"child-layer".to_vec(),
        HashMap::new(),
    )?;
    builder.set_subject(subject_descriptor.clone());
    let child = builder.build_in_registry(registry.clone(), RefConflictPolicy::KeepExisting)?;
    assert_eq!(child.subject()?, Some(subject_descriptor));
    Ok(())
}

#[test]
fn imports_legacy_v2_oci_dir_with_ommx_config_blob() -> Result<()> {
    // v2 SDK can produce Image Manifests whose `config` blob is an
    // OMMX-specific `application/org.ommx.v1.config+json` (instead of
    // the v3 builder's OCI 1.1 empty descriptor). v3 import / read must
    // preserve such manifests verbatim: parse-time check is artifactType
    // only (no config-shape requirement), and the config blob lands as
    // `BLOB_KIND_CONFIG` in the IndexStore so GC reachability and
    // queries treat it consistently with the v3 empty-config path.
    let dir = tempfile::tempdir()?;
    let legacy_dir = dir.path().join("v2-legacy");
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:v2-config")?;
    let v2_config_bytes = br#"{"description":"v2 legacy config"}"#;
    let (config_descriptor, layer_descriptor) =
        build_test_oci_dir_with_v2_config(legacy_dir.clone(), image_name.clone(), v2_config_bytes)?;

    let registry_root = dir.path().join("registry-v3");
    let index_store = SqliteIndexStore::open_in_registry_root(&registry_root)?;
    let blob_store = FileBlobStore::open_in_registry_root(&registry_root)?;
    let imported = import_oci_dir(&index_store, &blob_store, &legacy_dir)?;

    assert_eq!(imported.image_name, Some(image_name.clone()));
    assert!(blob_store.exists(&imported.manifest_digest)?);
    assert!(blob_store.exists(layer_descriptor.digest().as_ref())?);

    // OMMX-specific config blob is preserved with `BLOB_KIND_CONFIG`.
    let config_digest_str = config_descriptor.digest().to_string();
    assert!(blob_store.exists(&config_digest_str)?);
    let config_blob = index_store
        .get_blob(&config_digest_str)?
        .context("Imported config blob is missing")?;
    assert_eq!(config_blob.kind, BLOB_KIND_CONFIG);
    assert_eq!(
        config_blob.media_type.as_deref(),
        Some(media_types::V1_CONFIG_MEDIA_TYPE)
    );
    assert_eq!(blob_store.read_bytes(&config_digest_str)?, v2_config_bytes);

    // LocalArtifact reads the legacy manifest (parse-time check is on
    // artifactType only, so the OMMX-specific config is not rejected).
    let registry = Arc::new(LocalRegistry::open(&registry_root)?);
    let artifact = LocalArtifact::open_in_registry(registry, image_name)?;
    assert_eq!(
        artifact.get_manifest()?.media_type(),
        OCI_IMAGE_MANIFEST_MEDIA_TYPE
    );
    assert_eq!(artifact.layers()?, vec![layer_descriptor]);
    Ok(())
}

#[test]
fn rejects_import_of_deprecated_artifact_manifest_layout() -> Result<()> {
    // v3 does not support OCI Artifact Manifest
    // (`application/vnd.oci.artifact.manifest.v1+json`); import must
    // reject such layouts with a clear error, not silently fall back.
    let dir = tempfile::tempdir()?;
    let oci_dir = dir.path().join("oci-art");
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:art")?;
    build_test_oci_dir_with_artifact_manifest(&oci_dir, &image_name, b"art-instance")?;

    let registry_root = dir.path().join("registry-v3");
    let index_store = SqliteIndexStore::open_in_registry_root(&registry_root)?;
    let blob_store = FileBlobStore::open_in_registry_root(&registry_root)?;
    let err = import_oci_dir(&index_store, &blob_store, &oci_dir)
        .expect_err("Artifact Manifest import must error");
    let message = format!("{err:#}");
    assert!(
        message.contains("OCI Artifact Manifest"),
        "Error must mention the rejected format; got: {message}"
    );
    Ok(())
}

#[test]
fn concurrent_publish_different_digests_keeps_one_winner() -> Result<()> {
    // Two LocalArtifactBuilder writers race to publish *different*
    // manifest digests under the same image_name. With KeepExisting
    // policy the atomic publish must let exactly one writer win
    // (`Inserted`) and the other must surface as a conflict-error,
    // never leaving the registry in a state where a different
    // manifest digest claims the ref.
    let dir = tempfile::tempdir()?;
    let registry_root = dir.path().to_path_buf();
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:race-publish")?;

    let handles: Vec<_> = (0..2)
        .map(|i| {
            let registry_root = registry_root.clone();
            let image_name = image_name.clone();
            std::thread::spawn(move || -> Result<bool> {
                let registry = Arc::new(LocalRegistry::open(registry_root)?);
                let bytes = format!("racer-{i}");
                let (builder, _) =
                    new_test_local_artifact_builder(image_name.clone(), bytes.as_bytes())?;
                match builder.build_in_registry(registry, RefConflictPolicy::KeepExisting) {
                    Ok(_) => Ok(true),
                    Err(err) => {
                        // Only the conflict outcome is acceptable here.
                        assert!(
                            err.to_string().contains("already points to"),
                            "unexpected build_in_registry error: {err}"
                        );
                        Ok(false)
                    }
                }
            })
        })
        .collect();

    let outcomes: Vec<bool> = handles
        .into_iter()
        .map(|h| h.join().expect("publisher thread panicked"))
        .collect::<Result<_>>()?;

    let winners = outcomes.iter().filter(|w| **w).count();
    let losers = outcomes.iter().filter(|w| !**w).count();
    assert_eq!(winners, 1, "exactly one publisher must win the ref");
    assert_eq!(losers, 1, "exactly one publisher must surface a conflict");

    let final_registry = LocalRegistry::open(&registry_root)?;
    let resolved = final_registry
        .resolve_image_name(&image_name)?
        .context("ref disappeared after concurrent publish")?;
    assert!(
        !resolved.is_empty(),
        "ref must still resolve to the winning manifest digest"
    );
    Ok(())
}

#[test]
fn concurrent_blob_writes_publish_one_complete_blob() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let root = dir.path().join("blobs");
    let bytes = b"parallel blob".to_vec();

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let root = root.clone();
            let bytes = bytes.clone();
            std::thread::spawn(move || -> Result<BlobRecord> {
                let store = FileBlobStore::open(root)?;
                store.put_bytes(&bytes)
            })
        })
        .collect();

    let records: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("blob writer thread panicked"))
        .collect::<Result<_>>()?;

    let digest = sha256_digest(&bytes);
    assert!(records.iter().all(|record| record.digest == digest));
    let store = FileBlobStore::open(&root)?;
    assert_eq!(store.read_bytes(&digest)?, bytes);
    Ok(())
}

#[test]
fn import_oci_archive_surfaces_digest_conflict_for_same_ref() -> Result<()> {
    // Importing a second .ommx archive that shares the first's image
    // name but carries different bytes must surface a ref conflict
    // under the default `KeepExisting` policy, not a stale `Unchanged`.
    // Each call to `import_oci_archive` stages into a fresh tempdir
    // that is dropped before the function returns, so SQLite's ref
    // conflict check sees the new archive's freshly hashed manifest
    // digest rather than the prior archive's bytes.
    let dir = tempfile::tempdir()?;
    let registry = Arc::new(LocalRegistry::open(dir.path())?);
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:reextract")?;

    let archive_path_a = dir.path().join("a.ommx");
    save_test_archive(&archive_path_a, image_name.clone(), b"archive-A".to_vec())?;
    let archive_path_b = dir.path().join("b.ommx");
    save_test_archive(&archive_path_b, image_name.clone(), b"archive-B".to_vec())?;

    let outcome_a = import_oci_archive(&registry, &archive_path_a)?;
    let digest_a = outcome_a.manifest_digest.clone();

    // Stale legacy dir would silently shadow archive B's bytes. The
    // fix re-extracts B over the legacy path, so the second import sees
    // B's digest and surfaces a ref conflict against A under the
    // default `KeepExisting` policy.
    let err = import_oci_archive(&registry, &archive_path_b)
        .expect_err("second import with a different manifest digest must surface a conflict");
    let msg = err.to_string();
    assert!(
        msg.contains(&digest_a),
        "conflict message should mention archive A's existing digest, got: {msg}",
    );
    // The "incoming" side of the conflict must be a digest distinct
    // from A — i.e. B's digest — proving that the implementation read
    // B's freshly extracted bytes and not the stale A bytes.
    assert!(
        msg.contains("incoming manifest sha256:")
            && !msg.contains(&format!("incoming manifest {digest_a}")),
        "incoming digest must differ from archive A (i.e. come from B's re-extracted bytes): {msg}",
    );
    Ok(())
}

#[test]
fn import_oci_archive_does_not_leave_legacy_dir_behind() -> Result<()> {
    // Step C invariant: after a successful `import_oci_archive`, the
    // v2-era path `registry.root().join(image_name.as_path())` must
    // not exist. The archive's staging tempdir is created under the
    // registry root and dropped before the function returns; SQLite +
    // FileBlobStore are the sole post-import home.
    let dir = tempfile::tempdir()?;
    let registry = Arc::new(LocalRegistry::open(dir.path())?);
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:no-legacy-dir")?;
    let archive_path = dir.path().join("artifact.ommx");
    save_test_archive(
        &archive_path,
        image_name.clone(),
        b"step-c-payload".to_vec(),
    )?;

    let outcome = import_oci_archive(&registry, &archive_path)?;
    assert_eq!(outcome.image_name.as_ref(), Some(&image_name));

    let v2_path = registry.root().join(image_name.as_path());
    assert!(
        !v2_path.exists(),
        "legacy v2 OCI dir must not be promoted under registry root, but {} exists",
        v2_path.display(),
    );
    Ok(())
}

#[cfg(feature = "remote-artifact")]
#[test]
fn pull_image_short_circuits_when_ref_is_present_with_blob() -> Result<()> {
    // Step C fast path: `pull_image` against a ref already published
    // in the SQLite Local Registry must return `Unchanged` without
    // touching the network. Constructing the artifact via
    // `LocalArtifactBuilder` (no network) and then calling
    // `pull_image` against an unresolvable host exercises this — if
    // the short-circuit ever regresses, the call would attempt a DNS
    // lookup against a `.invalid` TLD and fail.
    let dir = tempfile::tempdir()?;
    let registry = Arc::new(LocalRegistry::open(dir.path())?);
    let image_name = ImageName::parse("does-not-resolve.invalid/jij-inc/ommx/demo:short-circuit")?;
    let local_artifact =
        build_test_local_artifact(&registry, &image_name, b"step-c-pull-short-circuit")?;
    let expected_digest = local_artifact.manifest_digest().to_string();

    let outcome = super::import::remote::pull_image(&registry, &image_name)?;
    assert_eq!(outcome.image_name.as_ref(), Some(&image_name));
    assert_eq!(outcome.manifest_digest, expected_digest);
    assert!(
        matches!(outcome.ref_update, Some(RefUpdate::Unchanged)),
        "expected RefUpdate::Unchanged on SQLite-hit short-circuit, got {:?}",
        outcome.ref_update,
    );
    Ok(())
}

#[cfg(feature = "remote-artifact")]
#[test]
fn pull_image_does_not_short_circuit_when_manifest_blob_is_missing() -> Result<()> {
    // P1 guard from Codex review: if the SQLite ref resolves but the
    // manifest blob is missing from the FileBlobStore (registry
    // corruption, manual deletion, interrupted import), `pull_image`
    // must fall through to a fresh pull rather than return a stale
    // `Unchanged`. We can't run the fall-through path without a real
    // remote, so the assertion is: an unreachable remote produces an
    // error (i.e., the function did attempt the pull) rather than the
    // happy-path `Unchanged` it would return without the blob check.
    let dir = tempfile::tempdir()?;
    let registry = Arc::new(LocalRegistry::open(dir.path())?);
    let image_name = ImageName::parse("does-not-resolve.invalid/jij-inc/ommx/demo:blob-missing")?;
    let local_artifact =
        build_test_local_artifact(&registry, &image_name, b"step-c-blob-corruption")?;

    // Simulate corruption: remove the manifest blob file under the
    // FileBlobStore while keeping the SQLite ref intact.
    let manifest_digest = local_artifact.manifest_digest().to_string();
    let blob_path = registry.blobs().path_for_digest(&manifest_digest)?;
    std::fs::remove_file(&blob_path)
        .with_context(|| format!("Failed to remove manifest blob at {}", blob_path.display()))?;

    let result = super::import::remote::pull_image(&registry, &image_name);
    assert!(
        result.is_err(),
        "pull_image must fall through to a remote pull when the manifest blob is \
         missing; the unreachable host should surface as Err, but got {:?}",
        result,
    );
    Ok(())
}

#[test]
fn local_artifact_save_round_trip_preserves_layers() -> Result<()> {
    // `LocalArtifact::save` is the CLI `save` command's only path
    // post-Step C. Verify the produced archive: (a) is a valid OCI
    // archive that `Artifact::from_oci_archive` can open, (b)
    // exposes the OMMX artifactType, (c) preserves layer descriptors
    // and bytes byte-for-byte. The manifest digest is **not**
    // asserted equal: `OciArchiveBuilder::build` re-serialises the
    // parsed `ImageManifest`, which can produce a different byte
    // representation (this is documented in `save.rs`'s module doc).
    use ocipkg::image::{Image, OciArchive};
    let dir = tempfile::tempdir()?;
    let registry = Arc::new(LocalRegistry::open(dir.path())?);
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:save-round-trip")?;
    let layer_bytes = b"step-c-save-round-trip-payload";
    let local_artifact = build_test_local_artifact(&registry, &image_name, layer_bytes)?;
    let expected_layers = local_artifact.layers()?;
    let archive_path = dir.path().join("round-trip.ommx");

    local_artifact.save(&archive_path)?;
    assert!(
        archive_path.is_file(),
        "save() must produce the archive file at {}",
        archive_path.display(),
    );

    let mut archive = ocipkg::image::OciArtifact::<OciArchive>::from_oci_archive(&archive_path)?;
    let manifest = archive.get_manifest()?;
    assert_eq!(
        manifest.artifact_type().as_ref(),
        Some(&crate::artifact::media_types::v1_artifact()),
        "saved archive must carry the OMMX artifactType",
    );
    assert_eq!(
        manifest.layers().len(),
        expected_layers.len(),
        "saved archive layer count differs from source",
    );
    for (expected, actual) in expected_layers.iter().zip(manifest.layers()) {
        assert_eq!(expected.digest(), actual.digest(), "layer digest drift");
        assert_eq!(expected.size(), actual.size(), "layer size drift");
        assert_eq!(
            expected.media_type(),
            actual.media_type(),
            "layer media_type drift",
        );
        let blob: Vec<u8> = archive.get_blob(actual.digest())?.to_vec();
        assert_eq!(
            blob.as_slice(),
            layer_bytes,
            "saved archive layer bytes differ from source",
        );
    }
    Ok(())
}

fn build_test_local_artifact(
    registry: &Arc<LocalRegistry>,
    image_name: &ImageName,
    layer_bytes: &[u8],
) -> Result<LocalArtifact> {
    let (builder, _) = new_test_local_artifact_builder(image_name.clone(), layer_bytes)?;
    builder.build_in_registry(registry.clone(), RefConflictPolicy::KeepExisting)
}

fn new_test_local_artifact_builder(
    image_name: ImageName,
    layer_bytes: &[u8],
) -> Result<(LocalArtifactBuilder, Descriptor)> {
    let mut builder = LocalArtifactBuilder::new(image_name);
    let descriptor = builder.add_layer_bytes(
        MediaType::Other(media_types::V1_INSTANCE_MEDIA_TYPE.to_string()),
        layer_bytes.to_vec(),
        HashMap::from([("org.ommx.v1.instance.title".to_string(), "demo".to_string())]),
    )?;
    Ok((builder, descriptor))
}

fn put_test_manifest_ref(
    index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    image_name: &ImageName,
    bytes: &[u8],
) -> Result<String> {
    let digest = put_test_manifest(index_store, blob_store, bytes)?;
    index_store.put_image_ref(image_name, &digest)?;
    Ok(digest)
}

fn put_test_manifest(
    index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    bytes: &[u8],
) -> Result<String> {
    let mut blob = blob_store.put_bytes(bytes)?;
    blob.media_type = Some("application/vnd.oci.image.manifest.v1+json".to_string());
    blob.kind = BLOB_KIND_MANIFEST.to_string();
    index_store.put_blob(&blob)?;
    index_store.put_manifest(
        &ManifestRecord {
            digest: blob.digest.clone(),
            media_type: "application/vnd.oci.image.manifest.v1+json".to_string(),
            size: bytes.len() as u64,
            subject_digest: None,
            annotations_json: "{}".to_string(),
            created_at: now_rfc3339(),
        },
        &[],
    )?;
    Ok(blob.digest)
}

fn build_test_oci_dir(legacy_dir: PathBuf, image_name: ImageName) -> Result<Descriptor> {
    let mut builder = OciDirBuilder::new(legacy_dir, image_name)?;

    let config = builder.add_empty_json()?;
    let (layer_digest, layer_size) = builder.add_blob(b"instance")?;
    let layer = DescriptorBuilder::default()
        .media_type(MediaType::Other(
            "application/org.ommx.v1.instance".to_string(),
        ))
        .digest(layer_digest)
        .size(layer_size)
        .build()?;
    let manifest = ImageManifestBuilder::default()
        .schema_version(2_u32)
        .artifact_type(media_types::v1_artifact())
        .config(config)
        .layers(vec![layer.clone()])
        .build()?;
    let _oci_dir = builder.build(manifest)?;
    Ok(layer)
}

/// Build an OCI Image Layout whose Image Manifest carries an
/// OMMX-specific config blob (`application/org.ommx.v1.config+json`)
/// rather than the OCI 1.1 empty descriptor. Mirrors the manifest shape
/// that SDK v2 produced when the user explicitly called
/// `ArchiveArtifactBuilder::add_config`, so the v3 import path can be
/// exercised against the legacy variant the SDK still has to read.
fn build_test_oci_dir_with_v2_config(
    legacy_dir: PathBuf,
    image_name: ImageName,
    config_bytes: &[u8],
) -> Result<(Descriptor, Descriptor)> {
    let mut builder = OciDirBuilder::new(legacy_dir, image_name)?;
    let (config_digest, config_size) = builder.add_blob(config_bytes)?;
    let config = DescriptorBuilder::default()
        .media_type(MediaType::Other(media_types::V1_CONFIG_MEDIA_TYPE.into()))
        .digest(config_digest)
        .size(config_size)
        .build()?;
    let (layer_digest, layer_size) = builder.add_blob(b"v2-instance")?;
    let layer = DescriptorBuilder::default()
        .media_type(MediaType::Other(
            media_types::V1_INSTANCE_MEDIA_TYPE.to_string(),
        ))
        .digest(layer_digest)
        .size(layer_size)
        .build()?;
    let manifest = ImageManifestBuilder::default()
        .schema_version(2_u32)
        .artifact_type(media_types::v1_artifact())
        .config(config.clone())
        .layers(vec![layer.clone()])
        .build()?;
    let _oci_dir = builder.build(manifest)?;
    Ok((config, layer))
}

/// Build a v3-shaped OCI Image Layout directory whose manifest is an
/// OCI **Artifact** Manifest (no `config`, layers in `blobs[]`). Used
/// to exercise the Artifact-Manifest dispatch in `import_oci_dir`.
fn build_test_oci_dir_with_artifact_manifest(
    oci_dir: &Path,
    image_name: &ImageName,
    layer_bytes: &[u8],
) -> Result<(Descriptor, String)> {
    use oci_spec::image::{
        ArtifactManifestBuilder, ImageIndexBuilder, OciLayoutBuilder, SCHEMA_VERSION,
    };
    use std::fs;

    let blobs_dir = oci_dir.join("blobs/sha256");
    fs::create_dir_all(&blobs_dir)?;
    fs::write(oci_dir.join("oci-layout"), {
        let layout = OciLayoutBuilder::default()
            .image_layout_version("1.0.0")
            .build()?;
        serde_json::to_vec(&layout)?
    })?;

    // Write a single layer blob to the CAS and build its descriptor.
    let layer_digest_str = sha256_digest(layer_bytes);
    let layer_digest_encoded = layer_digest_str
        .strip_prefix("sha256:")
        .expect("sha256 digest format");
    fs::write(blobs_dir.join(layer_digest_encoded), layer_bytes)?;
    let layer = DescriptorBuilder::default()
        .media_type(MediaType::Other(media_types::V1_INSTANCE_MEDIA_TYPE.into()))
        .digest(oci_spec::image::Digest::from_str(&layer_digest_str)?)
        .size(layer_bytes.len() as u64)
        .build()?;

    // Build the Artifact Manifest pointing at the layer.
    let manifest = ArtifactManifestBuilder::default()
        .artifact_type(media_types::v1_artifact())
        .blobs(vec![layer.clone()])
        .build()?;
    let manifest_bytes = serde_json::to_vec(&manifest)?;
    let manifest_digest_str = sha256_digest(&manifest_bytes);
    let manifest_digest_encoded = manifest_digest_str
        .strip_prefix("sha256:")
        .expect("sha256 digest format");
    fs::write(blobs_dir.join(manifest_digest_encoded), &manifest_bytes)?;

    // Manifest descriptor in `index.json` carries the
    // ref-name annotation and the ArtifactManifest media type.
    let mut annotations = HashMap::new();
    annotations.insert(
        OCI_IMAGE_REF_NAME_ANNOTATION.to_string(),
        image_name.to_string(),
    );
    let index_descriptor = DescriptorBuilder::default()
        .media_type(MediaType::ArtifactManifest)
        .digest(oci_spec::image::Digest::from_str(&manifest_digest_str)?)
        .size(manifest_bytes.len() as u64)
        .annotations(annotations)
        .build()?;
    let image_index = ImageIndexBuilder::default()
        .schema_version(SCHEMA_VERSION)
        .manifests(vec![index_descriptor])
        .build()?;
    fs::write(
        oci_dir.join("index.json"),
        serde_json::to_vec(&image_index)?,
    )?;

    Ok((layer, manifest_digest_str))
}
