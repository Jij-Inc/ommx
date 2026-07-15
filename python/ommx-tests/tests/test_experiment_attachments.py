import json
from dataclasses import dataclass

import pytest

from ommx.artifact import get_local_registry_root
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


class SentinelError(Exception):
    pass


class EncodeRaisesCodec:
    media_type = ToyPayloadCodec.media_type

    @staticmethod
    def encode(value: ToyPayload) -> bytes:
        raise SentinelError(f"encode failed for {value.label}")

    @staticmethod
    def decode(data: bytes) -> ToyPayload:
        return ToyPayloadCodec.decode(data)


class DecodeRaisesCodec:
    media_type = ToyPayloadCodec.media_type

    @staticmethod
    def encode(value: ToyPayload) -> bytes:
        return ToyPayloadCodec.encode(value)

    @staticmethod
    def decode(data: bytes) -> ToyPayload:
        raise SentinelError(f"decode failed for {len(data)} bytes")


class RaisingMediaTypeMeta(type):
    @property
    def media_type(cls) -> str:  # noqa: N805
        raise SentinelError("media_type lookup failed")


class RaisingMediaTypeCodec(metaclass=RaisingMediaTypeMeta):
    @staticmethod
    def encode(value: ToyPayload) -> bytes:
        return ToyPayloadCodec.encode(value)

    @staticmethod
    def decode(data: bytes) -> ToyPayload:
        return ToyPayloadCodec.decode(data)


class AttributeErrorMediaTypeMeta(type):
    @property
    def media_type(cls) -> str:  # noqa: N805
        raise AttributeError("media_type descriptor failed")


class AttributeErrorMediaTypeCodec(metaclass=AttributeErrorMediaTypeMeta):
    @staticmethod
    def encode(value: ToyPayload) -> bytes:
        return ToyPayloadCodec.encode(value)

    @staticmethod
    def decode(data: bytes) -> ToyPayload:
        return ToyPayloadCodec.decode(data)


class MissingMediaTypeCodec:
    @staticmethod
    def encode(value: ToyPayload) -> bytes:
        return ToyPayloadCodec.encode(value)


class NonStringMediaTypeCodec:
    media_type = 42

    @staticmethod
    def encode(value: ToyPayload) -> bytes:
        return ToyPayloadCodec.encode(value)


class NonBytesEncodeCodec:
    media_type = ToyPayloadCodec.media_type

    @staticmethod
    def encode(value: ToyPayload) -> str:
        return value.label


class ListEncodeCodec:
    media_type = ToyPayloadCodec.media_type

    @staticmethod
    def encode(value: ToyPayload) -> list[int]:
        return list(ToyPayloadCodec.encode(value))


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


def test_attachment_codec_callback_exceptions_are_preserved():
    value = ToyPayload(label="sentinel", value=17)
    experiment = Experiment.with_temp_local_registry()

    with pytest.raises(SentinelError, match="encode failed for sentinel"):
        experiment.log_with_codec(EncodeRaisesCodec, "encode", value)
    with pytest.raises(SentinelError, match="media_type lookup failed"):
        experiment.log_with_codec(
            RaisingMediaTypeCodec,  # pyright: ignore[reportArgumentType]
            "media-type",
            value,
        )
    with pytest.raises(AttributeError, match="media_type descriptor failed"):
        experiment.log_with_codec(
            AttributeErrorMediaTypeCodec,  # pyright: ignore[reportArgumentType]
            "attribute-error-media-type",
            value,
        )

    experiment.log_with_codec(ToyPayloadCodec, "decode", value)
    experiment.commit()
    loaded = Experiment.from_artifact(experiment.artifact)
    with pytest.raises(SentinelError, match="decode failed"):
        loaded.get_with_codec(DecodeRaisesCodec, "decode")


@pytest.mark.parametrize(
    "codec",
    [
        MissingMediaTypeCodec,
        NonStringMediaTypeCodec,
        NonBytesEncodeCodec,
        ListEncodeCodec,
    ],
)
def test_attachment_codec_protocol_failures_raise_type_error(codec):
    experiment = Experiment.with_temp_local_registry()
    with pytest.raises(TypeError):
        experiment.log_with_codec(
            codec,  # pyright: ignore[reportArgumentType]
            "invalid",
            ToyPayload(label="invalid", value=0),
        )


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
        with pytest.raises(ValueError, match="expected `none` or `zstd`"):
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
    with pytest.raises(ValueError, match="Expected media type"):
        loaded.get_with_codec(WrongMediaTypeCodec, "typed-payload")


def test_run_attachment_codec_rejects_media_type_mismatch():
    with Experiment.with_temp_local_registry() as experiment:
        with experiment.run() as run:
            run.log_with_codec(
                ToyPayloadCodec, "typed-payload", ToyPayload(label="run", value=11)
            )

    loaded = Experiment.from_artifact(experiment.artifact)
    with pytest.raises(ValueError, match="Expected media type"):
        loaded.runs[0].get_with_codec(WrongMediaTypeCodec, "typed-payload")


def test_codec_media_type_mismatch_precedes_blob_read():
    experiment = Experiment()
    experiment.log_with_codec(
        ToyPayloadCodec, "experiment-payload", ToyPayload(label="experiment", value=7)
    )
    with experiment.run() as run:
        run.log_with_codec(
            ToyPayloadCodec, "run-payload", ToyPayload(label="run", value=11)
        )
    artifact = experiment.commit()
    loaded = Experiment.from_artifact(artifact)

    payload_layers = [
        layer
        for layer in artifact.layers
        if layer.media_type == ToyPayloadCodec.media_type
    ]
    assert len(payload_layers) == 2
    blobs = {}
    for layer in payload_layers:
        algorithm, encoded = layer.digest.split(":", maxsplit=1)
        path = get_local_registry_root() / "blobs" / algorithm / encoded
        blobs[path] = path.read_bytes()
        path.unlink()

    try:
        with pytest.raises(ValueError, match="Expected media type"):
            loaded.get_with_codec(WrongMediaTypeCodec, "experiment-payload")
        with pytest.raises(ValueError, match="Expected media type"):
            loaded.runs[0].get_with_codec(WrongMediaTypeCodec, "run-payload")
        with pytest.raises(RuntimeError, match="Failed to read blob"):
            loaded.get_with_codec(ToyPayloadCodec, "experiment-payload")
    finally:
        for path, blob in blobs.items():
            path.write_bytes(blob)


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

    with pytest.raises(RuntimeError, match="already exists"):
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
