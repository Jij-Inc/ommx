import pytest

from ommx.artifact import (
    ArtifactBuilder,
    set_local_registry_root,
)


@pytest.fixture(scope="module", autouse=True)
def isolated_local_registry(tmp_path_factory):
    set_local_registry_root(tmp_path_factory.mktemp("artifact-blob") / "registry")


def test_get_blob_accepts_descriptor_and_digest_string(isolated_local_registry):
    builder = ArtifactBuilder.new_anonymous()
    descriptor = builder.add_layer("application/octet-stream", b"hello", {})
    artifact = builder.build()

    assert artifact.get_blob(descriptor) == b"hello"
    assert artifact.get_blob(descriptor.digest) == b"hello"


def test_get_blob_rejects_invalid_digest_string(isolated_local_registry):
    artifact = ArtifactBuilder.new_anonymous().build()

    with pytest.raises(ValueError, match="Invalid digest"):
        artifact.get_blob("sha256:../../outside")
