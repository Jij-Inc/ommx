use super::*;
use crate::artifact::{
    media_types, LocalArtifactBuild, LocalArtifactBuilder, OCI_ARTIFACT_MANIFEST_MEDIA_TYPE,
};
use anyhow::{Context, Result};
use oci_spec::image::{ArtifactManifest, MediaType};
use ocipkg::ImageName;
use ocipkg::{
    image::{ImageBuilder, OciDirBuilder},
    oci_spec::image::{Descriptor, DescriptorBuilder, ImageManifestBuilder},
};
use std::collections::HashMap;
use std::path::PathBuf;

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
fn imports_legacy_oci_dir_into_sqlite_registry() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let legacy_dir = dir.path().join("legacy");
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:v1")?;
    let layer = build_test_legacy_oci_dir(legacy_dir.clone(), image_name.clone())?;

    let registry_root = dir.path().join("registry-v3");
    let index_store = SqliteIndexStore::open_in_registry_root(&registry_root)?;
    let blob_store = FileBlobStore::open_in_registry_root(&registry_root)?;

    let imported = import_legacy_oci_dir(&index_store, &blob_store, &legacy_dir)?;

    assert_eq!(imported.image_name, Some(image_name.clone()));
    assert_eq!(
        index_store.resolve_image_name(&image_name)?,
        Some(imported.manifest_digest.clone())
    );
    assert!(blob_store.exists(&imported.manifest_digest)?);
    assert!(blob_store.exists(&layer.digest().to_string())?);

    let manifest = index_store
        .get_manifest(&imported.manifest_digest)?
        .context("Imported manifest is missing")?;
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
    assert_eq!(layer_blob.kind, BLOB_KIND_LAYER);
    Ok(())
}

#[test]
fn migrates_legacy_local_registry_explicitly() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let legacy_registry_root = dir.path().join("legacy-registry");
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:v2")?;
    let legacy_dir = legacy_local_registry_path(&legacy_registry_root, &image_name);
    build_test_legacy_oci_dir(legacy_dir, image_name.clone())?;

    let registry_root = dir.path().join("registry-v3");
    let index_store = SqliteIndexStore::open_in_registry_root(&registry_root)?;
    let blob_store = FileBlobStore::open_in_registry_root(&registry_root)?;

    assert!(index_store.resolve_image_name(&image_name)?.is_none());
    let report = migrate_legacy_local_registry(&index_store, &blob_store, &legacy_registry_root)?;
    assert_eq!(
        report,
        LegacyMigrationReport {
            scanned_dirs: 1,
            imported_dirs: 1,
            verified_dirs: 0,
            conflicted_dirs: 0,
            replaced_refs: 0
        }
    );
    let imported_digest = index_store
        .resolve_image_name(&image_name)?
        .context("Legacy local registry ref was not migrated")?;
    assert_eq!(
        migrate_legacy_local_registry(&index_store, &blob_store, &legacy_registry_root)?,
        LegacyMigrationReport {
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
fn migrate_legacy_local_registry_keeps_existing_ref_on_conflict() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let legacy_registry_root = dir.path().join("legacy-registry");
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:conflict")?;
    let legacy_dir = legacy_local_registry_path(&legacy_registry_root, &image_name);
    build_test_legacy_oci_dir(legacy_dir.clone(), image_name.clone())?;
    let legacy_manifest_digest = legacy_oci_dir_ref(&legacy_dir)?.manifest_digest;

    let registry_root = dir.path().join("registry-v3");
    let index_store = SqliteIndexStore::open_in_registry_root(&registry_root)?;
    let blob_store = FileBlobStore::open_in_registry_root(&registry_root)?;
    let existing_digest =
        put_test_manifest_ref(&index_store, &blob_store, &image_name, b"existing-manifest")?;
    assert_ne!(existing_digest, legacy_manifest_digest);

    let report = migrate_legacy_local_registry(&index_store, &blob_store, &legacy_registry_root)?;
    assert_eq!(
        report,
        LegacyMigrationReport {
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
fn migrate_legacy_local_registry_replaces_existing_ref_when_requested() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let legacy_registry_root = dir.path().join("legacy-registry");
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:replace")?;
    let legacy_dir = legacy_local_registry_path(&legacy_registry_root, &image_name);
    build_test_legacy_oci_dir(legacy_dir.clone(), image_name.clone())?;
    let legacy_manifest_digest = legacy_oci_dir_ref(&legacy_dir)?.manifest_digest;

    let registry_root = dir.path().join("registry-v3");
    let index_store = SqliteIndexStore::open_in_registry_root(&registry_root)?;
    let blob_store = FileBlobStore::open_in_registry_root(&registry_root)?;
    let existing_digest =
        put_test_manifest_ref(&index_store, &blob_store, &image_name, b"existing-manifest")?;
    assert_ne!(existing_digest, legacy_manifest_digest);

    let report = migrate_legacy_local_registry_with_policy(
        &index_store,
        &blob_store,
        &legacy_registry_root,
        RefConflictPolicy::Replace,
    )?;
    assert_eq!(
        report,
        LegacyMigrationReport {
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
fn local_registry_migrates_legacy_refs_when_requested() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:v3")?;
    let legacy_dir = legacy_local_registry_path(dir.path(), &image_name);
    build_test_legacy_oci_dir(legacy_dir, image_name.clone())?;

    let registry = LocalRegistry::open(dir.path())?;
    assert!(registry.resolve_image_name(&image_name)?.is_none());
    assert_eq!(
        registry.migrate_legacy_layout()?,
        LegacyMigrationReport {
            scanned_dirs: 1,
            imported_dirs: 1,
            verified_dirs: 0,
            conflicted_dirs: 0,
            replaced_refs: 0
        }
    );
    let imported_digest = registry
        .resolve_image_name(&image_name)?
        .context("Legacy local registry ref was not migrated")?;
    assert!(registry.blobs().exists(&imported_digest)?);
    assert!(registry.index().get_manifest(&imported_digest)?.is_some());
    Ok(())
}

#[test]
fn local_registry_builds_native_artifact_manifest() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:built")?;

    let built = build_test_local_artifact(&registry, &image_name, b"instance")?;

    assert_eq!(built.ref_update(), &RefUpdate::Inserted);
    let manifest_digest = registry
        .resolve_image_name(&image_name)?
        .context("Published ref is missing")?;
    assert_eq!(
        manifest_digest,
        built.manifest_descriptor().digest().to_string()
    );
    let manifest_bytes = registry.blobs().read_bytes(&manifest_digest)?;
    let manifest: ArtifactManifest = serde_json::from_slice(&manifest_bytes)?;
    let blob = manifest
        .blobs()
        .first()
        .context("Published blob is missing")?;

    let manifest_record = registry
        .index()
        .get_manifest(&manifest_digest)?
        .context("Published manifest is missing")?;
    assert_eq!(manifest_record.media_type, OCI_ARTIFACT_MANIFEST_MEDIA_TYPE);
    assert_eq!(manifest_record.size, built.manifest_descriptor().size());
    assert_eq!(manifest.media_type(), &MediaType::ArtifactManifest);
    assert_eq!(
        manifest.artifact_type(),
        &MediaType::Other(media_types::V1_ARTIFACT_MEDIA_TYPE.to_string())
    );

    let layers = registry.index().get_layers(&manifest_digest)?;
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].digest, blob.digest().to_string());
    assert_eq!(layers[0].media_type, media_types::V1_INSTANCE_MEDIA_TYPE);
    assert_eq!(
        registry.blobs().read_bytes(&blob.digest().to_string())?,
        b"instance"
    );
    Ok(())
}

#[test]
fn local_registry_build_keep_existing_skips_conflicting_manifest() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:keep")?;
    let first = build_test_local_artifact(&registry, &image_name, b"first")?;
    let second = build_test_local_artifact(&registry, &image_name, b"second")?;

    assert_eq!(first.ref_update(), &RefUpdate::Inserted);
    assert_eq!(
        second.ref_update(),
        &RefUpdate::Conflicted {
            existing_manifest_digest: first.manifest_descriptor().digest().to_string(),
            incoming_manifest_digest: second.manifest_descriptor().digest().to_string()
        }
    );
    assert_eq!(
        registry.resolve_image_name(&image_name)?,
        Some(first.manifest_descriptor().digest().to_string())
    );
    assert!(registry
        .index()
        .get_manifest(&second.manifest_descriptor().digest().to_string())?
        .is_none());
    assert!(!registry
        .blobs()
        .exists(&second.manifest_descriptor().digest().to_string())?);
    Ok(())
}

#[test]
fn concurrent_legacy_migrations_are_idempotent() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let root = dir.path().to_path_buf();
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:parallel")?;
    let legacy_dir = legacy_local_registry_path(&root, &image_name);
    build_test_legacy_oci_dir(legacy_dir, image_name.clone())?;

    let handles: Vec<_> = (0..2)
        .map(|_| {
            let root = root.clone();
            std::thread::spawn(move || -> Result<LegacyMigrationReport> {
                let registry = LocalRegistry::open(root)?;
                registry.migrate_legacy_layout()
            })
        })
        .collect();

    let reports: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("migration thread panicked"))
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
        .context("Legacy local registry ref was not migrated")?;
    assert!(registry.blobs().exists(&imported_digest)?);
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

fn build_test_local_artifact(
    registry: &LocalRegistry,
    image_name: &ImageName,
    layer_bytes: &[u8],
) -> Result<LocalArtifactBuild> {
    let mut builder = LocalArtifactBuilder::new_ommx();
    builder.add_blob_bytes(
        MediaType::Other(media_types::V1_INSTANCE_MEDIA_TYPE.to_string()),
        layer_bytes.to_vec(),
        HashMap::from([("org.ommx.v1.instance.title".to_string(), "demo".to_string())]),
    )?;
    builder.build_local(registry, image_name, RefConflictPolicy::KeepExisting)
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

fn build_test_legacy_oci_dir(legacy_dir: PathBuf, image_name: ImageName) -> Result<Descriptor> {
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
