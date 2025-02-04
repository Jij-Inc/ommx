"""
@generated by mypy-protobuf.  Do not edit manually!
isort:skip_file
"""

import builtins
import collections.abc
import google.protobuf.descriptor
import google.protobuf.internal.containers
import google.protobuf.message
import typing

DESCRIPTOR: google.protobuf.descriptor.FileDescriptor

@typing.final
class SOS1(google.protobuf.message.Message):
    """A message representing a one-hot constraint."""

    DESCRIPTOR: google.protobuf.descriptor.Descriptor

    CONSTRAINT_IDS_FIELD_NUMBER: builtins.int
    DECISION_VARIABLES_FIELD_NUMBER: builtins.int
    @property
    def constraint_ids(
        self,
    ) -> google.protobuf.internal.containers.RepeatedScalarFieldContainer[builtins.int]:
        """The IDs of the original constraints."""

    @property
    def decision_variables(
        self,
    ) -> google.protobuf.internal.containers.RepeatedScalarFieldContainer[builtins.int]:
        """The list of ids of decision variables that are constrained to be one-hot."""

    def __init__(
        self,
        *,
        constraint_ids: collections.abc.Iterable[builtins.int] | None = ...,
        decision_variables: collections.abc.Iterable[builtins.int] | None = ...,
    ) -> None: ...
    def ClearField(
        self,
        field_name: typing.Literal[
            "constraint_ids",
            b"constraint_ids",
            "decision_variables",
            b"decision_variables",
        ],
    ) -> None: ...

global___SOS1 = SOS1
