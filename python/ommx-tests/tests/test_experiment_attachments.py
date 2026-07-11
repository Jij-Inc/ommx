import json
from dataclasses import dataclass

import pytest

from ommx.experiment import Experiment


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


def test_experiment_attachment_codec_round_trip():
    expected = ToyPayload(label="experiment", value=7)

    with Experiment.with_temp_local_registry() as experiment:
        experiment.log_with_codec(ToyPayloadCodec, "typed-payload", expected)

    loaded = Experiment.from_artifact(experiment.artifact)
    assert loaded.get_with_codec(ToyPayloadCodec, "typed-payload") == expected

    assert loaded.attachment_names == ["typed-payload"]
    assert loaded.attachment_media_type("typed-payload") == ToyPayloadCodec.media_type
    assert loaded.get_blob("typed-payload") == ToyPayloadCodec.encode(expected)


def test_run_attachment_codec_round_trip():
    expected = ToyPayload(label="run", value=11)

    with Experiment.with_temp_local_registry() as experiment:
        with experiment.run() as run:
            run.log_with_codec(ToyPayloadCodec, "typed-payload", expected)

    loaded = Experiment.from_artifact(experiment.artifact)
    run = loaded.runs[0]
    assert run.get_with_codec(ToyPayloadCodec, "typed-payload") == expected

    assert run.attachment_names == ["typed-payload"]
    assert run.attachment_media_type("typed-payload") == ToyPayloadCodec.media_type
    assert run.get_blob("typed-payload") == ToyPayloadCodec.encode(expected)


def test_compressed_attachment_round_trip_is_transparent():
    expected = {"values": [1] * 100}

    with Experiment.with_temp_local_registry() as experiment:
        experiment.log_json("experiment-trace", expected, compression="zstd")
        with experiment.run() as run:
            run.log_with_codec(
                ToyPayloadCodec,
                "run-payload",
                ToyPayload(label="compressed", value=13),
                compression="zstd",
            )

    loaded = Experiment.from_artifact(experiment.artifact)
    assert loaded.attachment_media_type("experiment-trace") == "application/json"
    assert loaded.get_attachment("experiment-trace") == expected
    assert json.loads(loaded.get_blob("experiment-trace")) == expected

    loaded_run = loaded.runs[0]
    assert loaded_run.attachment_media_type("run-payload") == ToyPayloadCodec.media_type
    assert loaded_run.get_with_codec(ToyPayloadCodec, "run-payload") == ToyPayload(
        label="compressed", value=13
    )


@pytest.mark.parametrize("compression", ["none", "zstd"])
def test_logical_zstd_media_type_suffix_round_trip(compression):
    media_type = "application/vnd.ommx-tests.payload+zstd"
    payload = b"payload"

    with Experiment.with_temp_local_registry() as experiment:
        experiment.log_attachment(
            "suffix", media_type, payload, compression=compression
        )

    loaded = Experiment.from_artifact(experiment.artifact)
    assert loaded.attachment_media_type("suffix") == media_type
    assert loaded.get_blob("suffix") == payload


def test_attachment_rejects_unknown_compression():
    with Experiment.with_temp_local_registry() as experiment:
        with pytest.raises(Exception, match="expected `none` or `zstd`"):
            experiment.log_attachment(
                "payload",
                "application/octet-stream",
                b"payload",
                compression="gzip",  # pyright: ignore[reportArgumentType] - runtime rejection
            )


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
    assert loaded.attachment_names == ["source-file"]
    assert loaded.attachment_media_type("source-file") == "image/png"
    assert loaded.get_blob("source-file") == payload

    output_dir = tmp_path / "restored"
    output_dir.mkdir()
    output_path = loaded.write_attachment("source-file", output_dir)
    assert output_path == output_dir / "source.png"
    assert output_path.read_bytes() == payload

    with pytest.raises(Exception, match="already exists"):
        loaded.write_attachment("source-file", output_dir)
    loaded.write_attachment("source-file", output_dir, overwrite=True)


def test_compressed_file_attachment_round_trip(tmp_path):
    source = tmp_path / "trace.json"
    payload = b'{"values":[' + b"1," * 99 + b"1]}"
    source.write_bytes(payload)

    with Experiment.with_temp_local_registry() as experiment:
        experiment.log_file(
            "trace", source, media_type="application/json", compression="zstd"
        )

    loaded = Experiment.from_artifact(experiment.artifact)
    assert loaded.attachment_media_type("trace") == "application/json"
    assert loaded.get_blob("trace") == payload
    output = tmp_path / "trace.copy.json"
    loaded.write_attachment("trace", output)
    assert output.read_bytes() == payload


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
    assert run.attachment_names == ["solver-output"]
    assert run.attachment_media_type("solver-output") == "application/octet-stream"
    assert run.get_blob("solver-output") == b"\x00solver-output"

    output_dir = tmp_path / "restored"
    output_dir.mkdir()
    restored_path = run.write_attachment("solver-output", output_dir)
    assert restored_path == output_dir / "result.bin"
    assert restored_path.read_bytes() == b"\x00solver-output"

    output_path = tmp_path / "solver-output.copy"
    assert run.write_attachment("solver-output", output_path) == output_path
    assert output_path.read_bytes() == b"\x00solver-output"
