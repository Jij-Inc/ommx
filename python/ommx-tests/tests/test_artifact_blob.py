from typing import Any, cast

import pytest

from ommx.artifact import ArtifactDraft


def test_get_blob_accepts_descriptor():
    builder = ArtifactDraft.new_anonymous()
    descriptor = builder.add_layer("application/octet-stream", b"hello", {})
    artifact = builder.commit()

    assert artifact.get_blob(descriptor) == b"hello"


def test_get_blob_rejects_digest_string():
    artifact = ArtifactDraft.new_anonymous().commit()
    invalid_descriptor = cast(Any, "sha256:../../outside")

    with pytest.raises(TypeError):
        artifact.get_blob(invalid_descriptor)


def test_high_level_layer_annotations_reject_reserved_namespace():
    builder = ArtifactDraft.new_anonymous()

    with pytest.raises(Exception, match="reserved for OMMX metadata"):
        builder.add_json(
            {"value": 1},
            annotation_namespace="org.ommx.v1.instance",
            title="invalid",
        )
