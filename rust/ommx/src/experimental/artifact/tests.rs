//! Tests for experimental Artifact API

use super::Artifact;
use crate::artifact::{self, Builder, Config};
use ocipkg::ImageName;
use std::{fs, path::PathBuf, sync::OnceLock};
use uuid::Uuid;

fn registry_root() -> PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let candidate = std::env::temp_dir().join(format!("ommx_test_registry_{}", Uuid::new_v4()));
        fs::create_dir_all(&candidate).unwrap();
        if artifact::set_local_registry_root(candidate.clone()).is_ok() {
            candidate
        } else {
            let root = artifact::get_local_registry_root().to_path_buf();
            fs::create_dir_all(&root).unwrap();
            root
        }
    })
    .clone()
}

fn image_name(label: &str) -> ImageName {
    ImageName::parse(&format!("example.com/test/{label}:{}", Uuid::new_v4())).unwrap()
}

fn image_dir(image_name: &ImageName) -> PathBuf {
    registry_root().join(image_name.as_path())
}

fn archive_path(image_name: &ImageName) -> PathBuf {
    image_dir(image_name).with_extension("ommx")
}

fn build_dir_artifact(image_name: &ImageName) -> PathBuf {
    let mut builder = Builder::new(image_name.clone()).unwrap();
    builder.add_config(Config {}).unwrap();
    builder.build().unwrap();
    image_dir(image_name)
}

fn build_archive_artifact(image_name: &ImageName) -> PathBuf {
    let archive = archive_path(image_name);
    if let Some(parent) = archive.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut builder = Builder::new_archive(archive.clone(), image_name.clone()).unwrap();
    builder.add_config(Config {}).unwrap();
    builder.build().unwrap();
    archive
}

fn cleanup(image_name: &ImageName) {
    let archive = archive_path(image_name);
    if archive.exists() {
        let _ = fs::remove_file(&archive);
    }
    let dir = image_dir(image_name);
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
    }
}

#[test]
fn test_create_empty_oci_dir_artifact() {
    // Create temporary directory
    let temp_dir = std::env::temp_dir().join(format!("ommx_test_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).unwrap();

    let dir_path = temp_dir.join("empty_artifact");

    // For this test, create a minimal valid OCI dir structure manually
    fs::create_dir_all(&dir_path).unwrap();
    fs::write(
        dir_path.join("oci-layout"),
        r#"{"imageLayoutVersion":"1.0.0"}"#,
    )
    .unwrap();

    // Create a minimal valid index.json with a manifest
    let index_json = r#"{
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.index.v1+json",
            "manifests": [
                {
                    "mediaType": "application/vnd.oci.image.manifest.v1+json",
                    "digest": "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a",
                    "size": 200,
                    "annotations": {
                        "org.opencontainers.image.ref.name": "example.com/test/empty-dir:v1"
                    }
                }
            ]
        }"#;
    fs::write(dir_path.join("index.json"), index_json).unwrap();

    // Create blobs directory and a minimal manifest
    let blobs_dir = dir_path.join("blobs").join("sha256");
    fs::create_dir_all(&blobs_dir).unwrap();

    let manifest_json = r#"{
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "artifactType": "application/org.ommx.v1.artifact",
            "config": {
                "mediaType": "application/vnd.oci.image.config.v1+json",
                "digest": "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a",
                "size": 2
            },
            "layers": []
        }"#;
    fs::write(
        blobs_dir.join("44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"),
        manifest_json,
    )
    .unwrap();

    // Create minimal config blob (note: this overwrites the manifest, but we need unique digests)
    // For simplicity, use different digest for config
    let config_json = r#"{"schemaVersion":2}"#;
    let config_digest = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"; // sha256 of empty string
    fs::write(blobs_dir.join(config_digest), config_json).unwrap();

    // Verify directory structure exists
    assert!(dir_path.exists(), "OCI directory should exist");
    assert!(dir_path.is_dir(), "Path should be a directory");
    assert!(
        dir_path.join("oci-layout").exists(),
        "oci-layout file should exist"
    );
    assert!(
        dir_path.join("index.json").exists(),
        "index.json should exist"
    );

    // Test loading with experimental API
    let result = Artifact::from_oci_dir(&dir_path);
    assert!(result.is_ok(), "Failed to load OCI dir: {:?}", result.err());

    let artifact = result.unwrap();
    match artifact {
        Artifact::Dir(_) => {} // Expected
        _ => panic!("Expected Dir variant"),
    }

    fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_create_empty_oci_archive_artifact() {
    // Create temporary directory
    let temp_dir = std::env::temp_dir().join(format!("ommx_test_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).unwrap();

    let image_name = ocipkg::ImageName::parse("example.com/test/empty-archive:v1").unwrap();
    let archive_path = temp_dir.join("empty_artifact.ommx");

    // Build empty artifact in OCI archive format
    let builder =
        crate::artifact::Builder::new_archive(archive_path.clone(), image_name.clone()).unwrap();
    let _artifact = builder.build().unwrap();

    // Verify archive file exists
    assert!(archive_path.exists(), "OCI archive file should exist");
    assert!(archive_path.is_file(), "Path should be a file");
    assert!(
        archive_path.extension().unwrap() == "ommx",
        "Should have .ommx extension"
    );

    // Test loading with experimental API
    let result = Artifact::from_oci_archive(&archive_path);
    assert!(
        result.is_ok(),
        "Failed to load OCI archive: {:?}",
        result.err()
    );

    let artifact = result.unwrap();
    match artifact {
        Artifact::Archive(_) => {} // Expected
        _ => panic!("Expected Archive variant"),
    }

    fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_load_prefers_archive_when_available() {
    let image_name = image_name("load-prefers-archive");
    build_dir_artifact(&image_name);
    let _archive = build_archive_artifact(&image_name);

    let artifact = Artifact::load(&image_name).unwrap();
    assert!(matches!(artifact, Artifact::Archive(_)));

    drop(artifact);
    cleanup(&image_name);
}

#[test]
fn test_load_falls_back_to_directory_when_archive_invalid() {
    let image_name = image_name("load-fallback");
    let dir = build_dir_artifact(&image_name);
    if let Some(parent) = dir.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let archive = archive_path(&image_name);
    if let Some(parent) = archive.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&archive, b"invalid archive").unwrap();

    let artifact = Artifact::load(&image_name).unwrap();
    assert!(matches!(artifact, Artifact::Dir(_)));

    drop(artifact);
    cleanup(&image_name);
}

#[test]
fn test_save_as_archive_from_dir_variant() {
    let image_name = image_name("save-as-archive");
    let dir = build_dir_artifact(&image_name);
    let archive = archive_path(&image_name);
    if archive.exists() {
        fs::remove_file(&archive).unwrap();
    }

    let mut artifact = Artifact::from_oci_dir(&dir).unwrap();
    artifact.save_as_archive(&archive).unwrap();
    assert!(archive.exists(), "archive file should be created");
    assert!(Artifact::from_oci_archive(&archive).is_ok());

    drop(artifact);
    cleanup(&image_name);
}

#[test]
fn test_save_as_archive_requires_nonexistent_path() {
    let image_name = image_name("save-as-archive-clash");
    let dir = build_dir_artifact(&image_name);
    let archive = archive_path(&image_name);
    if let Some(parent) = archive.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&archive, b"existing").unwrap();

    let mut artifact = Artifact::from_oci_dir(&dir).unwrap();
    let err = artifact.save_as_archive(&archive).unwrap_err();
    assert!(err.to_string().contains("Output file already exists"));

    drop(artifact);
    cleanup(&image_name);
}
