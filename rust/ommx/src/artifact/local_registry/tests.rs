use super::*;
use crate::artifact::{
    media_types, ImageRef, LocalArtifact, LocalArtifactBuilder, LocalManifest,
    OCI_IMAGE_MANIFEST_MEDIA_TYPE,
};
use anyhow::{Context, Result};
use oci_spec::image::{Descriptor, DescriptorBuilder, Digest, ImageManifestBuilder, MediaType};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

/// Build a tiny single-layer artifact in a fresh temp SQLite registry,
/// save it to `archive_path` via the v3 native save writer, and drop
/// the temp registry. Used by archive-import tests that need a `.ommx`
/// file on disk without polluting the test's main registry.
fn save_test_archive(
    archive_path: &Path,
    image_name: ImageRef,
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
    let digest = store.put_bytes(b"hello")?;

    assert_eq!(
        digest.as_ref(),
        "sha256:2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );
    assert!(store.exists(&digest)?);
    assert_eq!(store.read_bytes(&digest)?, b"hello");
    assert!(Digest::from_str("sha256:../../outside").is_err());
    Ok(())
}

#[test]
fn sqlite_index_store_round_trip() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = SqliteIndexStore::open(dir.path().join(SQLITE_INDEX_FILE_NAME))?;
    assert_eq!(store.schema_version()?, 1);

    let manifest_descriptor = test_manifest_descriptor(b"manifest")?;
    store.put_ref(
        "example.com/ommx/experiment",
        "latest",
        &manifest_descriptor,
    )?;
    assert_eq!(
        store.resolve_ref("example.com/ommx/experiment", "latest")?,
        Some(manifest_descriptor.clone())
    );
    let refs = store.list_refs(Some("example.com/ommx"))?;
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].reference, "latest");
    assert_eq!(refs[0].descriptor, manifest_descriptor);

    let manifest_descriptor = test_manifest_descriptor(b"other-manifest")?;
    store.put_ref(
        "example.com/foo_bar/experiment",
        "latest",
        &manifest_descriptor,
    )?;
    store.put_ref(
        "example.com/fooXbar/experiment",
        "latest",
        &manifest_descriptor,
    )?;
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
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:race")?;
    let first_descriptor = put_test_manifest(&index_store, &blob_store, b"first-manifest")?;
    let second_descriptor = put_test_manifest(&index_store, &blob_store, b"second-manifest")?;
    let first_digest = first_descriptor.digest().clone();
    let second_digest = second_descriptor.digest().clone();
    assert_ne!(first_digest, second_digest);

    let handles: Vec<_> = [first_descriptor.clone(), second_descriptor.clone()]
        .into_iter()
        .map(|manifest_descriptor| {
            let root = root.clone();
            let image_name = image_name.clone();
            std::thread::spawn(move || -> Result<RefUpdate> {
                let index_store = SqliteIndexStore::open_in_registry_root(root)?;
                index_store.put_image_ref_with_policy(
                    &image_name,
                    &manifest_descriptor,
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
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:v1")?;
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
        .as_ref()
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
    assert!(blob_store.exists(layer.digest())?);

    // Strict identity: the manifest bytes the v3 BlobStore returns must
    // be exactly the bytes that lived in the legacy OCI dir. Digest
    // equality already implies this for SHA-256, but a direct check
    // catches any future regression where import accidentally rebuilds
    // / re-serialises the manifest.
    assert_eq!(
        blob_store.read_bytes(&imported.manifest_digest)?,
        legacy_manifest_bytes
    );

    let manifest_descriptor = index_store
        .resolve_image_descriptor(&image_name)?
        .context("Imported ref descriptor is missing")?;
    assert_eq!(manifest_descriptor.media_type(), &MediaType::ImageManifest);
    assert_eq!(manifest_descriptor.digest(), &imported.manifest_digest);
    assert_eq!(
        manifest_descriptor.size(),
        legacy_manifest_bytes.len() as u64
    );
    assert_eq!(
        blob_store.read_bytes(layer.digest())?.len() as u64,
        layer.size()
    );

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
    assert_eq!(artifact.get_blob(layer.digest())?, b"instance");
    Ok(())
}

#[test]
fn imports_legacy_local_registry_explicitly() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let legacy_registry_root = dir.path().join("legacy-registry");
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:v2")?;
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

/// End-to-end v2 → v3 invariant for Docker Hub shorthand. A v2 cache
/// lives on disk at `<root>/registry-1.docker.io/alpine/__latest/`
/// with the manifest descriptor carrying
/// `org.opencontainers.image.ref.name = "registry-1.docker.io/alpine:latest"`
/// (the form ocipkg's `Display` produced). After
/// `import_legacy_local_registry`, the v3 `Artifact.load("alpine")`
/// equivalent — `LocalRegistry::resolve_image_name(parse("alpine"))` —
/// must hit, because the legacy host normalisation shim in
/// [`ImageRef::parse`] collapses every spelling of the same image onto
/// one canonical SQLite key. A regression that removed the shim would
/// silently route `load("alpine")` to a SQLite miss → network pull of
/// the real Docker Hub `alpine`, ignoring the user's pre-imported v2
/// cache.
#[test]
fn imports_legacy_docker_hub_short_name_via_normalisation_shim() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let legacy_registry_root = dir.path().join("legacy-registry");
    // Reproduce the v2 on-disk shape verbatim — ocipkg's `as_path`
    // for `alpine` was `registry-1.docker.io/alpine/__latest`, not
    // the v3 canonical `docker.io/library/alpine/__latest`.
    let legacy_dir = legacy_registry_root
        .join("registry-1.docker.io")
        .join("alpine")
        .join("__latest");
    let raw_annotation = "registry-1.docker.io/alpine:latest".to_string();
    let mut builder =
        TestOciDirBuilder::with_raw_ref_annotation(legacy_dir.clone(), Some(raw_annotation))?;
    let config = builder.add_empty_json()?;
    let (layer_digest, layer_size) = builder.add_blob(b"alpine-rootfs")?;
    let layer = DescriptorBuilder::default()
        .media_type(MediaType::Other(media_types::V1_INSTANCE_MEDIA_TYPE.into()))
        .digest(layer_digest)
        .size(layer_size)
        .build()?;
    let manifest = ImageManifestBuilder::default()
        .schema_version(2_u32)
        .artifact_type(media_types::v1_artifact())
        .config(config)
        .layers(vec![layer])
        .build()?;
    builder.finish(manifest)?;

    let registry_root = dir.path().join("registry-v3");
    let index_store = SqliteIndexStore::open_in_registry_root(&registry_root)?;
    let blob_store = FileBlobStore::open_in_registry_root(&registry_root)?;

    let report = import_legacy_local_registry(&index_store, &blob_store, &legacy_registry_root)?;
    assert_eq!(report.imported_dirs, 1, "v2 dir must import cleanly");

    // The canonical SQLite key is `(docker.io/library/alpine, latest)`;
    // every spelling of the same image must resolve to the same row.
    for spelling in [
        "alpine",
        "alpine:latest",
        "docker.io/alpine:latest",
        "docker.io/library/alpine:latest",
        "registry-1.docker.io/alpine:latest",
    ] {
        let parsed =
            ImageRef::parse(spelling).with_context(|| format!("failed to parse {spelling}"))?;
        let resolved = index_store
            .resolve_image_name(&parsed)?
            .with_context(|| format!("post-import resolve missed for spelling {spelling}"))?;
        assert!(
            resolved.as_ref().starts_with("sha256:"),
            "resolved digest must be sha256 for {spelling}, got {resolved}",
        );
    }
    Ok(())
}

#[test]
fn import_legacy_local_registry_keeps_existing_ref_on_conflict() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let legacy_registry_root = dir.path().join("legacy-registry");
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:conflict")?;
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
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:replace")?;
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
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:v3")?;
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
    assert_eq!(
        registry
            .index()
            .resolve_image_descriptor(&image_name)?
            .context("Legacy local registry descriptor was not imported")?
            .digest(),
        &imported_digest
    );
    Ok(())
}

#[test]
fn local_registry_builds_native_image_manifest_with_artifact_type() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = Arc::new(LocalRegistry::open(dir.path())?);
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:built")?;

    let artifact = build_test_local_artifact(&registry, &image_name, b"instance")?;

    let manifest_digest = registry
        .resolve_image_name(&image_name)?
        .context("Published ref is missing")?;
    assert_eq!(&manifest_digest, artifact.manifest_digest());
    let manifest_bytes = registry.blobs().read_bytes(&manifest_digest)?;
    let manifest: oci_spec::image::ImageManifest = serde_json::from_slice(&manifest_bytes)?;
    let layer = manifest
        .layers()
        .first()
        .context("Published layer is missing")?;

    let manifest_descriptor = registry
        .index()
        .resolve_image_descriptor(&image_name)?
        .context("Published manifest descriptor is missing")?;
    assert_eq!(manifest_descriptor.media_type(), &MediaType::ImageManifest);
    assert_eq!(manifest_descriptor.size(), manifest_bytes.len() as u64);
    // Manifest's own `mediaType` field is left unset to match the v2 /
    // ArchiveArtifactBuilder shape; the ref descriptor carries the
    // format for query / dispatch purposes.
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

    assert_eq!(manifest.layers().len(), 1);
    assert_eq!(manifest.layers()[0].digest(), layer.digest());
    assert_eq!(
        manifest.layers()[0].media_type(),
        &media_types::v1_instance()
    );
    assert_eq!(artifact.get_blob(layer.digest())?, b"instance");

    // Empty config blob must be readable from the registry CAS.
    assert_eq!(
        artifact.get_blob(&Digest::from_str(media_types::OCI_EMPTY_CONFIG_DIGEST)?)?,
        media_types::OCI_EMPTY_CONFIG_BYTES
    );
    assert!(registry
        .blobs()
        .exists(&Digest::from_str(media_types::OCI_EMPTY_CONFIG_DIGEST)?)?);
    Ok(())
}

#[test]
fn local_registry_build_keep_existing_skips_conflicting_manifest() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = Arc::new(LocalRegistry::open(dir.path())?);
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:keep")?;
    let first = build_test_local_artifact(&registry, &image_name, b"first")?;
    let (second, second_blob) = new_test_local_artifact_builder(image_name.clone(), b"second")?;

    let error = second
        .build_in_registry(registry.clone(), RefConflictPolicy::KeepExisting)
        .expect_err("conflicting local registry ref should fail");
    assert!(error.to_string().contains("already points to"));
    assert_eq!(
        registry.resolve_image_name(&image_name)?,
        Some(first.manifest_digest().clone())
    );
    assert!(!registry.blobs().exists(second_blob.digest())?);
    Ok(())
}

#[test]
fn concurrent_legacy_imports_are_idempotent() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let root = dir.path().to_path_buf();
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:parallel")?;
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
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:ru1")?;
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
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:rwp")?;
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
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:cache")?;

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
    let plain_image = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:plain")?;
    let plain = build_test_local_artifact(&registry, &plain_image, b"no-subject")?;
    assert_eq!(plain.subject()?, None);

    let subject_descriptor = oci_spec::image::DescriptorBuilder::default()
        .media_type(MediaType::ImageManifest)
        .digest(oci_spec::image::Digest::from_str(&sha256_digest(
            b"parent-manifest-bytes",
        ))?)
        .size(b"parent-manifest-bytes".len() as u64)
        .build()?;

    let child_image = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:child")?;
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
    // only (no config-shape requirement), and the config blob lands in
    // both the BlobStore and blob index.
    let dir = tempfile::tempdir()?;
    let legacy_dir = dir.path().join("v2-legacy");
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:v2-config")?;
    let v2_config_bytes = br#"{"description":"v2 legacy config"}"#;
    let (config_descriptor, layer_descriptor) =
        build_test_oci_dir_with_v2_config(legacy_dir.clone(), image_name.clone(), v2_config_bytes)?;

    let registry_root = dir.path().join("registry-v3");
    let index_store = SqliteIndexStore::open_in_registry_root(&registry_root)?;
    let blob_store = FileBlobStore::open_in_registry_root(&registry_root)?;
    let imported = import_oci_dir(&index_store, &blob_store, &legacy_dir)?;

    assert_eq!(imported.image_name, Some(image_name.clone()));
    assert!(blob_store.exists(&imported.manifest_digest)?);
    assert!(blob_store.exists(layer_descriptor.digest())?);

    // OMMX-specific config blob is preserved in the BlobStore.
    let config_digest = config_descriptor.digest();
    assert!(blob_store.exists(config_digest)?);
    assert_eq!(blob_store.read_bytes(config_digest)?, v2_config_bytes);

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
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:art")?;
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
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:race-publish")?;

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
        !resolved.as_ref().is_empty(),
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
            std::thread::spawn(move || -> Result<Digest> {
                let store = FileBlobStore::open(root)?;
                store.put_bytes(&bytes)
            })
        })
        .collect();

    let records: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("blob writer thread panicked"))
        .collect::<Result<_>>()?;

    let digest = Digest::from_str(&sha256_digest(&bytes))?;
    assert!(records.iter().all(|record| record == &digest));
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
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:reextract")?;

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
        msg.contains(digest_a.as_ref()),
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
    // Invariant: after a successful `import_oci_archive`, the v2-era
    // path `registry.root().join(image_name.as_path())` must not
    // exist. The archive's staging tempdir is created under the
    // registry root and dropped before the function returns; SQLite +
    // FileBlobStore are the sole post-import home.
    let dir = tempfile::tempdir()?;
    let registry = Arc::new(LocalRegistry::open(dir.path())?);
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:no-legacy-dir")?;
    let archive_path = dir.path().join("artifact.ommx");
    save_test_archive(
        &archive_path,
        image_name.clone(),
        b"step-c-payload".to_vec(),
    )?;

    let outcome = import_oci_archive(&registry, &archive_path)?;
    assert_eq!(outcome.image_name.as_ref(), Some(&image_name));

    let v2_path = crate::artifact::local_registry::import::legacy::legacy_local_registry_path(
        registry.root(),
        &image_name,
    );
    assert!(
        !v2_path.exists(),
        "legacy v2 OCI dir must not be promoted under registry root, but {} exists",
        v2_path.display(),
    );
    Ok(())
}

#[test]
fn import_oci_archive_synthesizes_anonymous_name_for_unnamed_input() -> Result<()> {
    // v2-era OMMX SDKs produced `.ommx` files whose `index.json`
    // descriptor lacks the `org.opencontainers.image.ref.name`
    // annotation. `import_oci_archive` must accept those by
    // synthesizing a `<registry-id8>.ommx.local/anonymous:<ts>-<nonce>`
    // ref on the fly rather than refusing the import. A regression
    // that re-introduced the "no ref name → bail" behaviour would
    // strand v2 user workflows on upgrade.
    use oci_spec::image::{DescriptorBuilder, Digest, ImageIndexBuilder, MediaType};
    use std::collections::HashMap;
    use std::str::FromStr;

    let dir = tempfile::tempdir()?;
    let registry = Arc::new(LocalRegistry::open(dir.path())?);

    // Build a normal named archive first, then surgically rewrite its
    // `index.json` to drop the ref.name annotation — that is the
    // shape v2 archives have.
    let named = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:unnamed-input")?;
    let archive_path = dir.path().join("unnamed.ommx");
    save_test_archive(&archive_path, named.clone(), b"unnamed-payload".to_vec())?;

    // Re-pack the archive without the ref annotation. Cheapest path:
    // extract, drop annotation, repack.
    let staging = dir.path().join("staging");
    std::fs::create_dir_all(&staging)?;
    {
        let file = std::fs::File::open(&archive_path)?;
        let mut tar = tar::Archive::new(file);
        tar.unpack(&staging)?;
    }
    let index_path = staging.join("index.json");
    let index_bytes = std::fs::read(&index_path)?;
    let index: oci_spec::image::ImageIndex = serde_json::from_slice(&index_bytes)?;
    let descriptor = index.manifests().first().unwrap();
    let stripped_descriptor = DescriptorBuilder::default()
        .media_type(MediaType::ImageManifest)
        .digest(Digest::from_str(descriptor.digest().as_ref())?)
        .size(descriptor.size())
        .annotations(HashMap::new())
        .build()?;
    let stripped_index = ImageIndexBuilder::default()
        .schema_version(2u32)
        .media_type(MediaType::ImageIndex)
        .manifests(vec![stripped_descriptor])
        .build()?;
    std::fs::write(&index_path, serde_json::to_vec(&stripped_index)?)?;

    let unnamed_archive = dir.path().join("repacked.ommx");
    {
        let file = std::fs::File::create(&unnamed_archive)?;
        let mut tar = tar::Builder::new(file);
        tar.append_dir_all(".", &staging)?;
        tar.finish()?;
    }

    // Now the test: import the unnamed archive and assert the
    // returned ref is a synthesized anonymous name (no original ref
    // annotation to preserve).
    let outcome = import_oci_archive(&registry, &unnamed_archive)?;
    let synthesized = outcome
        .image_name
        .as_ref()
        .expect("import must synthesize a name for unnamed archives");
    let synthesized_str = synthesized.to_string();
    let (repo, tag) = synthesized_str
        .rsplit_once(':')
        .expect("synthesized image name must include a tag");
    assert!(
        crate::artifact::is_anonymous_artifact_ref_name(repo),
        "synthesized repository `{repo}` must match the anonymous shape",
    );
    assert!(
        crate::artifact::is_anonymous_artifact_tag(tag),
        "synthesized tag `{tag}` must match the anonymous shape",
    );
    Ok(())
}

#[test]
fn import_oci_archive_normalizes_dot_slash_prefixed_entries() -> Result<()> {
    // `tar -cf foo.ommx -C dir .` produces entries with a leading
    // `./` — `./oci-layout`, `./index.json`, `./blobs/sha256/<hex>`.
    // Both shapes are valid OCI Image Layouts (the prefix carries no
    // semantic information), so `import_oci_archive` must accept
    // them. Build a canonical archive, repack it with every entry's
    // path re-rooted under `./`, and confirm import succeeds.
    let dir = tempfile::tempdir()?;
    let registry = Arc::new(LocalRegistry::open(dir.path())?);
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:dot-slash")?;
    let archive_path = dir.path().join("canonical.ommx");
    save_test_archive(
        &archive_path,
        image_name.clone(),
        b"dot-slash-payload".to_vec(),
    )?;

    let dot_archive = dir.path().join("dot-prefixed.ommx");
    {
        let src = std::fs::File::open(&archive_path)?;
        let mut src_tar = tar::Archive::new(src);
        let dst = std::fs::File::create(&dot_archive)?;
        let mut dst_tar = tar::Builder::new(dst);
        for entry in src_tar.entries()? {
            let mut entry = entry?;
            if !matches!(entry.header().entry_type(), tar::EntryType::Regular) {
                continue;
            }
            let original = entry.path()?.into_owned();
            let prefixed = std::path::PathBuf::from("./").join(&original);
            let mut header = entry.header().clone();
            // `set_path` rewrites the header's path-name fields; the
            // checksum is recomputed by `append_data`.
            header.set_path(&prefixed)?;
            let mut buf = Vec::with_capacity(entry.header().size().unwrap_or(0) as usize);
            entry.read_to_end(&mut buf)?;
            dst_tar.append_data(&mut header, &prefixed, std::io::Cursor::new(&buf))?;
        }
        dst_tar.finish()?;
    }

    let outcome = import_oci_archive(&registry, &dot_archive)?;
    assert_eq!(outcome.image_name.as_ref(), Some(&image_name));
    Ok(())
}

#[cfg(feature = "remote-artifact")]
#[test]
fn pull_image_short_circuits_when_ref_is_present_with_blob() -> Result<()> {
    // Fast path: `pull_image` against a ref already published in the
    // SQLite Local Registry must return `Unchanged` without touching
    // the network. Constructing the artifact via
    // `LocalArtifactBuilder` (no network) and then calling
    // `pull_image` against an unresolvable host exercises this — if
    // the short-circuit ever regresses, the call would attempt a DNS
    // lookup against a `.invalid` TLD and fail.
    let dir = tempfile::tempdir()?;
    let registry = Arc::new(LocalRegistry::open(dir.path())?);
    let image_name = ImageRef::parse("does-not-resolve.invalid/jij-inc/ommx/demo:short-circuit")?;
    let local_artifact =
        build_test_local_artifact(&registry, &image_name, b"step-c-pull-short-circuit")?;
    let expected_digest = local_artifact.manifest_digest().clone();

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
    let image_name = ImageRef::parse("does-not-resolve.invalid/jij-inc/ommx/demo:blob-missing")?;
    let local_artifact =
        build_test_local_artifact(&registry, &image_name, b"step-c-blob-corruption")?;

    // Simulate corruption: remove the manifest blob file under the
    // FileBlobStore while keeping the SQLite ref intact.
    let manifest_digest = local_artifact.manifest_digest().clone();
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
    // `LocalArtifact::save` is the CLI `save` command's only path.
    // Verify the produced archive: (a) reads back through the v3
    // native `inspect_archive`, (b) exposes the OMMX artifactType,
    // (c) preserves layer descriptors and bytes byte-for-byte,
    // (d) preserves the manifest digest byte-for-byte — `save.rs`
    // writes the SQLite manifest bytes verbatim, so the saved
    // archive's manifest digest must match the registry's.
    let dir = tempfile::tempdir()?;
    let registry = Arc::new(LocalRegistry::open(dir.path())?);
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:save-round-trip")?;
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

    // `save.rs` writes the SQLite manifest bytes verbatim. The
    // public `inspect_archive` walks the tar with the native v3
    // reader, recomputes the manifest digest from its blob payload,
    // and would fail loudly if the bytes drifted — so the digest
    // returned here is exactly the saved bytes' digest.
    let expected_manifest_digest = local_artifact.manifest_digest().clone();
    let view = crate::artifact::local_registry::inspect_archive(&archive_path)?;
    assert_eq!(
        view.manifest_digest, expected_manifest_digest,
        "save() must write manifest bytes verbatim (digest must round-trip)",
    );
    assert_eq!(
        view.manifest.artifact_type().as_ref(),
        Some(&crate::artifact::media_types::v1_artifact()),
        "saved archive must carry the OMMX artifactType",
    );
    assert_eq!(
        view.manifest.layers().len(),
        expected_layers.len(),
        "saved archive layer count differs from source",
    );

    let archive_blobs = read_archive_blobs(&archive_path)?;
    for (expected, actual) in expected_layers.iter().zip(view.manifest.layers()) {
        assert_eq!(expected.digest(), actual.digest(), "layer digest drift");
        assert_eq!(expected.size(), actual.size(), "layer size drift");
        assert_eq!(
            expected.media_type(),
            actual.media_type(),
            "layer media_type drift",
        );
        let blob = archive_blobs
            .get(&actual.digest().to_string())
            .with_context(|| format!("layer blob {} missing from archive", actual.digest()))?;
        assert_eq!(
            blob.as_slice(),
            layer_bytes,
            "saved archive layer bytes differ from source",
        );
    }
    Ok(())
}

/// Walk the tar archive once and collect every `blobs/<alg>/<encoded>`
/// entry as a `<digest> -> bytes` map. Used by the round-trip test to
/// check layer payloads without depending on the legacy ocipkg
/// `OciArtifact` reader.
fn read_archive_blobs(path: &Path) -> Result<HashMap<String, Vec<u8>>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open archive {}", path.display()))?;
    let mut archive = tar::Archive::new(std::io::BufReader::new(file));
    let mut blobs = HashMap::new();
    for entry in archive
        .entries()
        .with_context(|| format!("Failed to read tar entries in {}", path.display()))?
    {
        let mut entry =
            entry.with_context(|| format!("Failed to read tar entry in {}", path.display()))?;
        if !matches!(entry.header().entry_type(), tar::EntryType::Regular) {
            continue;
        }
        let entry_path = entry
            .path()
            .with_context(|| format!("Failed to decode tar entry path in {}", path.display()))?
            .into_owned();
        let raw = entry_path.to_string_lossy();
        let path_str = raw.strip_prefix("./").unwrap_or(&raw);
        let Some(rest) = path_str.strip_prefix("blobs/") else {
            continue;
        };
        let mut parts = rest.splitn(2, '/');
        let (Some(alg), Some(encoded)) = (parts.next(), parts.next()) else {
            continue;
        };
        let digest = format!("{alg}:{encoded}");
        let mut bytes = Vec::with_capacity(entry.header().size().unwrap_or(0) as usize);
        entry
            .read_to_end(&mut bytes)
            .with_context(|| format!("Failed to read blob {digest} from {}", path.display()))?;
        blobs.insert(digest, bytes);
    }
    Ok(blobs)
}

fn build_test_local_artifact(
    registry: &Arc<LocalRegistry>,
    image_name: &ImageRef,
    layer_bytes: &[u8],
) -> Result<LocalArtifact> {
    let (builder, _) = new_test_local_artifact_builder(image_name.clone(), layer_bytes)?;
    builder.build_in_registry(registry.clone(), RefConflictPolicy::KeepExisting)
}

fn new_test_local_artifact_builder(
    image_name: ImageRef,
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
    image_name: &ImageRef,
    bytes: &[u8],
) -> Result<Digest> {
    let descriptor = put_test_manifest(index_store, blob_store, bytes)?;
    index_store.put_image_ref(image_name, &descriptor)?;
    Ok(descriptor.digest().clone())
}

fn put_test_manifest(
    _index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    bytes: &[u8],
) -> Result<Descriptor> {
    let digest = blob_store.put_bytes(bytes)?;
    test_manifest_descriptor_with_digest(digest, bytes.len() as u64)
}

fn test_manifest_descriptor(bytes: &[u8]) -> Result<Descriptor> {
    test_manifest_descriptor_with_digest(
        Digest::from_str(&sha256_digest(bytes))?,
        bytes.len() as u64,
    )
}

fn test_manifest_descriptor_with_digest(digest: Digest, size: u64) -> Result<Descriptor> {
    DescriptorBuilder::default()
        .media_type(MediaType::ImageManifest)
        .digest(digest)
        .size(size)
        .build()
        .context("Failed to build test manifest descriptor")
}

fn build_test_oci_dir(legacy_dir: PathBuf, image_name: ImageRef) -> Result<Descriptor> {
    let mut builder = TestOciDirBuilder::new(legacy_dir, Some(image_name))?;

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
    builder.finish(manifest)?;
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
    image_name: ImageRef,
    config_bytes: &[u8],
) -> Result<(Descriptor, Descriptor)> {
    let mut builder = TestOciDirBuilder::new(legacy_dir, Some(image_name))?;
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
    builder.finish(manifest)?;
    Ok((config, layer))
}

/// In-tests reimplementation of the ocipkg `OciDirBuilder`. Writes
/// `oci-layout`, an `index.json` with the manifest descriptor (with the
/// `org.opencontainers.image.ref.name` annotation when one is supplied),
/// and one `blobs/sha256/<encoded>` file per CAS write. Used by the
/// legacy-import tests to materialise v2-shaped OCI dirs without
/// keeping a runtime ocipkg dependency.
struct TestOciDirBuilder {
    oci_dir_root: PathBuf,
    ref_name_annotation: Option<String>,
    is_finished: bool,
}

impl TestOciDirBuilder {
    fn new(oci_dir_root: PathBuf, image_name: Option<ImageRef>) -> Result<Self> {
        // Default code path: the ref name annotation is the canonical
        // Display form of the ImageRef. Tests that need to simulate a
        // v2-shaped annotation (e.g. `registry-1.docker.io/alpine:latest`,
        // which v3 canonicalises to `docker.io/library/alpine:latest`)
        // call [`Self::with_raw_ref_annotation`] instead.
        Self::with_raw_ref_annotation(oci_dir_root, image_name.map(|n| n.to_string()))
    }

    /// Construct a builder that stamps an arbitrary string as the
    /// `org.opencontainers.image.ref.name` annotation, bypassing
    /// `ImageRef`'s canonicalisation. Used to reproduce v2-era
    /// on-disk states where the annotation carries the ocipkg
    /// default hostname `registry-1.docker.io/...`.
    fn with_raw_ref_annotation(
        oci_dir_root: PathBuf,
        ref_name_annotation: Option<String>,
    ) -> Result<Self> {
        anyhow::ensure!(
            !oci_dir_root.exists(),
            "test oci-dir {} already exists",
            oci_dir_root.display()
        );
        std::fs::create_dir_all(&oci_dir_root)?;
        Ok(Self {
            oci_dir_root,
            ref_name_annotation,
            is_finished: false,
        })
    }

    fn add_blob(&mut self, data: &[u8]) -> Result<(oci_spec::image::Digest, u64)> {
        let digest_str = sha256_digest(data);
        let encoded = digest_str
            .strip_prefix("sha256:")
            .expect("sha256_digest returns sha256: prefix");
        let blobs_dir = self.oci_dir_root.join("blobs/sha256");
        std::fs::create_dir_all(&blobs_dir)?;
        std::fs::write(blobs_dir.join(encoded), data)?;
        Ok((
            oci_spec::image::Digest::from_str(&digest_str)?,
            data.len() as u64,
        ))
    }

    fn add_empty_json(&mut self) -> Result<Descriptor> {
        let (digest, size) = self.add_blob(b"{}")?;
        Ok(DescriptorBuilder::default()
            .media_type(MediaType::EmptyJSON)
            .size(size)
            .digest(digest)
            .build()?)
    }

    fn finish(&mut self, manifest: oci_spec::image::ImageManifest) -> Result<()> {
        use oci_spec::image::ImageIndexBuilder;
        let manifest_json = serde_json::to_vec(&manifest)?;
        let (digest, size) = self.add_blob(&manifest_json)?;
        let mut descriptor_builder = DescriptorBuilder::default()
            .media_type(MediaType::ImageManifest)
            .size(size)
            .digest(digest);
        if let Some(name) = &self.ref_name_annotation {
            let mut annotations = HashMap::new();
            annotations.insert(
                "org.opencontainers.image.ref.name".to_string(),
                name.clone(),
            );
            descriptor_builder = descriptor_builder.annotations(annotations);
        }
        let descriptor = descriptor_builder.build()?;
        let index = ImageIndexBuilder::default()
            .schema_version(2_u32)
            .manifests(vec![descriptor])
            .build()?;
        std::fs::write(
            self.oci_dir_root.join("oci-layout"),
            r#"{"imageLayoutVersion":"1.0.0"}"#,
        )?;
        std::fs::write(
            self.oci_dir_root.join("index.json"),
            serde_json::to_vec(&index)?,
        )?;
        self.is_finished = true;
        Ok(())
    }
}

impl Drop for TestOciDirBuilder {
    fn drop(&mut self) {
        // Match the ocipkg behaviour: half-built dirs are cleaned up
        // so a test-helper panic doesn't leave a poisoned layout
        // behind for the next assertion.
        if !self.is_finished && self.oci_dir_root.exists() {
            let _ = std::fs::remove_dir_all(&self.oci_dir_root);
        }
    }
}

/// Build a v3-shaped OCI Image Layout directory whose manifest is an
/// OCI **Artifact** Manifest (no `config`, layers in `blobs[]`). Used
/// to exercise the Artifact-Manifest dispatch in `import_oci_dir`.
fn build_test_oci_dir_with_artifact_manifest(
    oci_dir: &Path,
    image_name: &ImageRef,
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
