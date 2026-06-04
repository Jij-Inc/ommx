import json
from dataclasses import dataclass

from ommx.artifact import Descriptor
from ommx.experiment import Experiment

_ATTACHMENT_NAME = "org.ommx.attachment.name"


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
