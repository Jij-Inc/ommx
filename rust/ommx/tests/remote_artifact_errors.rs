#![cfg(feature = "remote-artifact")]

use ommx::artifact::{
    fetch_remote_manifest, local_registry::LocalRegistry, ImageRef, RemoteArtifactError,
};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
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

    fn join(self) {
        self.handle.join().expect("mock registry thread");
    }
}

fn read_request(stream: &mut TcpStream) -> String {
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
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .expect("write HTTP response");
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
        error,
        RemoteArtifactError::ManifestNotFound { .. }
    ));
    assert_eq!(error.image(), &registry.image);
    registry.join();
}

#[test]
fn fetch_remote_manifest_keeps_authentication_distinct_from_absence() {
    let registry = MockRegistry::returning(401, "Unauthorized", "authentication required");
    let error = fetch_remote_manifest(&registry.image).expect_err("request must be rejected");
    assert!(matches!(error, RemoteArtifactError::Authentication { .. }));
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
        error,
        RemoteArtifactError::ManifestNotFound { .. }
    ));
    registry.join();
}
