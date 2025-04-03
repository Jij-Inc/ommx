"""
@generated by mypy-protobuf.  Do not edit manually!
isort:skip_file
"""

import builtins
import collections.abc
import google.protobuf.descriptor
import google.protobuf.internal.containers
import google.protobuf.message
import ommx.v1.k_hot_pb2
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

    @typing.final
    class KHotConstraintsEntry(google.protobuf.message.Message):
        DESCRIPTOR: google.protobuf.descriptor.Descriptor

        KEY_FIELD_NUMBER: builtins.int
        VALUE_FIELD_NUMBER: builtins.int
        key: builtins.int
        @property
        def value(self) -> global___KHotList: ...
        def __init__(
            self,
            *,
            key: builtins.int = ...,
            value: global___KHotList | None = ...,
        ) -> None: ...
        def HasField(
            self, field_name: typing.Literal["value", b"value"]
        ) -> builtins.bool: ...
        def ClearField(
            self, field_name: typing.Literal["key", b"key", "value", b"value"]
        ) -> None: ...

    ONE_HOT_CONSTRAINTS_FIELD_NUMBER: builtins.int
    SOS1_CONSTRAINTS_FIELD_NUMBER: builtins.int
    K_HOT_CONSTRAINTS_FIELD_NUMBER: builtins.int
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

    @property
    def k_hot_constraints(
        self,
    ) -> google.protobuf.internal.containers.MessageMap[
        builtins.int, global___KHotList
    ]:
        """K-hot constraints: map from k to a list of k-hot constraints."""

    def __init__(
        self,
        *,
        one_hot_constraints: collections.abc.Iterable[ommx.v1.one_hot_pb2.OneHot]
        | None = ...,
        sos1_constraints: collections.abc.Iterable[ommx.v1.sos1_pb2.SOS1] | None = ...,
        k_hot_constraints: collections.abc.Mapping[builtins.int, global___KHotList]
        | None = ...,
    ) -> None: ...
    def ClearField(
        self,
        field_name: typing.Literal[
            "k_hot_constraints",
            b"k_hot_constraints",
            "one_hot_constraints",
            b"one_hot_constraints",
            "sos1_constraints",
            b"sos1_constraints",
        ],
    ) -> None: ...

global___ConstraintHints = ConstraintHints

@typing.final
class KHotList(google.protobuf.message.Message):
    """A list of KHot constraints with the same k value."""

    DESCRIPTOR: google.protobuf.descriptor.Descriptor

    CONSTRAINTS_FIELD_NUMBER: builtins.int
    @property
    def constraints(
        self,
    ) -> google.protobuf.internal.containers.RepeatedCompositeFieldContainer[
        ommx.v1.k_hot_pb2.KHot
    ]: ...
    def __init__(
        self,
        *,
        constraints: collections.abc.Iterable[ommx.v1.k_hot_pb2.KHot] | None = ...,
    ) -> None: ...
    def ClearField(
        self, field_name: typing.Literal["constraints", b"constraints"]
    ) -> None: ...

global___KHotList = KHotList
