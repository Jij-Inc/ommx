import subprocess
import sys

import pytest

from ommx.artifact import (
    Artifact,
    InvalidRemoteArtifactError,
    RemoteArtifactAuthenticationError,
    RemoteArtifactAuthorizationError,
    RemoteArtifactError,
    RemoteArtifactNotFoundError,
    RemoteArtifactTransportError,
)


_MOCK_REGISTRY = r"""
import socket
import sys

status = int(sys.argv[1])
body = sys.argv[2].encode()

server = socket.socket()
server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
server.bind(("127.0.0.1", 0))
server.listen()
print(server.getsockname()[1], flush=True)

while True:
    connection, _ = server.accept()
    with connection:
        request = b""
        while b"\r\n\r\n" not in request:
            chunk = connection.recv(1024)
            if not chunk:
                break
            request += chunk
        path = request.split(b" ", 2)[1]
        if path == b"/v2/":
            response_status = 200
            reason = "OK"
            response_body = b"{}"
        else:
            response_status = status
            reason = "Error"
            response_body = body
        response = (
            f"HTTP/1.1 {response_status} {reason}\r\n"
            f"Content-Type: application/json\r\n"
            f"Content-Length: {len(response_body)}\r\n"
            "Connection: close\r\n\r\n"
        ).encode() + response_body
        connection.sendall(response)
        if path != b"/v2/":
            break
"""


def assert_load_raises(status: int, body: str, exception: type[Exception]):
    server = subprocess.Popen(
        [sys.executable, "-c", _MOCK_REGISTRY, str(status), body],
        stdout=subprocess.PIPE,
        text=True,
    )
    assert server.stdout is not None
    port = int(server.stdout.readline())
    try:
        with pytest.raises(exception):
            Artifact.load(f"127.0.0.1:{port}/test/artifact:tag")
    finally:
        server.wait(timeout=10)


def test_remote_artifact_exception_hierarchy():
    assert issubclass(RemoteArtifactError, RuntimeError)
    assert issubclass(RemoteArtifactNotFoundError, RemoteArtifactError)
    assert issubclass(RemoteArtifactAuthenticationError, RemoteArtifactError)
    assert issubclass(RemoteArtifactAuthorizationError, RemoteArtifactError)
    assert issubclass(RemoteArtifactTransportError, RemoteArtifactError)
    assert issubclass(InvalidRemoteArtifactError, RemoteArtifactError)


def test_artifact_load_reports_manifest_not_found():
    assert_load_raises(
        404,
        '{"errors":[{"code":"MANIFEST_UNKNOWN","message":"manifest unknown"}]}',
        RemoteArtifactNotFoundError,
    )


def test_artifact_load_keeps_authentication_distinct_from_absence():
    assert_load_raises(
        401,
        "authentication required",
        RemoteArtifactAuthenticationError,
    )
