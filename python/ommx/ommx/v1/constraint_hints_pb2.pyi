"""
@generated by mypy-protobuf.  Do not edit manually!
isort:skip_file
"""

import builtins
import collections.abc
import google.protobuf.descriptor
import google.protobuf.internal.containers
import google.protobuf.message
import ommx.v1.one_hot_pb2
import ommx.v1.sos1_pb2
import typing

DESCRIPTOR: google.protobuf.descriptor.FileDescriptor

@typing.final
class ConstraintHints(google.protobuf.message.Message):
    """A constraint hint is an additional inforomation to be used by solver to gain performance.
    They are derived from one-or-more constraints in the instance and typically contains information of special types of constraints (e.g. one-hot, SOS, ...).
    """

    DESCRIPTOR: google.protobuf.descriptor.Descriptor

    ONE_HOT_CONSTRAINTS_FIELD_NUMBER: builtins.int
    SOS1_CONSTRAINTS_FIELD_NUMBER: builtins.int
    @property
    def one_hot_constraints(
        self,
    ) -> google.protobuf.internal.containers.RepeatedCompositeFieldContainer[
        ommx.v1.one_hot_pb2.OneHot
    ]:
        """One-hot constraint: e.g. `x_1 + ... + x_n = 1` for binary variables `x_1, ..., x_n`."""

    @property
    def sos1_constraints(
        self,
    ) -> google.protobuf.internal.containers.RepeatedCompositeFieldContainer[
        ommx.v1.sos1_pb2.SOS1
    ]:
        """SOS1 constraint: at most one of x_1, ..., x_n can be non-zero."""

    def __init__(
        self,
        *,
        one_hot_constraints: collections.abc.Iterable[ommx.v1.one_hot_pb2.OneHot]
        | None = ...,
        sos1_constraints: collections.abc.Iterable[ommx.v1.sos1_pb2.SOS1] | None = ...,
    ) -> None: ...
    def ClearField(
        self,
        field_name: typing.Literal[
            "one_hot_constraints",
            b"one_hot_constraints",
            "sos1_constraints",
            b"sos1_constraints",
        ],
    ) -> None: ...

global___ConstraintHints = ConstraintHints
