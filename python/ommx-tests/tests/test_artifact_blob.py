from typing import Any, cast

import pytest

from ommx import Instance, ParametricInstance, SampleSet, Solution
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

    with pytest.raises(ValueError, match="reserved for OMMX metadata"):
        builder.add_json(
            {"value": 1},
            annotation_namespace="org.ommx.v1.instance",
            title="invalid",
        )


def test_artifact_reads_v2_instance_layer():
    builder = ArtifactDraft.new_anonymous()
    descriptor = builder.add_instance(Instance.empty())
    artifact = builder.commit()

    assert descriptor.media_type == "application/org.ommx.v2.instance"
    assert isinstance(artifact.get_instance(), Instance)
    assert isinstance(artifact.get_layer(descriptor), Instance)


def test_artifact_reads_v2_parametric_instance_layer():
    builder = ArtifactDraft.new_anonymous()
    descriptor = builder.add_parametric_instance(ParametricInstance.empty())
    artifact = builder.commit()

    assert descriptor.media_type == "application/org.ommx.v2.parametric-instance"
    assert isinstance(artifact.get_parametric_instance(), ParametricInstance)
    assert isinstance(artifact.get_layer(descriptor), ParametricInstance)


def test_artifact_reads_v2_solution_layer():
    solution = Instance.empty().evaluate({})
    builder = ArtifactDraft.new_anonymous()
    descriptor = builder.add_solution(solution)
    artifact = builder.commit()

    assert descriptor.media_type == "application/org.ommx.v2.solution"
    assert isinstance(artifact.get_solution(), Solution)
    assert isinstance(artifact.get_layer(descriptor), Solution)


def test_artifact_reads_v2_sample_set_layer():
    sample_set = Instance.empty().evaluate_samples([{}])
    builder = ArtifactDraft.new_anonymous()
    descriptor = builder.add_sample_set(sample_set)
    artifact = builder.commit()

    assert descriptor.media_type == "application/org.ommx.v2.sample-set"
    assert isinstance(artifact.get_sample_set(), SampleSet)
    assert isinstance(artifact.get_layer(descriptor), SampleSet)
