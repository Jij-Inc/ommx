from __future__ import annotations

import json
import os
import subprocess
import sys
from collections.abc import Callable
from typing import NoReturn

import numpy as np
import pandas as pd
import pytest

from ommx.artifact import (
    Artifact,
    ArtifactDraft,
    gc,
    get_local_registry_root,
    prune_anonymous,
    remove_image,
    restore_image,
)


INVALID_IMAGE_REF = "INVALID/IMAGE"
UNUSED_DIGEST = "sha256:" + "0" * 64


class SentinelError(Exception):
    pass


def raise_sentinel(*args: object, **kwargs: object) -> NoReturn:  # noqa: ARG001
    raise SentinelError("sentinel Python codec error")


def test_invalid_image_refs_raise_value_error(tmp_path):
    with pytest.raises(ValueError, match="Invalid image reference"):
        Artifact.load(INVALID_IMAGE_REF)

    with pytest.raises(ValueError) as error:
        ArtifactDraft.new(INVALID_IMAGE_REF)
    assert str(error.value) == (
        'Invalid image reference "INVALID/IMAGE": invalid reference format'
    )

    with pytest.raises(ValueError, match="Invalid image reference"):
        remove_image(INVALID_IMAGE_REF, root=tmp_path)

    with pytest.raises(ValueError, match="Invalid image reference"):
        restore_image(INVALID_IMAGE_REF, UNUSED_DIGEST, root=tmp_path)


def test_input_validation_precedes_registry_open(tmp_path):
    invalid_root = tmp_path / "not-a-registry-directory"
    invalid_root.write_text("registry root must be a directory")

    with pytest.raises(ValueError, match="Invalid image reference"):
        remove_image(INVALID_IMAGE_REF, root=invalid_root)

    with pytest.raises(ValueError, match="Invalid manifest digest"):
        restore_image(
            "example.com/ommx-tests/invalid-digest:latest",
            "not-a-digest",
            root=invalid_root,
        )

    with pytest.raises(ValueError, match="invalid duration suffix"):
        prune_anonymous(root=invalid_root, older_than="last-week")

    with pytest.raises(ValueError, match="invalid duration suffix"):
        gc(root=invalid_root, grace_period="last-week")


def test_corrupt_persisted_image_ref_is_runtime_error(tmp_path):
    root = tmp_path / "registry"
    env = os.environ.copy()
    env["OMMX_LOCAL_REGISTRY_ROOT"] = str(root)
    script = f"""
import sqlite3

from ommx.artifact import ArtifactDraft, get_images

ArtifactDraft.new_anonymous().commit()
with sqlite3.connect({str(root / "index.sqlite3")!r}) as connection:
    connection.execute(
        \"\"\"
        INSERT INTO refs (name, reference, manifest_digest, updated_at)
        VALUES (?, ?, ?, ?)
        \"\"\",
        (
            "ghcr.io/INVALID",
            "latest",
            {UNUSED_DIGEST!r},
            "2026-01-01T00:00:00Z",
        ),
    )

try:
    get_images()
except RuntimeError as error:
    assert "Invalid Local Registry image ref" in str(error)
else:
    raise AssertionError("get_images() accepted an invalid persisted image ref")
"""
    subprocess.run([sys.executable, "-c", script], check=True, env=env)


def test_invalid_and_unknown_digests_have_distinct_exceptions(tmp_path):
    artifact = ArtifactDraft.new_anonymous().commit()

    with pytest.raises(ValueError, match="Invalid layer digest"):
        artifact.get_layer_descriptor("not-a-digest")

    with pytest.raises(KeyError):
        artifact.get_layer_descriptor(UNUSED_DIGEST)

    with pytest.raises(ValueError, match="Invalid manifest digest"):
        restore_image(
            "example.com/ommx-tests/invalid-digest:latest",
            "not-a-digest",
            root=tmp_path,
        )


def test_missing_typed_layers_raise_value_error():
    artifact = ArtifactDraft.new_anonymous().commit()
    getters: tuple[Callable[[], object], ...] = (
        lambda: artifact.instance,
        lambda: artifact.get_instance(),
        lambda: artifact.parametric_instance,
        lambda: artifact.get_parametric_instance(),
        lambda: artifact.solution,
        lambda: artifact.get_solution(),
        lambda: artifact.sample_set,
        lambda: artifact.get_sample_set(),
    )

    for getter in getters:
        with pytest.raises(ValueError, match="layer not found"):
            getter()


def test_wrong_media_type_and_unsupported_dispatch_raise_value_error():
    draft = ArtifactDraft.new_anonymous()
    descriptor = draft.add_layer("application/octet-stream", b"payload", {})
    artifact = draft.commit()
    getters: tuple[Callable[[], object], ...] = (
        lambda: artifact.get_instance(descriptor),
        lambda: artifact.get_parametric_instance(descriptor),
        lambda: artifact.get_solution(descriptor),
        lambda: artifact.get_sample_set(descriptor),
        lambda: artifact.get_ndarray(descriptor),
        lambda: artifact.get_dataframe(descriptor),
        lambda: artifact.get_json(descriptor),
        lambda: artifact.get_layer(descriptor),
    )

    for getter in getters:
        with pytest.raises(ValueError):
            getter()


def test_malformed_ommx_payload_raises_value_error():
    draft = ArtifactDraft.new_anonymous()
    descriptor = draft.add_layer(
        "application/org.ommx.v2.instance",
        b"not a protobuf payload",
        {},
    )
    artifact = draft.commit()

    with pytest.raises(ValueError):
        artifact.get_instance(descriptor)
    with pytest.raises(ValueError):
        artifact.get_layer(descriptor)


def test_committing_a_draft_twice_raises_runtime_error():
    draft = ArtifactDraft.new_anonymous()
    draft.commit()

    with pytest.raises(RuntimeError, match="Already committed artifact"):
        draft.commit()


def test_missing_cas_blob_raises_runtime_error_and_is_restored():
    draft = ArtifactDraft.new_anonymous()
    descriptor = draft.add_json({"value": 1})
    artifact = draft.commit()
    algorithm, encoded = descriptor.digest.split(":", maxsplit=1)
    blob_path = get_local_registry_root() / "blobs" / algorithm / encoded
    original_blob = blob_path.read_bytes()

    blob_path.unlink()
    try:
        with pytest.raises(RuntimeError, match="Failed to read blob"):
            artifact.get_blob(descriptor)
        with pytest.raises(RuntimeError, match="Failed to read blob"):
            artifact.get_json(descriptor)
    finally:
        blob_path.write_bytes(original_blob)


def test_python_serialization_exceptions_are_preserved(monkeypatch: pytest.MonkeyPatch):
    draft = ArtifactDraft.new_anonymous()

    with monkeypatch.context() as patch:
        patch.setattr(json, "dumps", raise_sentinel)
        with pytest.raises(SentinelError, match="sentinel Python codec error"):
            draft.add_json({"value": 1})

    with monkeypatch.context() as patch:
        patch.setattr(np, "save", raise_sentinel)
        with pytest.raises(SentinelError, match="sentinel Python codec error"):
            draft.add_ndarray(np.array([1]))

    with monkeypatch.context() as patch:
        patch.setattr(pd.DataFrame, "to_parquet", raise_sentinel)
        with pytest.raises(SentinelError, match="sentinel Python codec error"):
            draft.add_dataframe(pd.DataFrame({"value": [1]}))


def test_python_deserialization_exceptions_are_preserved(
    monkeypatch: pytest.MonkeyPatch,
):
    draft = ArtifactDraft.new_anonymous()
    json_descriptor = draft.add_json({"value": 1})
    ndarray_descriptor = draft.add_ndarray(np.array([1]))
    dataframe_descriptor = draft.add_layer(
        "application/vnd.apache.parquet",
        b"not read because pandas is patched",
        {},
    )
    artifact = draft.commit()

    with monkeypatch.context() as patch:
        patch.setattr(json, "loads", raise_sentinel)
        with pytest.raises(SentinelError, match="sentinel Python codec error"):
            artifact.get_json(json_descriptor)

    with monkeypatch.context() as patch:
        patch.setattr(np, "load", raise_sentinel)
        with pytest.raises(SentinelError, match="sentinel Python codec error"):
            artifact.get_ndarray(ndarray_descriptor)

    with monkeypatch.context() as patch:
        patch.setattr(pd, "read_parquet", raise_sentinel)
        with pytest.raises(SentinelError, match="sentinel Python codec error"):
            artifact.get_dataframe(dataframe_descriptor)
