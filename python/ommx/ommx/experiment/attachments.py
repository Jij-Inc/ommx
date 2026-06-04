"""Typed helpers for Experiment attachments.

OMMX stores Experiment attachments as media-typed bytes. Provider packages
that own richer Python objects can implement :class:`AttachmentCodec` to define
how those objects are serialized into attachment bytes and deserialized back.
"""

from __future__ import annotations

from typing import Protocol, TypeVar

T = TypeVar("T")


class AttachmentCodec(Protocol[T]):
    """Codec for one Python attachment payload type.

    The codec implementation should live with the package that owns ``T``.
    For example, a JijModeling ``Problem`` codec belongs in ``jijmodeling``,
    not in OMMX.
    """

    @property
    def media_type(self) -> str:
        """OCI media type used for this attachment payload."""
        ...

    def serialize(self, value: T, /) -> bytes:
        """Serialize a Python object to attachment bytes."""
        ...

    def deserialize(self, data: bytes, /) -> T:
        """Deserialize attachment bytes back to a Python object."""
        ...


__all__ = [
    "AttachmentCodec",
    "T",
]
