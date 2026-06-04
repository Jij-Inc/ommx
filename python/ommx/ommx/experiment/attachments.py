"""Typed helpers for Experiment attachments.

OMMX stores Experiment attachments as media-typed bytes. Provider packages
that own richer Python objects can implement :class:`AttachmentCodec` to define
how those objects are encoded into attachment bytes and decoded back.
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

    media_type: str
    """OCI media type used for this attachment payload."""

    @staticmethod
    def encode(value: T, /) -> bytes:
        """Encode a Python object to attachment bytes."""
        ...

    @staticmethod
    def decode(data: bytes, /) -> T:
        """Decode attachment bytes back to a Python object."""
        ...


__all__ = [
    "AttachmentCodec",
    "T",
]
