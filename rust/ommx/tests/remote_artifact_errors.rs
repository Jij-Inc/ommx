#![cfg(feature = "remote-artifact")]

use oci_client::errors::{DigestError, OciDistributionError};
use oci_spec::image::{DescriptorBuilder, Digest, ImageManifestBuilder, MediaType};
use ommx::artifact::{
    fetch_remote_manifest, local_registry::LocalRegistry, media_types, sha256_digest, ImageRef,
    RemoteArtifactError, OCI_IMAGE_MANIFEST_MEDIA_TYPE,
};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::str::FromStr;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

struct MockRegistry {
    image: ImageRef,
    handle: JoinHandle<()>,
}

impl MockRegistry {
    fn returning(status: u16, reason: &'static str, body: &'static str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock registry");
        listener
            .set_nonblocking(true)
            .expect("configure mock registry");
        let address = listener.local_addr().expect("mock registry address");
        let image = ImageRef::parse(&format!("{address}/test/artifact:tag"))
            .expect("parse mock image reference");
        let handle = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let request = read_request(&mut stream);
                        let path = request
                            .lines()
                            .next()
                            .and_then(|line| line.split_whitespace().nth(1))
                            .expect("HTTP request path");
                        if path == "/v2/" {
                            write_response(&mut stream, 200, "OK", "{}");
                        } else {
                            assert_eq!(path, "/v2/test/artifact/manifests/tag");
                            write_response(&mut stream, status, reason, body);
                            return;
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "timed out waiting for remote manifest request"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("mock registry accept failed: {error}"),
                }
            }
        });
        Self { image, handle }
    }

    fn serving_corrupt_config_blob() -> Self {
        const EXPECTED_BLOB: &[u8] = b"expected";
        const CORRUPT_BLOB: &[u8] = b"corrupt!";
        assert_eq!(EXPECTED_BLOB.len(), CORRUPT_BLOB.len());

        let config_digest =
            Digest::from_str(&sha256_digest(EXPECTED_BLOB)).expect("parse config digest");
        let config_digest_string = config_digest.to_string();
        let config = DescriptorBuilder::default()
            .media_type(MediaType::EmptyJSON)
            .digest(config_digest)
            .size(EXPECTED_BLOB.len() as u64)
            .build()
            .expect("build config descriptor");
        let manifest = ImageManifestBuilder::default()
            .schema_version(2_u32)
            .media_type(MediaType::ImageManifest)
            .artifact_type(media_types::v1_artifact())
            .config(config)
            .layers(Vec::new())
            .build()
            .expect("build mock manifest");
        let manifest_bytes = serde_json::to_vec(&manifest).expect("encode mock manifest");

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock registry");
        listener
            .set_nonblocking(true)
            .expect("configure mock registry");
        let address = listener.local_addr().expect("mock registry address");
        let image = ImageRef::parse(&format!("{address}/test/artifact:tag"))
            .expect("parse mock image reference");
        let blob_path = format!("/v2/test/artifact/blobs/{config_digest_string}");
        let handle = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let request = read_request(&mut stream);
                        let path = request
                            .lines()
                            .next()
                            .and_then(|line| line.split_whitespace().nth(1))
                            .expect("HTTP request path");
                        match path {
                            "/v2/" => write_response(&mut stream, 200, "OK", "{}"),
                            "/v2/test/artifact/manifests/tag" => write_bytes_response(
                                &mut stream,
                                200,
                                "OK",
                                OCI_IMAGE_MANIFEST_MEDIA_TYPE,
                                &manifest_bytes,
                            ),
                            path if path == blob_path => {
                                write_bytes_response(
                                    &mut stream,
                                    200,
                                    "OK",
                                    "application/octet-stream",
                                    CORRUPT_BLOB,
                                );
                                return;
                            }
                            other => panic!("unexpected mock registry path: {other}"),
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "timed out waiting for corrupt blob request"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("mock registry accept failed: {error}"),
                }
            }
        });
        Self { image, handle }
    }

    fn join(self) {
        self.handle.join().expect("mock registry thread");
    }
}

fn read_request(stream: &mut TcpStream) -> String {
    stream
        .set_nonblocking(false)
        .expect("configure accepted mock connection");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set request timeout");
    let mut request = Vec::new();
    let mut buffer = [0; 1024];
    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
        let read = stream.read(&mut buffer).expect("read HTTP request");
        assert_ne!(read, 0, "connection closed before request headers");
        request.extend_from_slice(&buffer[..read]);
    }
    String::from_utf8(request).expect("UTF-8 HTTP request")
}

fn write_response(stream: &mut TcpStream, status: u16, reason: &str, body: &str) {
    write_bytes_response(stream, status, reason, "application/json", body.as_bytes());
}

fn write_bytes_response(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    body: &[u8],
) {
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .expect("write HTTP response");
    stream.write_all(body).expect("write HTTP response body");
}

fn remote_artifact_error(error: &ommx::Error) -> &RemoteArtifactError {
    error
        .downcast_ref::<RemoteArtifactError>()
        .expect("remote Artifact signal must remain in the ommx::Error chain")
}

#[test]
fn fetch_remote_manifest_reports_manifest_not_found() {
    let registry = MockRegistry::returning(
        404,
        "Not Found",
        r#"{"errors":[{"code":"MANIFEST_UNKNOWN","message":"manifest unknown"}]}"#,
    );
    let error = fetch_remote_manifest(&registry.image).expect_err("manifest must be absent");
    assert!(matches!(
        remote_artifact_error(&error),
        RemoteArtifactError::ManifestNotFound { .. }
    ));
    assert_eq!(remote_artifact_error(&error).image(), &registry.image);
    assert!(error
        .chain()
        .any(|cause| cause.downcast_ref::<OciDistributionError>().is_some()));
    registry.join();
}

#[test]
fn fetch_remote_manifest_keeps_authentication_distinct_from_absence() {
    let registry = MockRegistry::returning(401, "Unauthorized", "authentication required");
    let error = fetch_remote_manifest(&registry.image).expect_err("request must be rejected");
    assert!(matches!(
        remote_artifact_error(&error),
        RemoteArtifactError::Authentication { .. }
    ));
    registry.join();
}

#[test]
fn pull_image_uses_the_same_manifest_not_found_boundary() {
    let registry = MockRegistry::returning(
        404,
        "Not Found",
        r#"{"errors":[{"code":"MANIFEST_UNKNOWN","message":"manifest unknown"}]}"#,
    );
    let root = tempfile::tempdir().expect("temporary Local Registry");
    let local = LocalRegistry::open(root.path()).expect("open Local Registry");
    let error = local
        .pull_image(&registry.image)
        .expect_err("manifest must be absent");
    assert!(matches!(
        remote_artifact_error(&error),
        RemoteArtifactError::ManifestNotFound { .. }
    ));
    registry.join();
}

#[test]
fn pull_image_classifies_corrupt_blob_stream_as_invalid_artifact() {
    let registry = MockRegistry::serving_corrupt_config_blob();
    let root = tempfile::tempdir().expect("temporary Local Registry");
    let local = LocalRegistry::open(root.path()).expect("open Local Registry");
    let error = local
        .pull_image(&registry.image)
        .expect_err("corrupt remote blob must fail digest verification");
    assert!(
        matches!(
            remote_artifact_error(&error),
            RemoteArtifactError::InvalidArtifact { .. }
        ),
        "unexpected remote error category: {error:#?}"
    );
    assert!(error.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .and_then(std::io::Error::get_ref)
            .and_then(|source| source.downcast_ref::<DigestError>())
            .is_some()
    }));
    registry.join();
}
