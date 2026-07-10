import uuid

from ommx.artifact import ArtifactDraft, list_artifacts
from ommx.experiment import Experiment


def test_list_artifacts_returns_cached_manifest_records(tmp_path):
    prefix = f"example.com/ommx-tests/list-artifacts-{uuid.uuid4().hex}"
    artifact_name = f"{prefix}/artifact:latest"
    experiment_name = f"{prefix}/experiment:latest"

    draft = ArtifactDraft.new(artifact_name)
    layer = draft.add_layer("application/json", b'{"value": 1}', {})
    draft.commit()

    with Experiment(experiment_name) as experiment:
        experiment.set_annotation("com.example.problem", "qap")

    records = list_artifacts(prefix)
    assert [record.image_name for record in records] == [
        artifact_name,
        experiment_name,
    ]

    artifact = records[0]
    assert artifact.artifact_type == "application/org.ommx.v1.artifact"
    assert artifact.manifest["artifactType"] == artifact.artifact_type
    assert artifact.manifest["config"]["digest"] == artifact.config_digest
    assert artifact.manifest["layers"][0]["digest"] == layer.digest
    assert artifact.annotations == {}
    assert artifact.manifest_digest.startswith("sha256:")
    assert "T" in artifact.updated_at

    experiment = records[1]
    assert experiment.artifact_type == "application/org.ommx.v1.experiment"
    assert experiment.annotations["com.example.problem"] == "qap"
    assert experiment.manifest["annotations"]["com.example.problem"] == "qap"

    assert [
        record.image_name for record in list_artifacts(f"{prefix}/artifact:lat")
    ] == [artifact_name]
    assert list_artifacts(prefix, root=tmp_path / "empty-registry") == []
