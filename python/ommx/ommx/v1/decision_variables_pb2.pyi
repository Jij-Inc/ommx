# pyright: reportIncompatibleVariableOverride=false
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
import sys
import typing

if sys.version_info >= (3, 10):
    import typing as typing_extensions
else:
    import typing_extensions

DESCRIPTOR: google.protobuf.descriptor.FileDescriptor

@typing.final
class Bound(google.protobuf.message.Message):
    """Upper and lower bound of the decision variable."""

    DESCRIPTOR: google.protobuf.descriptor.Descriptor

    LOWER_FIELD_NUMBER: builtins.int
    UPPER_FIELD_NUMBER: builtins.int
    lower: builtins.float
    """Lower bound of the decision variable."""
    upper: builtins.float
    """Upper bound of the decision variable."""
    def __init__(
        self,
        *,
        lower: builtins.float = ...,
        upper: builtins.float = ...,
    ) -> None: ...
    def ClearField(
        self, field_name: typing.Literal["lower", b"lower", "upper", b"upper"]
    ) -> None: ...

global___Bound = Bound

@typing.final
class DecisionVariable(google.protobuf.message.Message):
    """Decison variable which mathematical programming solver will optimize.
    It must have its kind, i.e. binary, integer, real or others and unique identifier of 64-bit integer.
    It may have its name and subscripts which are used to identify in modeling tools.
    """

    DESCRIPTOR: google.protobuf.descriptor.Descriptor

    class _Kind:
        ValueType = typing.NewType("ValueType", builtins.int)
        V: typing_extensions.TypeAlias = ValueType

    class _KindEnumTypeWrapper(
        google.protobuf.internal.enum_type_wrapper._EnumTypeWrapper[
            DecisionVariable._Kind.ValueType
        ],
        builtins.type,
    ):
        DESCRIPTOR: google.protobuf.descriptor.EnumDescriptor
        KIND_UNSPECIFIED: DecisionVariable._Kind.ValueType  # 0
        KIND_BINARY: DecisionVariable._Kind.ValueType  # 1
        KIND_INTEGER: DecisionVariable._Kind.ValueType  # 2
        KIND_CONTINUOUS: DecisionVariable._Kind.ValueType  # 3
        KIND_SEMI_INTEGER: DecisionVariable._Kind.ValueType  # 4
        """Semi-integer decision variable is a decision variable that can take only integer values in the given range or zero."""
        KIND_SEMI_CONTINUOUS: DecisionVariable._Kind.ValueType  # 5
        """Semi-continuous decision variable is a decision variable that can take only continuous values in the given range or zero."""

    class Kind(_Kind, metaclass=_KindEnumTypeWrapper):
        """Kind of the decision variable"""

    KIND_UNSPECIFIED: DecisionVariable.Kind.ValueType  # 0
    KIND_BINARY: DecisionVariable.Kind.ValueType  # 1
    KIND_INTEGER: DecisionVariable.Kind.ValueType  # 2
    KIND_CONTINUOUS: DecisionVariable.Kind.ValueType  # 3
    KIND_SEMI_INTEGER: DecisionVariable.Kind.ValueType  # 4
    """Semi-integer decision variable is a decision variable that can take only integer values in the given range or zero."""
    KIND_SEMI_CONTINUOUS: DecisionVariable.Kind.ValueType  # 5
    """Semi-continuous decision variable is a decision variable that can take only continuous values in the given range or zero."""

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
    KIND_FIELD_NUMBER: builtins.int
    BOUND_FIELD_NUMBER: builtins.int
    NAME_FIELD_NUMBER: builtins.int
    SUBSCRIPTS_FIELD_NUMBER: builtins.int
    PARAMETERS_FIELD_NUMBER: builtins.int
    DESCRIPTION_FIELD_NUMBER: builtins.int
    SUBSTITUTED_VALUE_FIELD_NUMBER: builtins.int
    id: builtins.int
    """Decision variable ID.

    - IDs are not required to be sequential.
    """
    kind: global___DecisionVariable.Kind.ValueType
    """Kind of the decision variable"""
    name: builtins.str
    """Name of the decision variable. e.g. `x`"""
    description: builtins.str
    """Detail human-readable description of the decision variable."""
    substituted_value: builtins.float
    """The value substituted by partial evaluation of the instance. Not determined by the solver."""
    @property
    def bound(self) -> global___Bound:
        """Bound of the decision variable
        If the bound is not specified, the decision variable is considered as unbounded.
        """

    @property
    def subscripts(
        self,
    ) -> google.protobuf.internal.containers.RepeatedScalarFieldContainer[builtins.int]:
        """Subscripts of the decision variable. e.g. `[1, 3]` for an element of multidimensional deicion variable `x[1, 3]`"""

    @property
    def parameters(
        self,
    ) -> google.protobuf.internal.containers.ScalarMap[builtins.str, builtins.str]:
        """Additional parameters for decision variables"""

    def __init__(
        self,
        *,
        id: builtins.int = ...,
        kind: global___DecisionVariable.Kind.ValueType = ...,
        bound: global___Bound | None = ...,
        name: builtins.str | None = ...,
        subscripts: collections.abc.Iterable[builtins.int] | None = ...,
        parameters: collections.abc.Mapping[builtins.str, builtins.str] | None = ...,
        description: builtins.str | None = ...,
        substituted_value: builtins.float | None = ...,
    ) -> None: ...
    def HasField(
        self,
        field_name: typing.Literal[
            "_bound",
            b"_bound",
            "_description",
            b"_description",
            "_name",
            b"_name",
            "_substituted_value",
            b"_substituted_value",
            "bound",
            b"bound",
            "description",
            b"description",
            "name",
            b"name",
            "substituted_value",
            b"substituted_value",
        ],
    ) -> builtins.bool: ...
    def ClearField(
        self,
        field_name: typing.Literal[
            "_bound",
            b"_bound",
            "_description",
            b"_description",
            "_name",
            b"_name",
            "_substituted_value",
            b"_substituted_value",
            "bound",
            b"bound",
            "description",
            b"description",
            "id",
            b"id",
            "kind",
            b"kind",
            "name",
            b"name",
            "parameters",
            b"parameters",
            "subscripts",
            b"subscripts",
            "substituted_value",
            b"substituted_value",
        ],
    ) -> None: ...
    @typing.overload
    def WhichOneof(
        self, oneof_group: typing.Literal["_bound", b"_bound"]
    ) -> typing.Literal["bound"] | None: ...
    @typing.overload
    def WhichOneof(
        self, oneof_group: typing.Literal["_description", b"_description"]
    ) -> typing.Literal["description"] | None: ...
    @typing.overload
    def WhichOneof(
        self, oneof_group: typing.Literal["_name", b"_name"]
    ) -> typing.Literal["name"] | None: ...
    @typing.overload
    def WhichOneof(
        self, oneof_group: typing.Literal["_substituted_value", b"_substituted_value"]
    ) -> typing.Literal["substituted_value"] | None: ...

global___DecisionVariable = DecisionVariable
