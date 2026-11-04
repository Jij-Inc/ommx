Encode a Python object with an attachment codec and attach it in Experiment scope.

A codec supplies `media_type`, `encode(value) -> bytes`, and
`decode(bytes) -> object`, as described by
{class}`ommx.experiment.attachments.AttachmentCodec`. OMMX owns that protocol;
a concrete codec should live in the package that owns the payload type.
`compression="zstd"` compresses the encoded bytes transparently.

Prefer the typed logging methods for supported OMMX/JSON values and
{meth}`log_file` for an existing file. Use a codec when the Python object needs
its own encoding. For example, this codec stores a small application-owned
coordinate value:

```pycon
>>> import json
>>> from ommx.experiment import Experiment
>>> class PointCodec:
...     media_type = "application/vnd.example.point+json"
...
...     @staticmethod
...     def encode(value):
...         return json.dumps({"x": value[0], "y": value[1]}).encode("utf-8")
...
...     @staticmethod
...     def decode(data):
...         point = json.loads(data)
...         return (point["x"], point["y"])
>>> with Experiment.with_temp_local_registry() as experiment:
...     experiment.log_with_codec(PointCodec, "location", (2, 3), compression="zstd")
>>> experiment.get_with_codec(PointCodec, "location")
(2, 3)
>>> experiment.attachment_media_type("location") == PointCodec.media_type
True

```

The reader needs the matching codec and its provider package. For an external
modeling object, use the owning package's public serialization API and media
type/version contract; OMMX does not infer its meaning or guarantee compatibility
between provider versions. {meth}`get_with_codec` validates the attachment's
logical media type before decoding. Exceptions raised by a codec pass through
unchanged.

{meth}`Run.log_with_codec` and {meth}`SealedRun.get_with_codec` provide the same
protocol for a payload associated with one trial.
