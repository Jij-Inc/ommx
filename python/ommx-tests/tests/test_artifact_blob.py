from typing import Any, cast

import pytest

from ommx.artifact import (
    ArtifactDraft,
    set_local_registry_root,
)


@pytest.fixture(scope="module", autouse=True)
def isolated_local_registry(tmp_path_factory):
    set_local_registry_root(tmp_path_factory.mktemp("artifact-blob") / "registry")


def test_get_blob_accepts_descriptor(isolated_local_registry):
    builder = ArtifactDraft.new_anonymous()
    descriptor = builder.add_layer("application/octet-stream", b"hello", {})
    artifact = builder.commit()

    assert artifact.get_blob(descriptor) == b"hello"


def test_get_blob_rejects_digest_string(isolated_local_registry):
    artifact = ArtifactDraft.new_anonymous().commit()
    invalid_descriptor = cast(Any, "sha256:../../outside")

    with pytest.raises(TypeError):
        artifact.get_blob(invalid_descriptor)
