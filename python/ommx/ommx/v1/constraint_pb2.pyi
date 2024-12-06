"""
@generated by mypy-protobuf.  Do not edit manually!
isort:skip_file
"""

import builtins
import collections.abc
import google.protobuf.descriptor
import google.protobuf.internal.containers
import google.protobuf.internal.enum_type_wrapper
import google.protobuf.message
import ommx.v1.function_pb2
import sys
import typing

if sys.version_info >= (3, 10):
    import typing as typing_extensions
else:
    import typing_extensions

DESCRIPTOR: google.protobuf.descriptor.FileDescriptor

class _Equality:
    ValueType = typing.NewType("ValueType", builtins.int)
    V: typing_extensions.TypeAlias = ValueType

class _EqualityEnumTypeWrapper(
    google.protobuf.internal.enum_type_wrapper._EnumTypeWrapper[_Equality.ValueType],
    builtins.type,
):
    DESCRIPTOR: google.protobuf.descriptor.EnumDescriptor
    EQUALITY_UNSPECIFIED: _Equality.ValueType  # 0
    EQUALITY_EQUAL_TO_ZERO: _Equality.ValueType  # 1
    EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO: _Equality.ValueType  # 2

class Equality(_Equality, metaclass=_EqualityEnumTypeWrapper):
    """Equality of a constraint."""

EQUALITY_UNSPECIFIED: Equality.ValueType  # 0
EQUALITY_EQUAL_TO_ZERO: Equality.ValueType  # 1
EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO: Equality.ValueType  # 2
global___Equality = Equality

@typing.final
class Constraint(google.protobuf.message.Message):
    DESCRIPTOR: google.protobuf.descriptor.Descriptor

    @typing.final
    class ParametersEntry(google.protobuf.message.Message):
        DESCRIPTOR: google.protobuf.descriptor.Descriptor

        KEY_FIELD_NUMBER: builtins.int
        VALUE_FIELD_NUMBER: builtins.int
        key: builtins.str
        value: builtins.str
        def __init__(
            self,
            *,
            key: builtins.str = ...,
            value: builtins.str = ...,
        ) -> None: ...
        def ClearField(
            self, field_name: typing.Literal["key", b"key", "value", b"value"]
        ) -> None: ...

    ID_FIELD_NUMBER: builtins.int
    EQUALITY_FIELD_NUMBER: builtins.int
    FUNCTION_FIELD_NUMBER: builtins.int
    SUBSCRIPTS_FIELD_NUMBER: builtins.int
    PARAMETERS_FIELD_NUMBER: builtins.int
    NAME_FIELD_NUMBER: builtins.int
    DESCRIPTION_FIELD_NUMBER: builtins.int
    id: builtins.int
    """Constraint ID

    - Constraint IDs are managed separately from decision variable IDs.
      We can use the same ID for both. For example, we have a decision variable `x` with decision variable ID `1``
      and constraint `x == 0` with constraint ID `1`.
    - IDs are not required to be sequential.
    - IDs must be unique with other types of constraints.
    """
    equality: global___Equality.ValueType
    name: builtins.str
    """Name of the constraint."""
    description: builtins.str
    """Detail human-readable description of the constraint."""
    @property
    def function(self) -> ommx.v1.function_pb2.Function: ...
    @property
    def subscripts(
        self,
    ) -> google.protobuf.internal.containers.RepeatedScalarFieldContainer[builtins.int]:
        """Integer parameters of the constraint.

        Consider for example a problem constains a series of constraints `x[i, j] + y[i, j] <= 10` for `i = 1, 2, 3` and `j = 4, 5`,
        then 6 = 3x2 `Constraint` messages should be created corresponding to each pair of `i` and `j`.
        The `name` field of this message is intended to be a human-readable name of `x[i, j] + y[i, j] <= 10`,
        and the `subscripts` field is intended to be the value of `[i, j]` like `[1, 5]`.
        """

    @property
    def parameters(
        self,
    ) -> google.protobuf.internal.containers.ScalarMap[builtins.str, builtins.str]:
        """Key-value parameters of the constraint."""

    def __init__(
        self,
        *,
        id: builtins.int = ...,
        equality: global___Equality.ValueType = ...,
        function: ommx.v1.function_pb2.Function | None = ...,
        subscripts: collections.abc.Iterable[builtins.int] | None = ...,
        parameters: collections.abc.Mapping[builtins.str, builtins.str] | None = ...,
        name: builtins.str | None = ...,
        description: builtins.str | None = ...,
    ) -> None: ...
    def HasField(
        self,
        field_name: typing.Literal[
            "_description",
            b"_description",
            "_name",
            b"_name",
            "description",
            b"description",
            "function",
            b"function",
            "name",
            b"name",
        ],
    ) -> builtins.bool: ...
    def ClearField(
        self,
        field_name: typing.Literal[
            "_description",
            b"_description",
            "_name",
            b"_name",
            "description",
            b"description",
            "equality",
            b"equality",
            "function",
            b"function",
            "id",
            b"id",
            "name",
            b"name",
            "parameters",
            b"parameters",
            "subscripts",
            b"subscripts",
        ],
    ) -> None: ...
    @typing.overload
    def WhichOneof(
        self, oneof_group: typing.Literal["_description", b"_description"]
    ) -> typing.Literal["description"] | None: ...
    @typing.overload
    def WhichOneof(
        self, oneof_group: typing.Literal["_name", b"_name"]
    ) -> typing.Literal["name"] | None: ...

global___Constraint = Constraint

@typing.final
class EvaluatedConstraint(google.protobuf.message.Message):
    """A constraint evaluated with a state"""

    DESCRIPTOR: google.protobuf.descriptor.Descriptor

    @typing.final
    class ParametersEntry(google.protobuf.message.Message):
        DESCRIPTOR: google.protobuf.descriptor.Descriptor

        KEY_FIELD_NUMBER: builtins.int
        VALUE_FIELD_NUMBER: builtins.int
        key: builtins.str
        value: builtins.str
        def __init__(
            self,
            *,
            key: builtins.str = ...,
            value: builtins.str = ...,
        ) -> None: ...
        def ClearField(
            self, field_name: typing.Literal["key", b"key", "value", b"value"]
        ) -> None: ...

    ID_FIELD_NUMBER: builtins.int
    EQUALITY_FIELD_NUMBER: builtins.int
    EVALUATED_VALUE_FIELD_NUMBER: builtins.int
    USED_DECISION_VARIABLE_IDS_FIELD_NUMBER: builtins.int
    SUBSCRIPTS_FIELD_NUMBER: builtins.int
    PARAMETERS_FIELD_NUMBER: builtins.int
    NAME_FIELD_NUMBER: builtins.int
    DESCRIPTION_FIELD_NUMBER: builtins.int
    DUAL_VARIABLE_FIELD_NUMBER: builtins.int
    id: builtins.int
    equality: global___Equality.ValueType
    evaluated_value: builtins.float
    """The value of function for the state"""
    name: builtins.str
    """Name of the constraint."""
    description: builtins.str
    """Detail human-readable description of the constraint."""
    dual_variable: builtins.float
    """Value for the Lagrangian dual variable of this constraint.
    This is optional because not all solvers support to evaluate dual variables.
    """
    @property
    def used_decision_variable_ids(
        self,
    ) -> google.protobuf.internal.containers.RepeatedScalarFieldContainer[builtins.int]:
        """IDs of decision variables used to evalute this constraint"""

    @property
    def subscripts(
        self,
    ) -> google.protobuf.internal.containers.RepeatedScalarFieldContainer[builtins.int]:
        """Integer parameters of the constraint."""

    @property
    def parameters(
        self,
    ) -> google.protobuf.internal.containers.ScalarMap[builtins.str, builtins.str]:
        """Key-value parameters of the constraint."""

    def __init__(
        self,
        *,
        id: builtins.int = ...,
        equality: global___Equality.ValueType = ...,
        evaluated_value: builtins.float = ...,
        used_decision_variable_ids: collections.abc.Iterable[builtins.int] | None = ...,
        subscripts: collections.abc.Iterable[builtins.int] | None = ...,
        parameters: collections.abc.Mapping[builtins.str, builtins.str] | None = ...,
        name: builtins.str | None = ...,
        description: builtins.str | None = ...,
        dual_variable: builtins.float | None = ...,
    ) -> None: ...
    def HasField(
        self,
        field_name: typing.Literal[
            "_description",
            b"_description",
            "_dual_variable",
            b"_dual_variable",
            "_name",
            b"_name",
            "description",
            b"description",
            "dual_variable",
            b"dual_variable",
            "name",
            b"name",
        ],
    ) -> builtins.bool: ...
    def ClearField(
        self,
        field_name: typing.Literal[
            "_description",
            b"_description",
            "_dual_variable",
            b"_dual_variable",
            "_name",
            b"_name",
            "description",
            b"description",
            "dual_variable",
            b"dual_variable",
            "equality",
            b"equality",
            "evaluated_value",
            b"evaluated_value",
            "id",
            b"id",
            "name",
            b"name",
            "parameters",
            b"parameters",
            "subscripts",
            b"subscripts",
            "used_decision_variable_ids",
            b"used_decision_variable_ids",
        ],
    ) -> None: ...
    @typing.overload
    def WhichOneof(
        self, oneof_group: typing.Literal["_description", b"_description"]
    ) -> typing.Literal["description"] | None: ...
    @typing.overload
    def WhichOneof(
        self, oneof_group: typing.Literal["_dual_variable", b"_dual_variable"]
    ) -> typing.Literal["dual_variable"] | None: ...
    @typing.overload
    def WhichOneof(
        self, oneof_group: typing.Literal["_name", b"_name"]
    ) -> typing.Literal["name"] | None: ...

global___EvaluatedConstraint = EvaluatedConstraint

@typing.final
class RemovedConstraint(google.protobuf.message.Message):
    DESCRIPTOR: google.protobuf.descriptor.Descriptor

    @typing.final
    class ParametersEntry(google.protobuf.message.Message):
        DESCRIPTOR: google.protobuf.descriptor.Descriptor

        KEY_FIELD_NUMBER: builtins.int
        VALUE_FIELD_NUMBER: builtins.int
        key: builtins.str
        value: builtins.str
        def __init__(
            self,
            *,
            key: builtins.str = ...,
            value: builtins.str = ...,
        ) -> None: ...
        def ClearField(
            self, field_name: typing.Literal["key", b"key", "value", b"value"]
        ) -> None: ...

    CONSTRAINT_FIELD_NUMBER: builtins.int
    REASON_FIELD_NUMBER: builtins.int
    PARAMETERS_FIELD_NUMBER: builtins.int
    reason: builtins.str
    """Short reason why the constraint was removed.

    This should be the name of method, function or application which remove the constraint.
    """
    @property
    def constraint(self) -> global___Constraint:
        """The removed constraint"""

    @property
    def parameters(
        self,
    ) -> google.protobuf.internal.containers.ScalarMap[builtins.str, builtins.str]:
        """Arbitrary key-value parameters representing why the constraint was removed.

        This should be human-readable and can be used for debugging.
        """

    def __init__(
        self,
        *,
        constraint: global___Constraint | None = ...,
        reason: builtins.str = ...,
        parameters: collections.abc.Mapping[builtins.str, builtins.str] | None = ...,
    ) -> None: ...
    def HasField(
        self, field_name: typing.Literal["constraint", b"constraint"]
    ) -> builtins.bool: ...
    def ClearField(
        self,
        field_name: typing.Literal[
            "constraint",
            b"constraint",
            "parameters",
            b"parameters",
            "reason",
            b"reason",
        ],
    ) -> None: ...

global___RemovedConstraint = RemovedConstraint
