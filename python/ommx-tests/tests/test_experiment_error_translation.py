from __future__ import annotations

import json
from collections.abc import Callable
from typing import NoReturn

import pytest

from ommx.experiment import Experiment, list_experiment_checkpoints


INVALID_IMAGE_REF = "INVALID/EXPERIMENT"


class SentinelError(Exception):
    pass


def raise_sentinel(*args: object, **kwargs: object) -> NoReturn:  # noqa: ARG001
    raise SentinelError("sentinel Python callback error")


def test_invalid_image_refs_raise_value_error():
    experiment = Experiment.with_temp_local_registry()
    experiment.commit()

    operations: tuple[Callable[[], object], ...] = (
        lambda: Experiment(INVALID_IMAGE_REF),
        lambda: Experiment.with_temp_local_registry(INVALID_IMAGE_REF),
        lambda: Experiment.load(INVALID_IMAGE_REF),
        lambda: Experiment.restore_from_checkpoint(INVALID_IMAGE_REF),
        lambda: experiment.rename(INVALID_IMAGE_REF),
        lambda: experiment.fork(INVALID_IMAGE_REF),
    )

    for operation in operations:
        with pytest.raises(ValueError, match="Invalid image reference"):
            operation()


def test_checkpoint_status_validation_precedes_registry_open(tmp_path):
    invalid_root = tmp_path / "not-a-registry-directory"
    invalid_root.write_text("registry root must be a directory")

    with pytest.raises(ValueError, match="Unknown Experiment checkpoint status"):
        list_experiment_checkpoints(statuses=["finished"], root=invalid_root)


def test_missing_experiment_attachments_raise_key_error(tmp_path):
    with Experiment.with_temp_local_registry() as experiment:
        with experiment.run():
            pass

    loaded = Experiment.from_artifact(experiment.artifact)
    run = loaded.runs[0]
    operations: tuple[Callable[[], object], ...] = (
        lambda: loaded.attachment_media_type("missing"),
        lambda: loaded.get_attachment("missing"),
        lambda: loaded.get_json("missing"),
        lambda: loaded.get_instance("missing"),
        lambda: loaded.get_parametric_instance("missing"),
        lambda: loaded.get_solution("missing"),
        lambda: loaded.get_sample_set("missing"),
        lambda: loaded.get_blob("missing"),
        lambda: loaded.write_attachment("missing", tmp_path / "experiment.bin"),
        lambda: run.attachment_media_type("missing"),
        lambda: run.get_attachment("missing"),
        lambda: run.get_json("missing"),
        lambda: run.get_instance("missing"),
        lambda: run.get_parametric_instance("missing"),
        lambda: run.get_solution("missing"),
        lambda: run.get_sample_set("missing"),
        lambda: run.get_blob("missing"),
        lambda: run.write_attachment("missing", tmp_path / "run.bin"),
    )

    for operation in operations:
        with pytest.raises(KeyError, match="missing"):
            operation()


def test_json_callback_exceptions_are_preserved(monkeypatch: pytest.MonkeyPatch):
    experiment = Experiment.with_temp_local_registry()
    run = experiment.run()

    with monkeypatch.context() as patch:
        patch.setattr(json, "dumps", raise_sentinel)
        with pytest.raises(SentinelError, match="sentinel Python callback error"):
            experiment.log_json("experiment", {"value": 1})
        with pytest.raises(SentinelError, match="sentinel Python callback error"):
            run.log_json("run", {"value": 1})


def test_json_input_validation_raises_value_error():
    experiment = Experiment.with_temp_local_registry()
    run = experiment.run()

    with pytest.raises(ValueError, match="JSON-serializable"):
        experiment.log_json("experiment", object())
    with pytest.raises(ValueError, match="JSON-serializable"):
        run.log_json("run", object())


def test_json_decode_exceptions_are_preserved(monkeypatch: pytest.MonkeyPatch):
    with Experiment.with_temp_local_registry() as experiment:
        experiment.log_json("experiment", {"value": 1})
        with experiment.run() as run:
            run.log_json("run", {"value": 2})

    loaded = Experiment.from_artifact(experiment.artifact)
    loaded_run = loaded.runs[0]
    with monkeypatch.context() as patch:
        patch.setattr(json, "loads", raise_sentinel)
        with pytest.raises(SentinelError, match="sentinel Python callback error"):
            loaded.get_json("experiment")
        with pytest.raises(SentinelError, match="sentinel Python callback error"):
            loaded_run.get_json("run")
