use super::*;
use crate::artifact::{
    media_types, LocalArtifact, LocalArtifactBuilder, LocalManifest,
    OCI_ARTIFACT_MANIFEST_MEDIA_TYPE, OCI_IMAGE_MANIFEST_MEDIA_TYPE,
};
use anyhow::{Context, Result};
use oci_spec::image::{ArtifactManifest, MediaType};
use ocipkg::ImageName;
use ocipkg::{
    image::{ImageBuilder, OciDirBuilder},
    oci_spec::image::{Descriptor, DescriptorBuilder, ImageManifestBuilder},
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

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
    assert!(blob_store.exists(&layer.digest().to_string())?);

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
    assert!(matches!(artifact.get_manifest()?, LocalManifest::Image(_)));
    assert_eq!(artifact.layers()?, vec![layer.clone()]);
    assert_eq!(artifact.get_blob(&layer.digest().to_string())?, b"instance");
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
fn local_registry_builds_native_artifact_manifest() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = Arc::new(LocalRegistry::open(dir.path())?);
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:built")?;

    let artifact = build_test_local_artifact(&registry, &image_name, b"instance")?;

    let manifest_digest = registry
        .resolve_image_name(&image_name)?
        .context("Published ref is missing")?;
    assert_eq!(manifest_digest, artifact.manifest_digest());
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
    assert_eq!(manifest_record.size, manifest_bytes.len() as u64);
    assert_eq!(manifest.media_type(), &MediaType::ArtifactManifest);
    assert_eq!(
        manifest.artifact_type(),
        &MediaType::Other(media_types::V1_ARTIFACT_MEDIA_TYPE.to_string())
    );
    assert_eq!(artifact.layers()?, manifest.blobs().to_vec());
    // LocalArtifact must dispatch on the stored manifest media type and
    // surface the v3 native Artifact Manifest's blob descriptors through
    // the common LocalManifest view (symmetric to the legacy import
    // test that exercises LocalManifest::Image).
    assert!(matches!(
        artifact.get_manifest()?,
        LocalManifest::Artifact(_)
    ));
    assert_eq!(
        artifact.get_manifest()?.artifact_type(),
        &MediaType::Other(media_types::V1_ARTIFACT_MEDIA_TYPE.to_string())
    );

    let layers = registry.index().get_layers(&manifest_digest)?;
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].digest, blob.digest().to_string());
    assert_eq!(layers[0].media_type, media_types::V1_INSTANCE_MEDIA_TYPE);
    assert_eq!(artifact.get_blob(&blob.digest().to_string())?, b"instance");
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
    assert!(!registry.blobs().exists(&second_blob.digest().to_string())?);
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
        .media_type(MediaType::ArtifactManifest)
        .digest(oci_spec::image::Digest::from_str(&sha256_digest(
            b"parent-manifest-bytes",
        ))?)
        .size(b"parent-manifest-bytes".len() as u64)
        .build()?;

    let child_image = ImageName::parse("ghcr.io/jij-inc/ommx/demo:child")?;
    let mut builder = LocalArtifactBuilder::new_ommx(child_image.clone());
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
fn imports_oci_dir_with_artifact_manifest_layout() -> Result<()> {
    // OCI dir whose manifest is an Artifact Manifest (no Image
    // config, layers in `blobs[]`) must import as identity-preserving
    // and surface as `LocalManifest::Artifact` on read.
    let dir = tempfile::tempdir()?;
    let oci_dir = dir.path().join("oci-art");
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:art")?;
    let (layer, expected_manifest_digest) =
        build_test_oci_dir_with_artifact_manifest(&oci_dir, &image_name, b"art-instance")?;

    let registry_root = dir.path().join("registry-v3");
    let index_store = SqliteIndexStore::open_in_registry_root(&registry_root)?;
    let blob_store = FileBlobStore::open_in_registry_root(&registry_root)?;
    let imported = import_oci_dir(&index_store, &blob_store, &oci_dir)?;

    assert_eq!(imported.manifest_digest, expected_manifest_digest);
    assert_eq!(imported.image_name.as_ref(), Some(&image_name));
    let manifest_record = index_store
        .get_manifest(&imported.manifest_digest)?
        .context("Imported manifest is missing")?;
    assert_eq!(manifest_record.media_type, OCI_ARTIFACT_MANIFEST_MEDIA_TYPE);
    assert!(blob_store.exists(&imported.manifest_digest)?);
    assert!(blob_store.exists(&layer.digest().to_string())?);

    let registry = LocalRegistry::open(&registry_root)?;
    let artifact = LocalArtifact::open_in_registry(Arc::new(registry), image_name)?;
    assert!(matches!(
        artifact.get_manifest()?,
        LocalManifest::Artifact(_)
    ));
    assert_eq!(artifact.layers()?, vec![layer]);
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
fn import_oci_archive_re_extracts_when_legacy_dir_is_stale() -> Result<()> {
    // P1 regression: importing a second .ommx archive with the same
    // image_name but different content used to leave the prior archive's
    // bytes at `legacy_path` (because `load_to` skipped when the dir
    // existed), so `import_oci_dir_as_ref` re-imported the *old* manifest
    // digest and returned `Unchanged` instead of surfacing the digest
    // mismatch. Fixed by always staging into a temp dir and atomically
    // promoting it over `legacy_path`. This test would loop back into a
    // false `Ok(Unchanged)` without that fix.
    let dir = tempfile::tempdir()?;
    let registry = Arc::new(LocalRegistry::open(dir.path())?);
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:reextract")?;

    let archive_path_a = dir.path().join("a.ommx");
    {
        let mut builder = crate::artifact::ArchiveArtifactBuilder::new_archive(
            archive_path_a.clone(),
            image_name.clone(),
        )?;
        builder.add_layer(
            MediaType::Other(media_types::V1_INSTANCE_MEDIA_TYPE.into()),
            b"archive-A",
            HashMap::new(),
        )?;
        let _ = builder.build()?;
    }
    let archive_path_b = dir.path().join("b.ommx");
    {
        let mut builder = crate::artifact::ArchiveArtifactBuilder::new_archive(
            archive_path_b.clone(),
            image_name.clone(),
        )?;
        builder.add_layer(
            MediaType::Other(media_types::V1_INSTANCE_MEDIA_TYPE.into()),
            b"archive-B",
            HashMap::new(),
        )?;
        let _ = builder.build()?;
    }

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
    let mut builder = LocalArtifactBuilder::new_ommx(image_name);
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
