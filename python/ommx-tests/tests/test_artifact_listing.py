import json
import os
import sqlite3
import subprocess
import sys
import uuid

import pytest

from ommx.artifact import ArtifactDraft, list_artifacts
from ommx.experiment import Experiment, list_experiment_checkpoints


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


def test_internal_checkpoints_are_hidden_and_have_a_dedicated_listing():
    prefix = f"example.com/ommx-tests/checkpoints-{uuid.uuid4().hex}"
    draft_name = f"{prefix}/draft:latest"
    failed_name = f"{prefix}/failed:latest"

    draft = Experiment(draft_name)
    with draft.run() as run:
        run.log_parameter("seed", 1)

    with pytest.raises(ValueError, match="failed deliberately"):
        with Experiment(failed_name) as failed:
            with failed.run() as run:
                run.log_parameter("seed", 2)
            raise ValueError("failed deliberately")

    checkpoints = list_experiment_checkpoints(prefix)
    assert [checkpoint.requested_image_name for checkpoint in checkpoints] == [
        draft_name,
        failed_name,
    ]
    assert [checkpoint.status for checkpoint in checkpoints] == ["draft", "failed"]
    assert checkpoints[0].config["requested_image_name"] == draft_name

    failed_checkpoints = list_experiment_checkpoints(prefix, statuses=["failed"])
    assert [checkpoint.requested_image_name for checkpoint in failed_checkpoints] == [
        failed_name
    ]
    assert list_experiment_checkpoints(prefix, statuses=["interrupted"]) == []
    with pytest.raises(ValueError, match="Unknown Experiment checkpoint status"):
        list_experiment_checkpoints(prefix, statuses=["finished"])

    checkpoint_image_name = checkpoints[0].checkpoint_image_name
    assert list_artifacts(checkpoint_image_name) == []
    internal = list_artifacts(checkpoint_image_name, include_internal=True)
    assert [record.image_name for record in internal] == [checkpoint_image_name]


def test_list_artifacts_warns_on_repair_and_strict_rejects_corruption(tmp_path):
    root = tmp_path / "registry"
    prefix = f"example.com/ommx-tests/list-repair-{uuid.uuid4().hex}"
    image_name = f"{prefix}/artifact:latest"
    env = os.environ.copy()
    env["OMMX_LOCAL_REGISTRY_ROOT"] = str(root)
    subprocess.run(
        [
            sys.executable,
            "-c",
            (
                "from ommx.artifact import ArtifactDraft; "
                f"ArtifactDraft.new({image_name!r}).commit()"
            ),
        ],
        check=True,
        env=env,
    )

    index_path = root / "index.sqlite3"
    with sqlite3.connect(index_path) as conn:
        manifest_digest, original_manifest = conn.execute(
            "SELECT manifest_digest, manifest_json FROM artifact_manifests"
        ).fetchone()
        changed_manifest = json.loads(original_manifest)
        changed_manifest["annotations"] = {"com.example.changed": "true"}
        changed_bytes = json.dumps(changed_manifest, separators=(",", ":")).encode()
        conn.execute(
            "UPDATE artifact_manifests SET manifest_json = ? WHERE manifest_digest = ?",
            (changed_bytes, manifest_digest),
        )

    with pytest.warns(RuntimeWarning, match="repaired from CAS"):
        repaired = list_artifacts(prefix, root=root)
    assert [record.image_name for record in repaired] == [image_name]

    with sqlite3.connect(index_path) as conn:
        conn.execute(
            "UPDATE artifact_manifests SET manifest_json = ? WHERE manifest_digest = ?",
            (changed_bytes, manifest_digest),
        )
    with pytest.raises(RuntimeError, match="Cached Manifest JSON does not match"):
        list_artifacts(prefix, root=root, strict=True)
