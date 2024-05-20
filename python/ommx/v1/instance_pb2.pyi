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
import ommx.v1.constraint_pb2
import ommx.v1.decision_variables_pb2
import ommx.v1.function_pb2
import sys
import typing

if sys.version_info >= (3, 10):
    import typing as typing_extensions
else:
    import typing_extensions

DESCRIPTOR: google.protobuf.descriptor.FileDescriptor

@typing.final
class Instance(google.protobuf.message.Message):
    DESCRIPTOR: google.protobuf.descriptor.Descriptor

    class _Sense:
        ValueType = typing.NewType("ValueType", builtins.int)
        V: typing_extensions.TypeAlias = ValueType

    class _SenseEnumTypeWrapper(
        google.protobuf.internal.enum_type_wrapper._EnumTypeWrapper[
            Instance._Sense.ValueType
        ],
        builtins.type,
    ):
        DESCRIPTOR: google.protobuf.descriptor.EnumDescriptor
        SENSE_UNSPECIFIED: Instance._Sense.ValueType  # 0
        SENSE_MINIMIZE: Instance._Sense.ValueType  # 1
        SENSE_MAXIMIZE: Instance._Sense.ValueType  # 2

    class Sense(_Sense, metaclass=_SenseEnumTypeWrapper):
        """Other types of constraints will be appended here

        TODO: Add semi-definite constraints to represent SDP
        repeated SemiDefiniteConstraint semi_definite_constraints = ?;

        The sense of this instance
        """

    SENSE_UNSPECIFIED: Instance.Sense.ValueType  # 0
    SENSE_MINIMIZE: Instance.Sense.ValueType  # 1
    SENSE_MAXIMIZE: Instance.Sense.ValueType  # 2

    @typing.final
    class Description(google.protobuf.message.Message):
        DESCRIPTOR: google.protobuf.descriptor.Descriptor

        NAME_FIELD_NUMBER: builtins.int
        DESCRIPTION_FIELD_NUMBER: builtins.int
        AUTHORS_FIELD_NUMBER: builtins.int
        CREATED_BY_FIELD_NUMBER: builtins.int
        name: builtins.str
        description: builtins.str
        created_by: builtins.str
        """The application or library name that created this message."""
        @property
        def authors(
            self,
        ) -> google.protobuf.internal.containers.RepeatedScalarFieldContainer[
            builtins.str
        ]: ...
        def __init__(
            self,
            *,
            name: builtins.str | None = ...,
            description: builtins.str | None = ...,
            authors: collections.abc.Iterable[builtins.str] | None = ...,
            created_by: builtins.str | None = ...,
        ) -> None: ...
        def HasField(
            self,
            field_name: typing.Literal[
                "_created_by",
                b"_created_by",
                "_description",
                b"_description",
                "_name",
                b"_name",
                "created_by",
                b"created_by",
                "description",
                b"description",
                "name",
                b"name",
            ],
        ) -> builtins.bool: ...
        def ClearField(
            self,
            field_name: typing.Literal[
                "_created_by",
                b"_created_by",
                "_description",
                b"_description",
                "_name",
                b"_name",
                "authors",
                b"authors",
                "created_by",
                b"created_by",
                "description",
                b"description",
                "name",
                b"name",
            ],
        ) -> None: ...
        @typing.overload
        def WhichOneof(
            self, oneof_group: typing.Literal["_created_by", b"_created_by"]
        ) -> typing.Literal["created_by"] | None: ...
        @typing.overload
        def WhichOneof(
            self, oneof_group: typing.Literal["_description", b"_description"]
        ) -> typing.Literal["description"] | None: ...
        @typing.overload
        def WhichOneof(
            self, oneof_group: typing.Literal["_name", b"_name"]
        ) -> typing.Literal["name"] | None: ...

    DESCRIPTION_FIELD_NUMBER: builtins.int
    DECISION_VARIABLES_FIELD_NUMBER: builtins.int
    OBJECTIVE_FIELD_NUMBER: builtins.int
    CONSTRAINTS_FIELD_NUMBER: builtins.int
    SENSE_FIELD_NUMBER: builtins.int
    sense: global___Instance.Sense.ValueType
    """The sense of this problem, i.e. minimize the objective or maximize it.

    Design decision note:
    - This is a required field. Most mathematical modeling tools allow for an empty sense and default to minimization. Alternatively, some tools do not create such a field and represent maximization problems by negating the objective function. This project prefers explicit descriptions over implicit ones to avoid such ambiguity and to make it unnecessary for developers to look up the reference for the treatment of omitted cases.
    """
    @property
    def description(self) -> global___Instance.Description: ...
    @property
    def decision_variables(
        self,
    ) -> google.protobuf.internal.containers.RepeatedCompositeFieldContainer[
        ommx.v1.decision_variables_pb2.DecisionVariable
    ]:
        """Decision variables used in this instance

        - This must constain every decision variables used in the objective and constraints.
        - This can contains a decision variable that is not used in the objective or constraints.
        """

    @property
    def objective(self) -> ommx.v1.function_pb2.Function: ...
    @property
    def constraints(
        self,
    ) -> google.protobuf.internal.containers.RepeatedCompositeFieldContainer[
        ommx.v1.constraint_pb2.Constraint
    ]:
        """Constraints of the optimization problem"""

    def __init__(
        self,
        *,
        description: global___Instance.Description | None = ...,
        decision_variables: collections.abc.Iterable[
            ommx.v1.decision_variables_pb2.DecisionVariable
        ]
        | None = ...,
        objective: ommx.v1.function_pb2.Function | None = ...,
        constraints: collections.abc.Iterable[ommx.v1.constraint_pb2.Constraint]
        | None = ...,
        sense: global___Instance.Sense.ValueType = ...,
    ) -> None: ...
    def HasField(
        self,
        field_name: typing.Literal[
            "description", b"description", "objective", b"objective"
        ],
    ) -> builtins.bool: ...
    def ClearField(
        self,
        field_name: typing.Literal[
            "constraints",
            b"constraints",
            "decision_variables",
            b"decision_variables",
            "description",
            b"description",
            "objective",
            b"objective",
            "sense",
            b"sense",
        ],
    ) -> None: ...

global___Instance = Instance
