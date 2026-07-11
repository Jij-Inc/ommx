use super::index::SqliteIndexStore;
use super::*;
use crate::artifact::{
    media_types, stable_json_bytes, ArtifactDraft, ImageRef, LocalArtifact, LocalManifest,
    OCI_IMAGE_MANIFEST_MEDIA_TYPE,
};
use anyhow::{Context, Result};
use oci_spec::image::{Descriptor, DescriptorBuilder, Digest, ImageManifestBuilder, MediaType};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

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
    let sender_registry = LocalRegistry::open(sender_dir.path())?;
    let mut builder = ArtifactDraft::with_registry(&sender_registry, image_name);
    builder.add_layer_bytes(
        MediaType::Other(media_types::V1_INSTANCE_MEDIA_TYPE.into()),
        layer_bytes,
        HashMap::new(),
    )?;
    let local = builder.commit()?;
    local.save(archive_path)?;
    Ok(())
}

fn open_test_index(registry: &LocalRegistry) -> Result<SqliteIndexStore> {
    SqliteIndexStore::open_in_registry_root(registry.root())
}

fn remove_test_blob(registry: &LocalRegistry, digest: &Digest) -> Result<()> {
    let (algorithm, encoded) = digest
        .as_ref()
        .split_once(':')
        .context("test blob digest must contain an algorithm")?;
    fs::remove_file(registry.root().join("blobs").join(algorithm).join(encoded))?;
    Ok(())
}

fn table_columns(conn: &rusqlite::Connection, table: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT name FROM pragma_table_info(?1) ORDER BY cid")?;
    let rows = stmt.query_map([table], |row| row.get(0))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn has_table(conn: &rusqlite::Connection, table: &str) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [table],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

fn query_plan(conn: &rusqlite::Connection, sql: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |row| row.get(3))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

#[test]
fn gc_report_marks_unreachable_old_blobs_as_orphan_candidates() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:gc-root")?;
    let artifact = build_test_local_artifact(&registry, &image_name, b"reachable-layer")?;
    let reachable_layer = artifact.layers()?[0].digest().clone();
    let orphan = registry.store_layer_blob(
        MediaType::Other("application/octet-stream".to_string()),
        b"orphan-layer",
        HashMap::new(),
    )?;

    let report = registry.gc_report(&GcOptions {
        grace_period: Duration::ZERO,
        ..GcOptions::default()
    })?;

    assert!(blob_list_contains(
        &report.reachable_blobs,
        &reachable_layer
    ));
    assert!(blob_list_contains(
        &report.orphan_candidates,
        orphan.digest()
    ));
    assert!(!blob_list_contains(
        &report.reachable_blobs,
        orphan.digest()
    ));
    assert!(report.deferred_blobs.is_empty());
    assert!(report.missing_blobs.is_empty());
    assert!(report.invalid_manifests.is_empty());
    Ok(())
}

#[test]
fn gc_report_defers_recent_unreachable_blobs_within_grace_period() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let orphan = registry.store_layer_blob(
        MediaType::Other("application/octet-stream".to_string()),
        b"active-run-layer",
        HashMap::new(),
    )?;

    let report = registry.gc_report(&GcOptions {
        grace_period: Duration::from_secs(24 * 60 * 60),
        ..GcOptions::default()
    })?;

    assert!(blob_list_contains(&report.deferred_blobs, orphan.digest()));
    assert!(!blob_list_contains(
        &report.orphan_candidates,
        orphan.digest()
    ));
    Ok(())
}

#[test]
fn gc_report_walks_subject_chain_from_live_ref() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let parent_image = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:parent")?;
    let parent = build_test_local_artifact(&registry, &parent_image, b"parent-layer")?;
    let parent_manifest = parent.stored_manifest_descriptor()?;
    let parent_manifest_digest = parent.manifest_digest().clone();
    let parent_layer_digest = parent.layers()?[0].digest().clone();
    registry.remove_image_ref(&parent_image)?;

    let child_image = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:child")?;
    let mut child = ArtifactDraft::with_registry(&registry, child_image);
    child.add_layer_bytes(
        MediaType::Other(media_types::V1_INSTANCE_MEDIA_TYPE.to_string()),
        b"child-layer".to_vec(),
        HashMap::new(),
    )?;
    child.set_subject(parent_manifest.into());
    child.commit()?;

    let report = registry.gc_report(&GcOptions {
        grace_period: Duration::ZERO,
        ..GcOptions::default()
    })?;

    assert!(blob_list_contains(
        &report.reachable_blobs,
        &parent_manifest_digest
    ));
    assert!(blob_list_contains(
        &report.reachable_blobs,
        &parent_layer_digest
    ));
    assert!(!blob_list_contains(
        &report.orphan_candidates,
        &parent_manifest_digest
    ));
    assert!(!blob_list_contains(
        &report.orphan_candidates,
        &parent_layer_digest
    ));
    Ok(())
}

#[test]
fn gc_deletes_only_orphan_candidates() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:gc-delete")?;
    let artifact = build_test_local_artifact(&registry, &image_name, b"reachable-layer")?;
    let reachable_layer = artifact.layers()?[0].digest().clone();
    let orphan = registry.store_layer_blob(
        MediaType::Other("application/octet-stream".to_string()),
        b"delete-me",
        HashMap::new(),
    )?;
    let orphan_digest = orphan.digest().clone();

    let result = registry.gc(&GcOptions {
        grace_period: Duration::ZERO,
        ..GcOptions::default()
    })?;

    assert!(blob_list_contains(&result.deleted_blobs, &orphan_digest));
    assert!(!registry.contains_blob(&orphan_digest)?);
    assert!(registry.contains_blob(&reachable_layer)?);
    Ok(())
}

#[test]
fn remove_image_ref_removes_only_the_mutable_ref() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let image_name = ImageRef::parse("example.com/ommx/remove-me:latest")?;
    let artifact = build_test_local_artifact(&registry, &image_name, b"retained-layer")?;
    let manifest_digest = artifact.manifest_digest().clone();
    let layer_digest = artifact.layers()?[0].digest().clone();

    let removed = registry
        .remove_image_ref(&image_name)?
        .expect("published ref is removed");
    assert_eq!(removed.manifest_digest, manifest_digest);
    assert!(registry.remove_image_ref(&image_name)?.is_none());
    assert!(registry.resolve_image_name(&image_name)?.is_none());
    assert!(registry.contains_blob(&manifest_digest)?);
    assert!(registry.contains_blob(&layer_digest)?);

    let report = registry.gc_report(&GcOptions {
        grace_period: Duration::ZERO,
        ..GcOptions::default()
    })?;
    assert!(blob_list_contains(
        &report.orphan_candidates,
        &manifest_digest
    ));
    assert!(blob_list_contains(&report.orphan_candidates, &layer_digest));

    assert_eq!(
        registry.restore_image_ref(&image_name, &manifest_digest)?,
        RefUpdate::Inserted
    );
    assert_eq!(
        registry.resolve_image_name(&image_name)?,
        Some(manifest_digest)
    );
    Ok(())
}

#[test]
fn restore_image_ref_does_not_replace_a_new_target() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let image_name = ImageRef::parse("example.com/ommx/restore-conflict:latest")?;
    let original = build_test_local_artifact(&registry, &image_name, b"original")?;
    let original_digest = original.manifest_digest().clone();
    registry
        .remove_image_ref(&image_name)?
        .expect("original ref is removed");

    let replacement = build_test_local_artifact(&registry, &image_name, b"replacement")?;
    let replacement_digest = replacement.manifest_digest().clone();
    assert_eq!(
        registry.restore_image_ref(&image_name, &original_digest)?,
        RefUpdate::Conflicted {
            existing_manifest_digest: replacement_digest.clone(),
            incoming_manifest_digest: original_digest,
        }
    );
    assert_eq!(
        registry.resolve_image_name(&image_name)?,
        Some(replacement_digest)
    );
    Ok(())
}

#[test]
fn restore_experiment_ref_restores_listing_projection_atomically() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let image_name = ImageRef::parse("example.com/ommx/restore-experiment:latest")?;
    let experiment =
        crate::experiment::Experiment::with_registry(&registry, image_name.clone())?.commit()?;
    let manifest_digest = experiment.artifact().manifest_digest().clone();

    registry
        .remove_image_ref(&image_name)?
        .expect("Experiment ref is removed");
    assert_eq!(
        registry.restore_image_ref(&image_name, &manifest_digest)?,
        RefUpdate::Inserted
    );

    let index = open_test_index(&registry)?;
    assert!(index
        .list_missing_experiment_config_refs(Some(&image_name.to_string()))?
        .is_empty());
    let records = registry.list_experiments(Some(&image_name.to_string()))?;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].image_name, image_name);
    assert_eq!(records[0].manifest_digest, manifest_digest);
    Ok(())
}

#[test]
fn restore_image_ref_rejects_a_missing_config_blob() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let image_name = ImageRef::parse("example.com/ommx/restore-missing-config:latest")?;
    let artifact = build_test_local_artifact(&registry, &image_name, b"layer")?;
    let manifest_digest = artifact.manifest_digest().clone();
    let config_digest = artifact.stored_config()?.digest().clone();
    registry.remove_image_ref(&image_name)?;
    remove_test_blob(&registry, &config_digest)?;

    let error = registry
        .restore_image_ref(&image_name, &manifest_digest)
        .expect_err("restore must reject a missing config blob");
    assert!(format!("{error:#}").contains(config_digest.as_ref()));
    assert!(registry.resolve_image_name(&image_name)?.is_none());
    Ok(())
}

#[test]
fn restore_image_ref_rejects_a_missing_layer_blob() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let image_name = ImageRef::parse("example.com/ommx/restore-missing-layer:latest")?;
    let artifact = build_test_local_artifact(&registry, &image_name, b"layer")?;
    let manifest_digest = artifact.manifest_digest().clone();
    let layer_digest = artifact.layers()?[0].digest().clone();
    registry.remove_image_ref(&image_name)?;
    remove_test_blob(&registry, &layer_digest)?;

    let error = registry
        .restore_image_ref(&image_name, &manifest_digest)
        .expect_err("restore must reject a missing layer blob");
    assert!(format!("{error:#}").contains(layer_digest.as_ref()));
    assert!(registry.resolve_image_name(&image_name)?.is_none());
    Ok(())
}

#[test]
fn restore_image_ref_rejects_a_missing_subject_manifest() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let parent_name = ImageRef::parse("example.com/ommx/restore-subject:parent")?;
    let parent = build_test_local_artifact(&registry, &parent_name, b"parent")?;
    let parent_manifest = parent.stored_manifest_descriptor()?;
    let parent_digest = parent.manifest_digest().clone();

    let child_name = ImageRef::parse("example.com/ommx/restore-subject:child")?;
    let mut child = ArtifactDraft::with_registry(&registry, child_name.clone());
    child.add_layer_bytes(
        MediaType::Other("application/octet-stream".to_string()),
        b"child".to_vec(),
        HashMap::new(),
    )?;
    child.set_subject(parent_manifest.into());
    let child = child.commit()?;
    let child_digest = child.manifest_digest().clone();

    registry.remove_image_ref(&parent_name)?;
    registry.remove_image_ref(&child_name)?;
    remove_test_blob(&registry, &parent_digest)?;

    let error = registry
        .restore_image_ref(&child_name, &child_digest)
        .expect_err("restore must reject a missing subject manifest");
    assert!(format!("{error:#}").contains(parent_digest.as_ref()));
    assert!(registry.resolve_image_name(&child_name)?.is_none());
    Ok(())
}

#[test]
fn anonymous_ref_cleanup_can_include_experiments_and_filter_by_age() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let anonymous_artifact = ArtifactDraft::new_anonymous_in_registry(&registry)?.commit()?;
    let anonymous_artifact_name = anonymous_artifact.image_name().clone();
    let anonymous_experiment = crate::experiment::Experiment::with_registry(
        &registry,
        crate::experiment::Name::Anonymous,
    )?
    .commit()?
    .into_artifact();
    let anonymous_experiment_name = anonymous_experiment.image_name().clone();
    let named_image = ImageRef::parse("example.com/ommx/named:latest")?;
    build_test_local_artifact(&registry, &named_image, b"named")?;

    let artifacts_only = registry.list_anonymous_refs(&AnonymousRefOptions::default())?;
    assert_eq!(artifacts_only.len(), 1);
    assert_eq!(
        ImageRef::from_repository_and_reference(
            &artifacts_only[0].name,
            &artifacts_only[0].reference
        )?,
        anonymous_artifact_name
    );

    let all_anonymous = AnonymousRefOptions {
        include_experiments: true,
        older_than: None,
    };
    assert_eq!(registry.list_anonymous_refs(&all_anonymous)?.len(), 2);
    assert!(registry
        .list_anonymous_refs(&AnonymousRefOptions {
            include_experiments: true,
            older_than: Some(Duration::from_secs(365 * 24 * 60 * 60)),
        })?
        .is_empty());

    let removed = registry.prune_anonymous_refs(&AnonymousRefOptions {
        include_experiments: true,
        older_than: Some(Duration::ZERO),
    })?;
    assert_eq!(removed.len(), 2);
    assert!(registry
        .resolve_image_name(&anonymous_artifact_name)?
        .is_none());
    assert!(registry
        .resolve_image_name(&anonymous_experiment_name)?
        .is_none());
    assert!(registry.resolve_image_name(&named_image)?.is_some());
    Ok(())
}

#[test]
fn conditional_prune_does_not_delete_a_replaced_ref() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = SqliteIndexStore::open(dir.path().join(SQLITE_INDEX_FILE_NAME))?;
    let first = test_manifest_descriptor(b"first")?;
    store.replace_ref("example.com/ommx/anonymous", "latest", &first)?;
    let candidate = store.list_refs(None)?;

    let replacement = test_manifest_descriptor(b"replacement")?;
    store.replace_ref("example.com/ommx/anonymous", "latest", &replacement)?;

    assert!(store.delete_refs_if_unchanged(&candidate)?.is_empty());
    let remaining = store.list_refs(None)?;
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].manifest_digest, replacement.digest().clone());
    Ok(())
}

#[test]
fn sqlite_index_store_round_trip() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = SqliteIndexStore::open(dir.path().join(SQLITE_INDEX_FILE_NAME))?;
    assert_eq!(store.schema_version()?, 2);

    let manifest_descriptor = test_manifest_descriptor(b"manifest")?;
    store.replace_ref(
        "example.com/ommx/experiment",
        "latest",
        &manifest_descriptor,
    )?;
    let image_name = ImageRef::parse("example.com/ommx/experiment:latest")?;
    assert_eq!(
        store.resolve_image_name(&image_name)?,
        Some(manifest_descriptor.digest().clone())
    );
    let refs = store.list_refs(Some("example.com/ommx"))?;
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].reference, "latest");
    assert_eq!(
        refs[0].manifest_digest,
        manifest_descriptor.digest().clone()
    );

    let manifest_descriptor = test_manifest_descriptor(b"other-manifest")?;
    store.replace_ref(
        "example.com/foo_bar/experiment",
        "latest",
        &manifest_descriptor,
    )?;
    store.replace_ref(
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
fn sqlite_cache_prune_uses_reverse_lookup_indexes() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join(SQLITE_INDEX_FILE_NAME);
    SqliteIndexStore::open(&path)?;
    let conn = rusqlite::Connection::open(path)?;

    let manifest_plan = query_plan(
        &conn,
        r#"
        EXPLAIN QUERY PLAN
        DELETE FROM artifact_manifests
        WHERE NOT EXISTS (
            SELECT 1 FROM refs
            WHERE refs.manifest_digest = artifact_manifests.manifest_digest
        )
        "#,
    )?;
    assert!(
        manifest_plan
            .iter()
            .any(|detail| detail.contains("idx_refs_manifest_digest")),
        "unexpected query plan: {manifest_plan:?}"
    );

    let config_plan = query_plan(
        &conn,
        r#"
        EXPLAIN QUERY PLAN
        DELETE FROM experiment_configs
        WHERE NOT EXISTS (
            SELECT 1 FROM artifact_manifests
            WHERE artifact_manifests.config_digest = experiment_configs.config_digest
        )
        "#,
    )?;
    assert!(
        config_plan
            .iter()
            .any(|detail| detail.contains("idx_artifact_manifests_config_digest")),
        "unexpected query plan: {config_plan:?}"
    );
    Ok(())
}

#[test]
fn sqlite_index_migrates_v1_schema() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join(SQLITE_INDEX_FILE_NAME);
    let conn = rusqlite::Connection::open(&path)?;
    conn.pragma_update(None, "user_version", 1_i64)?;
    conn.execute_batch(
        r#"
        CREATE TABLE ommx_local_registry_metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE refs (
            name TEXT NOT NULL,
            reference TEXT NOT NULL,
            manifest_media_type TEXT NOT NULL,
            manifest_digest TEXT NOT NULL,
            manifest_size INTEGER NOT NULL CHECK(manifest_size >= 0),
            manifest_annotations_json TEXT NOT NULL DEFAULT '{}',
            updated_at TEXT NOT NULL,
            PRIMARY KEY(name, reference)
        );
        CREATE INDEX idx_refs_name ON refs(name);
        INSERT INTO ommx_local_registry_metadata (key, value)
        VALUES ('registry_id', 'existing-registry-id');
        "#,
    )?;
    let descriptor = test_manifest_descriptor(b"v1-manifest")?;
    conn.execute(
        r#"
        INSERT INTO refs (
            name,
            reference,
            manifest_media_type,
            manifest_digest,
            manifest_size,
            manifest_annotations_json,
            updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, '{}', ?6)
        "#,
        rusqlite::params![
            "example.com/ommx/experiment",
            "latest",
            descriptor.media_type().to_string(),
            descriptor.digest().to_string(),
            descriptor.size() as i64,
            "2026-07-10T00:00:00Z",
        ],
    )?;
    drop(conn);

    let store = SqliteIndexStore::open(&path)?;
    assert_eq!(store.schema_version()?, 2);
    assert_eq!(store.registry_id()?, "existing-registry-id");
    assert_eq!(
        store.resolve_image_name(&ImageRef::parse("example.com/ommx/experiment:latest")?)?,
        Some(descriptor.digest().clone())
    );
    let refs = store.list_refs(None)?;
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].updated_at, "2026-07-10T00:00:00Z");

    let conn = rusqlite::Connection::open(&path)?;
    let ref_columns = table_columns(&conn, "refs")?;
    assert_eq!(
        ref_columns,
        vec!["name", "reference", "manifest_digest", "updated_at"]
    );
    assert_eq!(
        table_columns(&conn, "experiment_configs")?,
        vec!["config_digest", "config_json"]
    );
    Ok(())
}

#[test]
fn sqlite_index_v1_migration_is_atomic() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join(SQLITE_INDEX_FILE_NAME);
    let conn = rusqlite::Connection::open(&path)?;
    conn.pragma_update(None, "user_version", 1_i64)?;
    conn.execute_batch(
        r#"
        CREATE TABLE refs (
            name TEXT NOT NULL,
            reference TEXT NOT NULL,
            manifest_digest TEXT NOT NULL,
            PRIMARY KEY(name, reference)
        );
        "#,
    )?;
    drop(conn);

    SqliteIndexStore::open(&path).expect_err("malformed v1 schema must not migrate partially");

    let conn = rusqlite::Connection::open(&path)?;
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    assert_eq!(version, 1);
    assert!(table_columns(&conn, "refs")?.contains(&"manifest_digest".to_string()));
    assert!(!has_table(&conn, "refs_legacy")?);
    assert!(!has_table(&conn, "artifact_manifests")?);
    Ok(())
}

#[test]
fn sqlite_index_normalizes_unreleased_version_2_shape() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join(SQLITE_INDEX_FILE_NAME);
    let conn = rusqlite::Connection::open(&path)?;
    conn.pragma_update(None, "user_version", 2_i64)?;
    conn.execute_batch(
        r#"
        CREATE TABLE refs (
            name TEXT NOT NULL,
            reference TEXT NOT NULL,
            manifest_media_type TEXT NOT NULL,
            manifest_digest TEXT NOT NULL,
            manifest_size INTEGER NOT NULL CHECK(manifest_size >= 0),
            updated_at TEXT NOT NULL,
            PRIMARY KEY(name, reference)
        );
        INSERT INTO refs (
            name,
            reference,
            manifest_media_type,
            manifest_digest,
            manifest_size,
            updated_at
        ) VALUES (
            'example.com/ommx/experiment',
            'latest',
            'application/vnd.oci.image.manifest.v1+json',
            'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
            0,
            '2026-07-10T00:00:00Z'
        );
        "#,
    )?;
    drop(conn);

    let store = SqliteIndexStore::open(&path)?;
    assert_eq!(store.schema_version()?, 2);
    assert_eq!(store.list_refs(None)?.len(), 1);
    let conn = rusqlite::Connection::open(&path)?;
    assert_eq!(
        table_columns(&conn, "experiment_configs")?,
        vec!["config_digest", "config_json"]
    );
    Ok(())
}

#[test]
fn sqlite_index_rejects_unknown_schema_version() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join(SQLITE_INDEX_FILE_NAME);
    let conn = rusqlite::Connection::open(&path)?;
    conn.pragma_update(None, "user_version", 3_i64)?;
    drop(conn);

    let err = SqliteIndexStore::open(&path).expect_err("unknown schema version must fail");
    assert!(
        err.to_string()
            .contains("Unsupported local registry schema version: 3"),
        "unexpected error: {err}"
    );
    Ok(())
}

#[test]
fn sqlite_experiment_ref_rejects_mismatched_projection_descriptor() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = SqliteIndexStore::open(dir.path().join(SQLITE_INDEX_FILE_NAME))?;
    let image_name = ImageRef::parse("example.com/ommx/experiment:mismatch")?;
    let ref_descriptor = test_manifest_descriptor(b"ref-manifest")?;
    let config_bytes = b"experiment-config".to_vec();
    let config_descriptor = test_manifest_descriptor(&config_bytes)?;
    let projection_manifest = ImageManifestBuilder::default()
        .schema_version(2_u32)
        .artifact_type(media_types::v1_experiment())
        .config(config_descriptor.clone())
        .layers(Vec::new())
        .build()?;
    let projection_manifest_bytes = stable_json_bytes(&projection_manifest)?;
    let projection_manifest_descriptor = test_manifest_descriptor(&projection_manifest_bytes)?;
    let artifact = ArtifactManifestRecord::from_image_manifest(
        projection_manifest_descriptor.digest().clone(),
        projection_manifest_bytes,
        &projection_manifest,
    )?;
    let experiment = ExperimentManifestRecord::from_validated_config(artifact, config_bytes)?;

    let err = store
        .publish_experiment_ref(&image_name, &ref_descriptor, &experiment)
        .expect_err("ref descriptor and Experiment projection descriptor must match");
    assert!(
        err.to_string().contains("Manifest cache digest"),
        "unexpected error: {err}"
    );
    assert!(
        store
            .resolve_ref(&image_name.repository_key(), image_name.reference())?
            .is_none(),
        "mismatched Experiment projection must not publish the ref"
    );
    Ok(())
}

#[test]
fn concurrent_keep_existing_ref_publish_keeps_one_digest() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let root = dir.path().join("registry-v3");
    let registry = LocalRegistry::open(&root)?;
    let index_store = open_test_index(&registry)?;
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:race")?;
    let first_descriptor = put_test_manifest(&registry, b"first-manifest")?;
    let second_descriptor = put_test_manifest(&registry, b"second-manifest")?;
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
                index_store.publish_image_ref(&image_name, &manifest_descriptor)
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
    let expected_digest = OciDirRef::read(&legacy_dir)?.manifest_digest;

    let registry_root = dir.path().join("registry-v3");
    let registry = LocalRegistry::open(&registry_root)?;
    let index_store = open_test_index(&registry)?;

    // Snapshot the original legacy manifest bytes so we can assert
    // byte-for-byte equality with what ends up in the v3 registry.
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

    let imported = registry.import_oci_dir(&legacy_dir)?;

    assert_eq!(imported.image_name, image_name.clone());
    assert_eq!(imported.manifest_digest, expected_digest);
    assert_eq!(
        index_store.resolve_image_name(&image_name)?,
        Some(imported.manifest_digest.clone())
    );
    assert!(registry.contains_blob(&imported.manifest_digest)?);
    assert!(registry.contains_blob(layer.digest())?);

    // Strict identity: the manifest bytes the v3 registry returns must
    // be exactly the bytes that lived in the legacy OCI dir. Digest
    // equality already implies this for SHA-256, but a direct check
    // catches any future regression where import accidentally rebuilds
    // / re-serialises the manifest.
    assert_eq!(
        registry.read_blob(&imported.manifest_digest)?,
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
        registry.read_blob(layer.digest())?.len() as u64,
        layer.size()
    );

    let artifact = LocalArtifact::open_in_registry(&registry, image_name)?;
    // LocalArtifact must dispatch on the stored manifest media type and
    // surface the legacy Image Manifest's layer descriptors through the
    // common LocalManifest view.
    assert_eq!(
        artifact.get_manifest()?.media_type(),
        OCI_IMAGE_MANIFEST_MEDIA_TYPE
    );
    assert_eq!(stored_layer_descriptors(&artifact)?, vec![layer.clone()]);
    let stored_layer = artifact.registry().stored_descriptor(layer)?;
    assert_eq!(artifact.get_blob(&stored_layer)?, b"instance");
    Ok(())
}

#[test]
fn imports_oci_dir_does_not_persist_descriptor_annotations() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let oci_dir = dir.path().join("oci-dir");
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:descriptor-annotation")?;
    let mut builder = TestOciDirBuilder::new(oci_dir.clone(), Some(image_name.clone()))?;
    builder.add_index_descriptor_annotation("org.ommx.test.descriptor", "preserved");

    let config = builder.add_empty_json()?;
    let (layer_digest, layer_size) = builder.add_blob(b"instance")?;
    let layer = DescriptorBuilder::default()
        .media_type(media_types::v1_instance())
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

    let registry_root = dir.path().join("registry");
    let registry = LocalRegistry::open(&registry_root)?;
    let index_store = open_test_index(&registry)?;
    registry.import_oci_dir(&oci_dir)?;

    let descriptor = index_store
        .resolve_image_descriptor(&image_name)?
        .context("Imported ref descriptor is missing")?;
    assert!(descriptor.annotations().is_none());
    Ok(())
}

#[test]
fn imports_legacy_local_registry_explicitly() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let legacy_registry_root = dir.path().join("legacy-registry");
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:v2")?;
    let legacy_dir = LocalRegistry::legacy_ref_path_in(&legacy_registry_root, &image_name);
    build_test_oci_dir(legacy_dir, image_name.clone())?;

    let registry = LocalRegistry::open(&legacy_registry_root)?;
    let index_store = open_test_index(&registry)?;

    assert!(index_store.resolve_image_name(&image_name)?.is_none());
    let report = registry.import_legacy_layout()?;
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
        registry.import_legacy_layout()?,
        LegacyImportReport {
            scanned_dirs: 1,
            imported_dirs: 0,
            verified_dirs: 1,
            conflicted_dirs: 0,
            replaced_refs: 0
        }
    );
    assert!(registry.contains_blob(&imported_digest)?);
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

    let registry = LocalRegistry::open(&legacy_registry_root)?;
    let index_store = open_test_index(&registry)?;

    let report = registry.import_legacy_layout()?;
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
    let legacy_dir = LocalRegistry::legacy_ref_path_in(&legacy_registry_root, &image_name);
    build_test_oci_dir(legacy_dir.clone(), image_name.clone())?;
    let legacy_manifest_digest = OciDirRef::read(&legacy_dir)?.manifest_digest;

    let registry = LocalRegistry::open(&legacy_registry_root)?;
    let index_store = open_test_index(&registry)?;
    let existing_digest = put_test_manifest_ref(&registry, &image_name, b"existing-manifest")?;
    assert_ne!(existing_digest, legacy_manifest_digest);

    let report = registry.import_legacy_layout()?;
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
    assert!(!registry.contains_blob(&legacy_manifest_digest)?);
    Ok(())
}

#[test]
fn import_legacy_local_registry_replaces_existing_ref_when_requested() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let legacy_registry_root = dir.path().join("legacy-registry");
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:replace")?;
    let legacy_dir = LocalRegistry::legacy_ref_path_in(&legacy_registry_root, &image_name);
    build_test_oci_dir(legacy_dir.clone(), image_name.clone())?;
    let legacy_manifest_digest = OciDirRef::read(&legacy_dir)?.manifest_digest;

    let registry = LocalRegistry::open(&legacy_registry_root)?;
    let index_store = open_test_index(&registry)?;
    let existing_digest = put_test_manifest_ref(&registry, &image_name, b"existing-manifest")?;
    assert_ne!(existing_digest, legacy_manifest_digest);

    let report = registry.replace_legacy_layout()?;
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
    assert!(registry.contains_blob(&legacy_manifest_digest)?);
    Ok(())
}

#[test]
fn local_registry_imports_legacy_refs_when_requested() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:v3")?;
    let legacy_dir = LocalRegistry::legacy_ref_path_in(dir.path(), &image_name);
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
    assert!(registry.contains_blob(&imported_digest)?);
    let index_store = open_test_index(&registry)?;
    assert_eq!(
        index_store
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
    let registry = LocalRegistry::open(dir.path())?;
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:built")?;

    let artifact = build_test_local_artifact(&registry, &image_name, b"instance")?;

    let manifest_digest = registry
        .resolve_image_name(&image_name)?
        .context("Published ref is missing")?;
    assert_eq!(&manifest_digest, artifact.manifest_digest());
    let manifest_bytes = registry.read_blob(&manifest_digest)?;
    let manifest: oci_spec::image::ImageManifest = serde_json::from_slice(&manifest_bytes)?;
    let layer = manifest
        .layers()
        .first()
        .context("Published layer is missing")?;

    let index_store = open_test_index(&registry)?;
    let manifest_descriptor = index_store
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
    assert_eq!(
        stored_layer_descriptors(&artifact)?,
        manifest.layers().to_vec()
    );
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
    let stored_layer = artifact.registry().stored_descriptor(layer.clone())?;
    assert_eq!(artifact.get_blob(&stored_layer)?, b"instance");

    // Empty config blob must be readable from the registry CAS.
    let config = artifact.stored_config()?;
    assert_eq!(
        config.digest().as_ref(),
        media_types::OCI_EMPTY_CONFIG_DIGEST
    );
    assert_eq!(
        artifact.get_blob(&config)?,
        media_types::OCI_EMPTY_CONFIG_BYTES
    );
    assert!(registry.contains_blob(&Digest::from_str(media_types::OCI_EMPTY_CONFIG_DIGEST)?)?);
    Ok(())
}

#[test]
fn sqlite_backfill_does_not_retain_unreferenced_cache_rows() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join(SQLITE_INDEX_FILE_NAME);
    let store = SqliteIndexStore::open(&path)?;
    let config_bytes = br#"{"status":"finished"}"#.to_vec();
    let config_descriptor = test_manifest_descriptor(&config_bytes)?;
    let manifest = ImageManifestBuilder::default()
        .schema_version(2_u32)
        .artifact_type(media_types::v1_experiment())
        .config(config_descriptor)
        .layers(Vec::new())
        .build()?;
    let manifest_bytes = stable_json_bytes(&manifest)?;
    let manifest_descriptor = test_manifest_descriptor(&manifest_bytes)?;
    let artifact = ArtifactManifestRecord::from_image_manifest(
        manifest_descriptor.digest().clone(),
        manifest_bytes,
        &manifest,
    )?;
    let experiment = ExperimentManifestRecord::from_validated_config(artifact, config_bytes)?;

    // This is the state seen when a ref is deleted while a backfill is reading
    // the CAS. The upsert transaction must re-check reachability before commit.
    store.upsert_experiment_manifest(&experiment)?;

    let conn = rusqlite::Connection::open(path)?;
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM artifact_manifests", [], |row| {
            row.get::<_, i64>(0)
        })?,
        0
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM experiment_configs", [], |row| {
            row.get::<_, i64>(0)
        })?,
        0
    );
    Ok(())
}

#[test]
fn registry_list_warning_stage_strings_are_consistently_lowercase() {
    let stages = [
        (
            RegistryListWarningStage::ManifestBackfill,
            "manifest backfill",
        ),
        (
            RegistryListWarningStage::ManifestCacheRepair,
            "manifest cache repair",
        ),
        (
            RegistryListWarningStage::ExperimentConfigBackfill,
            "experiment config backfill",
        ),
        (
            RegistryListWarningStage::ExperimentConfigCacheRepair,
            "experiment config cache repair",
        ),
        (
            RegistryListWarningStage::CheckpointProjection,
            "checkpoint projection",
        ),
    ];

    for (stage, expected) in stages {
        assert_eq!(stage.as_str(), expected);
        assert_eq!(stage.to_string(), expected);
    }
}

#[test]
fn local_registry_lists_artifacts_from_manifest_cache() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let image_name = ImageRef::parse("example.com/catalog/artifact:latest")?;
    let other_name = ImageRef::parse("example.com/other/artifact:latest")?;

    let mut draft = ArtifactDraft::with_registry(&registry, image_name.clone());
    draft.add_annotation("com.example.problem", "qap");
    draft.add_layer_bytes(
        MediaType::Other("application/json".to_string()),
        br#"{"value":1}"#.to_vec(),
        HashMap::new(),
    )?;
    let artifact = draft.commit()?;
    build_test_local_artifact(&registry, &other_name, b"other")?;

    let records = registry.list_artifacts(Some("example.com/catalog"))?;
    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.image_name(), &image_name);
    assert_eq!(record.manifest_digest(), artifact.manifest_digest());
    assert_eq!(
        record.manifest().artifact_type(),
        &Some(MediaType::Other(
            media_types::V1_ARTIFACT_MEDIA_TYPE.to_string()
        ))
    );
    assert_eq!(
        record.manifest().config().digest(),
        artifact.stored_config()?.digest()
    );
    assert_eq!(
        record
            .manifest()
            .annotations()
            .as_ref()
            .unwrap()
            .get("com.example.problem"),
        Some(&"qap".to_string())
    );
    assert_eq!(
        record.manifest().artifact_type().as_ref().unwrap().as_ref(),
        media_types::V1_ARTIFACT_MEDIA_TYPE,
    );
    assert_eq!(record.manifest().layers().len(), 1);
    assert!(record.updated_at().contains('T'));
    Ok(())
}

#[test]
fn list_artifacts_backfills_shared_missing_manifest() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let image_name = ImageRef::parse("example.com/catalog/original:latest")?;
    let alias = ImageRef::parse("example.com/catalog/alias:latest")?;
    let artifact = build_test_local_artifact(&registry, &image_name, b"shared")?;
    artifact.tag_as(alias.clone())?;

    let conn = rusqlite::Connection::open(registry.root().join(SQLITE_INDEX_FILE_NAME))?;
    conn.execute(
        "DELETE FROM artifact_manifests WHERE manifest_digest = ?1",
        [artifact.manifest_digest().to_string()],
    )?;

    let index = open_test_index(&registry)?;
    let missing = index.list_missing_artifact_manifest_refs(Some("example.com/catalog"))?;
    assert_eq!(missing.len(), 2);
    assert!(missing.iter().all(|identity| {
        matches!(
            &identity.parsed,
            Ok((_, digest)) if digest == artifact.manifest_digest()
        )
    }));

    let records = registry.list_artifacts(Some("example.com/catalog"))?;
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].image_name(), &alias);
    assert_eq!(records[1].image_name(), &image_name);
    assert!(records
        .iter()
        .all(|record| record.manifest_digest() == artifact.manifest_digest()));
    Ok(())
}

#[test]
fn artifact_ref_size_uses_manifest_cache_and_excludes_subjects() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let shared_bytes = b"shared-layer".to_vec();

    let parent_name = ImageRef::parse("example.com/catalog/size:parent")?;
    let mut parent_builder = ArtifactDraft::with_registry(&registry, parent_name);
    parent_builder.add_layer_bytes(
        MediaType::Other("application/octet-stream".to_string()),
        shared_bytes.clone(),
        HashMap::new(),
    )?;
    let parent = parent_builder.commit()?;
    let parent_descriptor =
        Descriptor::from(registry.stored_manifest_descriptor(parent.manifest_digest())?);

    let child_name = ImageRef::parse("example.com/catalog/size:child")?;
    let mut child_builder = ArtifactDraft::with_registry(&registry, child_name.clone());
    child_builder.add_layer_bytes(
        MediaType::Other("application/octet-stream".to_string()),
        shared_bytes.clone(),
        HashMap::new(),
    )?;
    child_builder.add_layer_bytes(
        MediaType::Other("application/octet-stream".to_string()),
        shared_bytes.clone(),
        HashMap::new(),
    )?;
    child_builder.set_subject(parent_descriptor);
    let child = child_builder.commit()?;
    let child_manifest_size = registry.blob_size(child.manifest_digest())?;

    // The catalog calculation must remain available from SQLite even when the
    // root Manifest CAS file cannot be read. The parent subject closure and the
    // duplicate layer descriptor are both excluded from the total.
    remove_test_blob(&registry, child.manifest_digest())?;
    let records = registry.list_artifacts(Some(&child_name.to_string()))?;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].image_name(), &child_name);
    assert_eq!(records[0].manifest_size(), child_manifest_size);
    assert_eq!(
        records[0].referenced_blob_size()?,
        child_manifest_size
            + media_types::OCI_EMPTY_CONFIG_BYTES.len() as u64
            + shared_bytes.len() as u64
    );
    Ok(())
}

#[test]
fn list_artifacts_repairs_manifest_cache_from_cas() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let image_name = ImageRef::parse("example.com/catalog/integrity:latest")?;
    let artifact = build_test_local_artifact(&registry, &image_name, b"payload")?;
    let manifest_digest = artifact.manifest_digest().clone();
    let original_manifest = registry.read_blob(&manifest_digest)?;
    let mut changed_manifest: serde_json::Value = serde_json::from_slice(&original_manifest)?;
    changed_manifest["annotations"] = serde_json::json!({ "com.example.changed": "true" });
    let conn = rusqlite::Connection::open(registry.root().join(SQLITE_INDEX_FILE_NAME))?;
    conn.execute(
        "UPDATE artifact_manifests SET manifest_json = ?1 WHERE manifest_digest = ?2",
        rusqlite::params![
            stable_json_bytes(&changed_manifest)?,
            manifest_digest.to_string()
        ],
    )?;

    let report = registry.list_artifacts_with_options(
        Some("example.com/catalog/integrity"),
        &ArtifactListOptions::default(),
    )?;
    assert_eq!(report.records.len(), 1);
    assert_eq!(report.records[0].image_name(), &image_name);
    assert_eq!(report.warnings.len(), 1);
    assert_eq!(
        report.warnings[0].stage,
        RegistryListWarningStage::ManifestCacheRepair
    );
    assert!(report.warnings[0].message.contains("repaired from CAS"));
    let repaired: Vec<u8> = conn.query_row(
        "SELECT manifest_json FROM artifact_manifests WHERE manifest_digest = ?1",
        [manifest_digest.to_string()],
        |row| row.get(0),
    )?;
    assert_eq!(repaired, original_manifest);

    conn.execute(
        "UPDATE artifact_manifests SET manifest_json = ?1 WHERE manifest_digest = ?2",
        rusqlite::params![
            stable_json_bytes(&changed_manifest)?,
            manifest_digest.to_string()
        ],
    )?;
    let error = registry
        .list_artifacts_with_options(
            Some("example.com/catalog/integrity"),
            &ArtifactListOptions {
                include_internal: false,
                strict: true,
            },
        )
        .expect_err("strict listing must reject a corrupt Manifest cache row");
    assert!(
        format!("{error:#}").contains("Cached Manifest JSON does not match"),
        "unexpected error: {error:#}"
    );
    Ok(())
}

#[test]
fn list_artifacts_propagates_manifest_cache_write_failures() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let image_name = ImageRef::parse("example.com/catalog/write-failure:latest")?;
    let artifact = build_test_local_artifact(&registry, &image_name, b"payload")?;
    let manifest_digest = artifact.manifest_digest().clone();
    let original_manifest = registry.read_blob(&manifest_digest)?;
    let mut changed_manifest: serde_json::Value = serde_json::from_slice(&original_manifest)?;
    changed_manifest["annotations"] = serde_json::json!({ "com.example.changed": "true" });
    let conn = rusqlite::Connection::open(registry.root().join(SQLITE_INDEX_FILE_NAME))?;
    conn.execute(
        "UPDATE artifact_manifests SET manifest_json = ?1 WHERE manifest_digest = ?2",
        rusqlite::params![
            stable_json_bytes(&changed_manifest)?,
            manifest_digest.to_string()
        ],
    )?;
    conn.execute_batch(
        r#"
        CREATE TRIGGER reject_manifest_cache_repair
        BEFORE UPDATE ON artifact_manifests
        BEGIN
            SELECT RAISE(ABORT, 'manifest cache writes disabled');
        END;
        "#,
    )?;

    let error = registry
        .list_artifacts_with_options(
            Some("example.com/catalog/write-failure"),
            &ArtifactListOptions::default(),
        )
        .expect_err("SQLite cache write failures must abort the listing");
    assert!(
        format!("{error:#}").contains("manifest cache writes disabled"),
        "unexpected error: {error:#}"
    );
    Ok(())
}

#[test]
fn list_artifacts_warns_and_skips_malformed_ref_identity() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let good_name = ImageRef::parse("example.com/catalog/identity-good:latest")?;
    let bad_name = ImageRef::parse("example.com/catalog/identity-bad-name:latest")?;
    let bad_digest = ImageRef::parse("example.com/catalog/identity-bad-digest:latest")?;
    let bad_type = ImageRef::parse("example.com/catalog/identity-bad-type:latest")?;
    build_test_local_artifact(&registry, &good_name, b"good")?;
    build_test_local_artifact(&registry, &bad_name, b"bad-name")?;
    build_test_local_artifact(&registry, &bad_digest, b"bad-digest")?;
    build_test_local_artifact(&registry, &bad_type, b"bad-type")?;

    let invalid_repository = "example.com/catalog/invalid name";
    let invalid_digest = "not-an-oci-digest";
    let conn = rusqlite::Connection::open(registry.root().join(SQLITE_INDEX_FILE_NAME))?;
    conn.execute(
        "UPDATE refs SET name = ?1 WHERE name = ?2 AND reference = ?3",
        rusqlite::params![
            invalid_repository,
            bad_name.repository_key(),
            bad_name.reference()
        ],
    )?;
    conn.execute(
        "UPDATE refs SET manifest_digest = ?1 WHERE name = ?2 AND reference = ?3",
        rusqlite::params![
            invalid_digest,
            bad_digest.repository_key(),
            bad_digest.reference()
        ],
    )?;
    conn.execute(
        "UPDATE refs SET manifest_digest = ?1 WHERE name = ?2 AND reference = ?3",
        rusqlite::params![
            vec![0_u8, 1_u8, 2_u8],
            bad_type.repository_key(),
            bad_type.reference()
        ],
    )?;

    let report = registry.list_artifacts_with_options(
        Some("example.com/catalog"),
        &ArtifactListOptions::default(),
    )?;
    assert_eq!(report.records.len(), 1);
    assert_eq!(report.records[0].image_name(), &good_name);
    assert_eq!(report.warnings.len(), 3);
    assert!(report.warnings.iter().any(|warning| warning.image_name
        == format!("{invalid_repository}:latest")
        && warning.message.contains("Invalid Local Registry image ref")));
    assert!(report.warnings.iter().any(|warning| {
        warning.manifest_digest == invalid_digest
            && warning
                .message
                .contains("Invalid Local Registry manifest digest")
    }));
    assert!(report.warnings.iter().any(|warning| {
        warning.image_name == bad_type.to_string()
            && warning
                .message
                .contains("Local Registry manifest digest must be TEXT, got BLOB")
    }));

    let error = registry
        .list_artifacts_with_options(
            Some("example.com/catalog"),
            &ArtifactListOptions {
                include_internal: false,
                strict: true,
            },
        )
        .expect_err("strict listing must reject malformed ref identity");
    assert!(
        format!("{error:#}").contains("Local Registry"),
        "unexpected error: {error:#}"
    );
    Ok(())
}

#[test]
fn list_artifacts_warns_and_skips_unrepairable_ref() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let good_name = ImageRef::parse("example.com/catalog/good:latest")?;
    let bad_name = ImageRef::parse("example.com/catalog/bad:latest")?;
    build_test_local_artifact(&registry, &good_name, b"good")?;
    let bad = build_test_local_artifact(&registry, &bad_name, b"bad")?;
    let bad_digest = bad.manifest_digest().clone();
    let original_manifest = registry.read_blob(&bad_digest)?;
    let mut changed_manifest: serde_json::Value = serde_json::from_slice(&original_manifest)?;
    changed_manifest["annotations"] = serde_json::json!({ "com.example.changed": "true" });
    let conn = rusqlite::Connection::open(registry.root().join(SQLITE_INDEX_FILE_NAME))?;
    conn.execute(
        "UPDATE artifact_manifests SET manifest_json = ?1 WHERE manifest_digest = ?2",
        rusqlite::params![
            stable_json_bytes(&changed_manifest)?,
            bad_digest.to_string()
        ],
    )?;
    let (algorithm, encoded) = bad_digest
        .as_ref()
        .split_once(':')
        .context("test Manifest digest must contain an algorithm")?;
    fs::remove_file(registry.root().join("blobs").join(algorithm).join(encoded))?;

    let report = registry.list_artifacts_with_options(
        Some("example.com/catalog"),
        &ArtifactListOptions::default(),
    )?;
    assert_eq!(report.records.len(), 1);
    assert_eq!(report.records[0].image_name(), &good_name);
    assert_eq!(report.warnings.len(), 1);
    assert_eq!(report.warnings[0].image_name, bad_name.to_string());
    assert!(report.warnings[0].message.contains("CAS repair failed"));

    let error = registry
        .list_artifacts_with_options(
            Some("example.com/catalog"),
            &ArtifactListOptions {
                include_internal: false,
                strict: true,
            },
        )
        .expect_err("strict listing must reject the corrupt ref");
    assert!(format!("{error:#}").contains("Cached Manifest JSON does not match"));
    Ok(())
}

#[test]
fn local_registry_build_publish_skips_conflicting_manifest() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:keep")?;
    let first = build_test_local_artifact(&registry, &image_name, b"first")?;
    let (second, second_blob) =
        new_test_local_artifact_builder(&registry, image_name.clone(), b"second")?;

    let error = second
        .commit()
        .expect_err("conflicting local registry ref should fail");
    assert!(error.to_string().contains("already points to"));
    assert_eq!(
        registry.resolve_image_name(&image_name)?,
        Some(first.manifest_digest().clone())
    );
    // The builder writes non-manifest blobs before asking the registry
    // to publish the manifest. On conflict, the ref is left unchanged,
    // but already-stored CAS bytes may remain for later GC.
    assert!(registry.contains_blob(second_blob.digest())?);
    Ok(())
}

#[test]
fn local_registry_build_replace_moves_conflicting_ref() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:replace-build")?;
    let first = build_test_local_artifact(&registry, &image_name, b"first")?;
    let (second_builder, _) =
        new_test_local_artifact_builder(&registry, image_name.clone(), b"second")?;

    let second = second_builder.commit_replace()?;
    assert_ne!(first.manifest_digest(), second.manifest_digest());
    assert_eq!(
        registry.resolve_image_name(&image_name)?,
        Some(second.manifest_digest().clone())
    );
    Ok(())
}

#[test]
fn publish_rejects_sealed_artifact_from_different_registry_instance() -> Result<()> {
    let source_dir = tempfile::tempdir()?;
    let target_dir = tempfile::tempdir()?;
    let source = LocalRegistry::open(source_dir.path())?;
    let target = LocalRegistry::open(target_dir.path())?;

    let config = DescriptorBuilder::default()
        .media_type(MediaType::EmptyJSON)
        .digest(Digest::from_str(media_types::OCI_EMPTY_CONFIG_DIGEST)?)
        .size(media_types::OCI_EMPTY_CONFIG_BYTES.len() as u64)
        .build()?;
    let config = source.store_blob(config, media_types::OCI_EMPTY_CONFIG_BYTES)?;

    let layer_bytes = b"instance";
    let layer = DescriptorBuilder::default()
        .media_type(media_types::v1_instance())
        .digest(Digest::from_str(&sha256_digest(layer_bytes))?)
        .size(layer_bytes.len() as u64)
        .build()?;
    let layer = source.store_blob(layer, layer_bytes)?;

    let artifact = UnsealedArtifact::new(
        MediaType::Other(media_types::V1_ARTIFACT_MEDIA_TYPE.to_string()),
        config,
        vec![layer],
        None,
        HashMap::new(),
    );
    let sealed = source.seal_artifact(artifact)?;
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:foreign-sealed")?;

    let err = target
        .publish_manifest_ref(&image_name, &sealed)
        .expect_err("foreign sealed artifact must not be publishable");
    assert!(err
        .to_string()
        .contains("belongs to a different Local Registry"));
    Ok(())
}

#[test]
fn concurrent_legacy_imports_are_idempotent() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let root = dir.path().to_path_buf();
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:parallel")?;
    let legacy_dir = LocalRegistry::legacy_ref_path_in(&root, &image_name);
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
    assert!(registry.contains_blob(&imported_digest)?);
    Ok(())
}

#[test]
fn import_oci_dir_surfaces_ref_update() -> Result<()> {
    // First import returns Inserted; an idempotent re-import returns
    // Unchanged. Both come back through the public OciDirImport.
    let dir = tempfile::tempdir()?;
    let legacy_dir = dir.path().join("legacy");
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:ru1")?;
    build_test_oci_dir(legacy_dir.clone(), image_name.clone())?;

    let registry_root = dir.path().join("registry-v3");
    let registry = LocalRegistry::open(&registry_root)?;

    let first = registry.import_oci_dir(&legacy_dir)?;
    assert_eq!(&first.image_name, &image_name);
    assert!(matches!(first.ref_update, RefUpdate::Inserted));

    let second = registry.import_oci_dir(&legacy_dir)?;
    assert_eq!(second.manifest_digest, first.manifest_digest);
    assert!(matches!(second.ref_update, RefUpdate::Unchanged));
    Ok(())
}

#[test]
fn local_registry_replace_legacy_ref_replaces_existing() -> Result<()> {
    // Exercises the per-image replace variant. The ref ends up at the
    // legacy digest and the outcome is Replaced { previous }.
    let dir = tempfile::tempdir()?;
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:rwp")?;
    let legacy_dir = LocalRegistry::legacy_ref_path_in(dir.path(), &image_name);
    build_test_oci_dir(legacy_dir.clone(), image_name.clone())?;
    let legacy_manifest_digest = OciDirRef::read(&legacy_dir)?.manifest_digest;

    let registry = LocalRegistry::open(dir.path())?;
    let existing_digest = put_test_manifest_ref(&registry, &image_name, b"prior-manifest")?;
    assert_ne!(existing_digest, legacy_manifest_digest);

    let import = registry.replace_legacy_ref(&image_name)?;
    assert_eq!(import.manifest_digest, legacy_manifest_digest);
    assert!(matches!(
        import.ref_update,
        RefUpdate::Replaced { ref previous_manifest_digest }
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
    let registry = LocalRegistry::open(dir.path())?;
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
    // and surfaces the Descriptor that ArtifactDraft set via
    // `set_subject`. None when no subject is set.
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
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
    let mut builder = ArtifactDraft::with_registry(&registry, child_image.clone());
    builder.add_layer_bytes(
        MediaType::Other(media_types::V1_INSTANCE_MEDIA_TYPE.to_string()),
        b"child-layer".to_vec(),
        HashMap::new(),
    )?;
    builder.set_subject(subject_descriptor.clone());
    let child = builder.commit()?;
    assert_eq!(child.subject()?, Some(subject_descriptor));
    Ok(())
}

#[test]
fn imports_legacy_v2_oci_dir_with_ommx_config_blob() -> Result<()> {
    // v2 SDK can produce Image Manifests whose `config` blob is an
    // OMMX-specific `application/org.ommx.v1.config+json` (instead of
    // the v3 draft's OCI 1.1 empty descriptor). v3 import / read must
    // preserve such manifests verbatim: parse-time check is artifactType
    // only (no config-shape requirement), and the config blob lands in
    // the registry.
    let dir = tempfile::tempdir()?;
    let legacy_dir = dir.path().join("v2-legacy");
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:v2-config")?;
    let v2_config_bytes = br#"{"description":"v2 legacy config"}"#;
    let (config_descriptor, layer_descriptor) =
        build_test_oci_dir_with_v2_config(legacy_dir.clone(), image_name.clone(), v2_config_bytes)?;

    let registry_root = dir.path().join("registry-v3");
    let registry = LocalRegistry::open(&registry_root)?;
    let imported = registry.import_oci_dir(&legacy_dir)?;

    assert_eq!(imported.image_name, image_name.clone());
    assert!(registry.contains_blob(&imported.manifest_digest)?);
    assert!(registry.contains_blob(layer_descriptor.digest())?);

    // OMMX-specific config blob is preserved in the registry.
    let config_digest = config_descriptor.digest();
    assert!(registry.contains_blob(config_digest)?);
    assert_eq!(registry.read_blob(config_digest)?, v2_config_bytes);

    // LocalArtifact reads the legacy manifest (parse-time check is on
    // artifactType only, so the OMMX-specific config is not rejected).
    let artifact = LocalArtifact::open_in_registry(&registry, image_name)?;
    assert_eq!(
        artifact.get_manifest()?.media_type(),
        OCI_IMAGE_MANIFEST_MEDIA_TYPE
    );
    assert_eq!(stored_layer_descriptors(&artifact)?, vec![layer_descriptor]);
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
    let registry = LocalRegistry::open(&registry_root)?;
    let err = registry
        .import_oci_dir(&oci_dir)
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
    // Two ArtifactDraft writers race to publish *different*
    // manifest digests under the same image_name. The atomic publish
    // operation must let exactly one writer win
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
                let registry = LocalRegistry::open(registry_root)?;
                let bytes = format!("racer-{i}");
                let (builder, _) = new_test_local_artifact_builder(
                    &registry,
                    image_name.clone(),
                    bytes.as_bytes(),
                )?;
                match builder.commit() {
                    Ok(_) => Ok(true),
                    Err(err) => {
                        // Only the conflict outcome is acceptable here.
                        assert!(
                            err.to_string().contains("already points to"),
                            "unexpected commit error: {err}"
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
    let root = dir.path().join("registry");
    let bytes = b"parallel blob".to_vec();

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let root = root.clone();
            let bytes = bytes.clone();
            std::thread::spawn(move || -> Result<Digest> {
                let registry = LocalRegistry::open(root)?;
                let digest = Digest::from_str(&sha256_digest(&bytes))?;
                let descriptor = DescriptorBuilder::default()
                    .media_type(MediaType::Other("application/octet-stream".to_string()))
                    .digest(digest)
                    .size(bytes.len() as u64)
                    .build()?;
                Ok(registry.store_blob(descriptor, &bytes)?.digest().clone())
            })
        })
        .collect();

    let records: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("blob writer thread panicked"))
        .collect::<Result<_>>()?;

    let digest = Digest::from_str(&sha256_digest(&bytes))?;
    assert!(records.iter().all(|record| record == &digest));
    let registry = LocalRegistry::open(&root)?;
    assert_eq!(registry.read_blob(&digest)?, bytes);
    Ok(())
}

#[test]
fn import_oci_archive_surfaces_digest_conflict_for_same_ref() -> Result<()> {
    // Importing a second .ommx archive that shares the first's image
    // name but carries different bytes must surface a ref conflict
    // under default publish semantics, not a stale `Unchanged`.
    // Each call to `import_oci_archive` extracts into a fresh tempdir
    // that is dropped before the function returns, so SQLite's ref
    // conflict check sees the new archive's freshly hashed manifest
    // digest rather than the prior archive's bytes.
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:reextract")?;

    let archive_path_a = dir.path().join("a.ommx");
    save_test_archive(&archive_path_a, image_name.clone(), b"archive-A".to_vec())?;
    let archive_path_b = dir.path().join("b.ommx");
    save_test_archive(&archive_path_b, image_name.clone(), b"archive-B".to_vec())?;

    let outcome_a = registry.import_oci_archive(&archive_path_a)?;
    let digest_a = outcome_a.manifest_digest.clone();

    // Stale legacy dir would silently shadow archive B's bytes. The
    // fix re-extracts B over the legacy path, so the second import sees
    // B's digest and surfaces a ref conflict against A under default
    // publish semantics.
    let err = registry
        .import_oci_archive(&archive_path_b)
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
    // registry-owned CAS files are the sole post-import home.
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:no-legacy-dir")?;
    let archive_path = dir.path().join("artifact.ommx");
    save_test_archive(
        &archive_path,
        image_name.clone(),
        b"step-c-payload".to_vec(),
    )?;

    let outcome = registry.import_oci_archive(&archive_path)?;
    assert_eq!(&outcome.image_name, &image_name);

    let v2_path = registry.legacy_ref_path(&image_name);
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
    let registry = LocalRegistry::open(dir.path())?;

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
    let outcome = registry.import_oci_archive(&unnamed_archive)?;
    let synthesized_str = outcome.image_name.to_string();
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
    let registry = LocalRegistry::open(dir.path())?;
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

    let outcome = registry.import_oci_archive(&dot_archive)?;
    assert_eq!(&outcome.image_name, &image_name);
    Ok(())
}

#[cfg(feature = "remote-artifact")]
#[test]
fn pull_image_short_circuits_when_ref_is_present_with_blob() -> Result<()> {
    // Fast path: `pull_image` against a ref already published in the
    // SQLite Local Registry must return `Unchanged` without touching
    // the network. Constructing the artifact via
    // `ArtifactDraft` (no network) and then calling
    // `pull_image` against an unresolvable host exercises this — if
    // the short-circuit ever regresses, the call would attempt a DNS
    // lookup against a `.invalid` TLD and fail.
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let image_name = ImageRef::parse("does-not-resolve.invalid/jij-inc/ommx/demo:short-circuit")?;
    let local_artifact =
        build_test_local_artifact(&registry, &image_name, b"step-c-pull-short-circuit")?;
    let expected_digest = local_artifact.manifest_digest().clone();

    let outcome = registry.pull_image(&image_name)?;
    assert_eq!(&outcome.image_name, &image_name);
    assert_eq!(outcome.manifest_digest, expected_digest);
    assert!(
        matches!(outcome.ref_update, RefUpdate::Unchanged),
        "expected RefUpdate::Unchanged on SQLite-hit short-circuit, got {:?}",
        outcome.ref_update,
    );
    Ok(())
}

#[cfg(feature = "remote-artifact")]
#[test]
fn pull_image_does_not_short_circuit_when_manifest_blob_is_missing() -> Result<()> {
    // P1 guard from Codex review: if the SQLite ref resolves but the
    // manifest blob is missing from the registry (registry
    // corruption, manual deletion, interrupted import), `pull_image`
    // must fall through to a fresh pull rather than return a stale
    // `Unchanged`. We can't run the fall-through path without a real
    // remote, so the assertion is: an unreachable remote produces an
    // error (i.e., the function did attempt the pull) rather than the
    // happy-path `Unchanged` it would return without the blob check.
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let image_name = ImageRef::parse("does-not-resolve.invalid/jij-inc/ommx/demo:blob-missing")?;

    // Simulate corruption: the SQLite ref exists, but its manifest digest
    // has no corresponding CAS blob in the Local Registry.
    let missing_manifest = b"missing manifest blob";
    let missing_descriptor = DescriptorBuilder::default()
        .media_type(MediaType::ImageManifest)
        .digest(Digest::from_str(&sha256_digest(missing_manifest))?)
        .size(missing_manifest.len() as u64)
        .build()?;
    open_test_index(&registry)?.replace_image_ref(&image_name, &missing_descriptor)?;

    let result = registry.pull_image(&image_name);
    assert!(
        result.is_err(),
        "pull_image must fall through to a remote pull when the manifest blob is \
         missing; the unreachable host should surface as Err, but got {:?}",
        result,
    );
    Ok(())
}

#[cfg(feature = "remote-artifact")]
#[test]
fn pull_image_does_not_short_circuit_when_payload_blob_is_missing() -> Result<()> {
    // The cache hit path must require the manifest payload closure, not only
    // the root manifest blob. Otherwise `Artifact.load` can return a local
    // handle that later fails when a layer/config blob is read.
    let dir = tempfile::tempdir()?;
    let registry = LocalRegistry::open(dir.path())?;
    let image_name = ImageRef::parse("does-not-resolve.invalid/jij-inc/ommx/demo:payload-missing")?;

    let config_descriptor = DescriptorBuilder::default()
        .media_type(MediaType::EmptyJSON)
        .digest(Digest::from_str(media_types::OCI_EMPTY_CONFIG_DIGEST)?)
        .size(media_types::OCI_EMPTY_CONFIG_BYTES.len() as u64)
        .build()?;
    registry.store_blob(
        config_descriptor.clone(),
        media_types::OCI_EMPTY_CONFIG_BYTES,
    )?;

    let missing_layer_bytes = b"missing cached layer";
    let missing_layer = DescriptorBuilder::default()
        .media_type(MediaType::Other(
            media_types::V1_INSTANCE_MEDIA_TYPE.to_string(),
        ))
        .digest(Digest::from_str(&sha256_digest(missing_layer_bytes))?)
        .size(missing_layer_bytes.len() as u64)
        .build()?;
    let manifest = ImageManifestBuilder::default()
        .schema_version(2_u32)
        .artifact_type(media_types::v1_artifact())
        .config(config_descriptor)
        .layers(vec![missing_layer])
        .build()?;
    let manifest_bytes = serde_json::to_vec(&manifest)?;
    let manifest_descriptor = DescriptorBuilder::default()
        .media_type(MediaType::ImageManifest)
        .digest(Digest::from_str(&sha256_digest(&manifest_bytes))?)
        .size(manifest_bytes.len() as u64)
        .build()?;
    registry.store_blob(manifest_descriptor.clone(), &manifest_bytes)?;
    open_test_index(&registry)?.replace_image_ref(&image_name, &manifest_descriptor)?;

    let result = registry.pull_image(&image_name);
    assert!(
        result.is_err(),
        "pull_image must fall through to a remote pull when a payload blob is \
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
    let registry = LocalRegistry::open(dir.path())?;
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
    let view = ArchiveInspectView::read(&archive_path)?;
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

fn build_test_local_artifact<'reg>(
    registry: &'reg LocalRegistry,
    image_name: &ImageRef,
    layer_bytes: &[u8],
) -> Result<LocalArtifact<'reg>> {
    let (builder, _) = new_test_local_artifact_builder(registry, image_name.clone(), layer_bytes)?;
    builder.commit()
}

fn new_test_local_artifact_builder<'reg>(
    registry: &'reg LocalRegistry,
    image_name: ImageRef,
    layer_bytes: &[u8],
) -> Result<(ArtifactDraft<'reg>, Descriptor)> {
    let mut builder = ArtifactDraft::with_registry(registry, image_name);
    let descriptor = builder.add_layer_bytes(
        MediaType::Other(media_types::V1_INSTANCE_MEDIA_TYPE.to_string()),
        layer_bytes.to_vec(),
        HashMap::from([(
            crate::annotation_keys::INSTANCE_TITLE.to_string(),
            "demo".to_string(),
        )]),
    )?;
    Ok((builder, descriptor.into()))
}

fn stored_layer_descriptors(artifact: &LocalArtifact<'_>) -> Result<Vec<Descriptor>> {
    Ok(artifact
        .layers()?
        .into_iter()
        .map(Descriptor::from)
        .collect())
}

fn blob_list_contains(blobs: &[GcBlob], digest: &Digest) -> bool {
    blobs.iter().any(|blob| blob.digest == *digest)
}

fn put_test_manifest_ref(
    registry: &LocalRegistry,
    image_name: &ImageRef,
    bytes: &[u8],
) -> Result<Digest> {
    let descriptor = put_test_manifest(registry, bytes)?;
    open_test_index(registry)?.replace_image_ref(image_name, &descriptor)?;
    Ok(descriptor.digest().clone())
}

fn put_test_manifest(registry: &LocalRegistry, bytes: &[u8]) -> Result<Descriptor> {
    let descriptor = test_manifest_descriptor(bytes)?;
    registry.store_blob(descriptor.clone(), bytes)?;
    Ok(descriptor)
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
    index_descriptor_annotations: HashMap<String, String>,
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
            index_descriptor_annotations: HashMap::new(),
            is_finished: false,
        })
    }

    fn add_index_descriptor_annotation(&mut self, key: &str, value: &str) {
        self.index_descriptor_annotations
            .insert(key.to_string(), value.to_string());
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
        let mut annotations = self.index_descriptor_annotations.clone();
        if let Some(name) = &self.ref_name_annotation {
            annotations.insert(
                "org.opencontainers.image.ref.name".to_string(),
                name.clone(),
            );
        }
        if !annotations.is_empty() {
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
