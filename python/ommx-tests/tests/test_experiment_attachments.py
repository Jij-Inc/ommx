import json
from dataclasses import dataclass

import pytest

from ommx.artifact import Descriptor
from ommx.experiment import Experiment

_ATTACHMENT_NAME = "org.ommx.attachment.name"
_ATTACHMENT_FILENAME = "org.ommx.attachment.filename"


@dataclass(frozen=True)
class ToyPayload:
    label: str
    value: int


class ToyPayloadCodec:
    media_type = "application/vnd.ommx-tests.toy-payload+json"

    @staticmethod
    def encode(value: ToyPayload) -> bytes:
        payload = {"label": value.label, "value": value.value}
        return json.dumps(payload, sort_keys=True, separators=(",", ":")).encode()

    @staticmethod
    def decode(data: bytes) -> ToyPayload:
        payload = json.loads(data.decode())
        return ToyPayload(label=payload["label"], value=payload["value"])


class WrongMediaTypeCodec:
    media_type = "application/vnd.ommx-tests.other-payload+json"

    @staticmethod
    def encode(value: ToyPayload) -> bytes:
        return ToyPayloadCodec.encode(value)

    @staticmethod
    def decode(data: bytes) -> ToyPayload:
        return ToyPayloadCodec.decode(data)


def _attachment_by_name(attachments: list[Descriptor], name: str) -> Descriptor:
    for attachment in attachments:
        if attachment.annotations[_ATTACHMENT_NAME] == name:
            return attachment
    raise AssertionError(f"attachment {name!r} not found")


def test_experiment_attachment_codec_round_trip():
    expected = ToyPayload(label="experiment", value=7)

    with Experiment.with_temp_local_registry() as experiment:
        experiment.log_with_codec(ToyPayloadCodec, "typed-payload", expected)

    loaded = Experiment.from_artifact(experiment.artifact)
    assert loaded.get_with_codec(ToyPayloadCodec, "typed-payload") == expected

    descriptor = _attachment_by_name(loaded.experiment_attachments, "typed-payload")
    assert descriptor.media_type == ToyPayloadCodec.media_type
    assert loaded.get_blob("typed-payload") == ToyPayloadCodec.encode(expected)


def test_run_attachment_codec_round_trip():
    expected = ToyPayload(label="run", value=11)

    with Experiment.with_temp_local_registry() as experiment:
        with experiment.run() as run:
            run.log_with_codec(ToyPayloadCodec, "typed-payload", expected)

    loaded = Experiment.from_artifact(experiment.artifact)
    run = loaded.runs[0]
    assert run.get_with_codec(ToyPayloadCodec, "typed-payload") == expected

    descriptor = _attachment_by_name(run.attachments, "typed-payload")
    assert descriptor.media_type == ToyPayloadCodec.media_type
    assert run.get_blob("typed-payload") == ToyPayloadCodec.encode(expected)


def test_experiment_attachment_codec_rejects_media_type_mismatch():
    with Experiment.with_temp_local_registry() as experiment:
        experiment.log_with_codec(
            ToyPayloadCodec, "typed-payload", ToyPayload(label="experiment", value=7)
        )

    loaded = Experiment.from_artifact(experiment.artifact)
    with pytest.raises(Exception, match="Expected media type"):
        loaded.get_with_codec(WrongMediaTypeCodec, "typed-payload")


def test_run_attachment_codec_rejects_media_type_mismatch():
    with Experiment.with_temp_local_registry() as experiment:
        with experiment.run() as run:
            run.log_with_codec(
                ToyPayloadCodec, "typed-payload", ToyPayload(label="run", value=11)
            )

    loaded = Experiment.from_artifact(experiment.artifact)
    with pytest.raises(Exception, match="Expected media type"):
        loaded.runs[0].get_with_codec(WrongMediaTypeCodec, "typed-payload")


def test_experiment_file_attachment_round_trip(tmp_path):
    source = tmp_path / "source.png"
    payload = b"\x89PNG\r\n\x1a\n"
    source.write_bytes(payload)

    with Experiment.with_temp_local_registry() as experiment:
        experiment.log_file("source-file", source)

    loaded = Experiment.from_artifact(experiment.artifact)
    descriptor = _attachment_by_name(loaded.experiment_attachments, "source-file")
    assert descriptor.media_type == "image/png"
    assert descriptor.annotations[_ATTACHMENT_FILENAME] == "source.png"
    assert loaded.get_blob("source-file") == payload

    output_dir = tmp_path / "restored"
    output_dir.mkdir()
    output_path = loaded.write_attachment("source-file", output_dir)
    assert output_path == output_dir / "source.png"
    assert output_path.read_bytes() == payload

    with pytest.raises(Exception, match="already exists"):
        loaded.write_attachment("source-file", output_dir)
    loaded.write_attachment("source-file", output_dir, overwrite=True)


def test_run_file_attachment_round_trip(tmp_path):
    source = tmp_path / "solver-output.bin"
    source.write_bytes(b"\x00solver-output")

    with Experiment.with_temp_local_registry() as experiment:
        with experiment.run() as run:
            run.log_file(
                "solver-output",
                source,
                media_type="application/octet-stream",
                filename="result.bin",
            )

    loaded = Experiment.from_artifact(experiment.artifact)
    run = loaded.runs[0]
    descriptor = _attachment_by_name(run.attachments, "solver-output")
    assert descriptor.media_type == "application/octet-stream"
    assert descriptor.annotations[_ATTACHMENT_FILENAME] == "result.bin"
    assert run.get_blob("solver-output") == b"\x00solver-output"

    output_path = tmp_path / "solver-output.copy"
    assert run.write_attachment("solver-output", output_path) == output_path
    assert output_path.read_bytes() == b"\x00solver-output"
