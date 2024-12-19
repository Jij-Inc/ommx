"""
@generated by mypy-protobuf.  Do not edit manually!
isort:skip_file
"""

import builtins
import collections.abc
import google.protobuf.descriptor
import google.protobuf.internal.containers
import google.protobuf.message
import ommx.v1.constraint_pb2
import ommx.v1.decision_variables_pb2
import ommx.v1.solution_pb2
import typing

DESCRIPTOR: google.protobuf.descriptor.FileDescriptor

@typing.final
class States(google.protobuf.message.Message):
    """List of states"""

    DESCRIPTOR: google.protobuf.descriptor.Descriptor

    STATES_FIELD_NUMBER: builtins.int
    @property
    def states(
        self,
    ) -> google.protobuf.internal.containers.RepeatedCompositeFieldContainer[
        ommx.v1.solution_pb2.State
    ]: ...
    def __init__(
        self,
        *,
        states: collections.abc.Iterable[ommx.v1.solution_pb2.State] | None = ...,
    ) -> None: ...
    def ClearField(self, field_name: typing.Literal["states", b"states"]) -> None: ...

global___States = States

@typing.final
class SampledValues(google.protobuf.message.Message):
    """A map from sample IDs to sampled values"""

    DESCRIPTOR: google.protobuf.descriptor.Descriptor

    @typing.final
    class ValuesEntry(google.protobuf.message.Message):
        DESCRIPTOR: google.protobuf.descriptor.Descriptor

        KEY_FIELD_NUMBER: builtins.int
        VALUE_FIELD_NUMBER: builtins.int
        key: builtins.int
        value: builtins.float
        def __init__(
            self,
            *,
            key: builtins.int = ...,
            value: builtins.float = ...,
        ) -> None: ...
        def ClearField(
            self, field_name: typing.Literal["key", b"key", "value", b"value"]
        ) -> None: ...

    VALUES_FIELD_NUMBER: builtins.int
    @property
    def values(
        self,
    ) -> google.protobuf.internal.containers.ScalarMap[
        builtins.int, builtins.float
    ]: ...
    def __init__(
        self,
        *,
        values: collections.abc.Mapping[builtins.int, builtins.float] | None = ...,
    ) -> None: ...
    def ClearField(self, field_name: typing.Literal["values", b"values"]) -> None: ...

global___SampledValues = SampledValues

@typing.final
class SampledDecisionVariable(google.protobuf.message.Message):
    """A pair of decision variable description and its sampled values"""

    DESCRIPTOR: google.protobuf.descriptor.Descriptor

    DECISION_VARIABLE_FIELD_NUMBER: builtins.int
    SAMPLES_FIELD_NUMBER: builtins.int
    @property
    def decision_variable(self) -> ommx.v1.decision_variables_pb2.DecisionVariable: ...
    @property
    def samples(self) -> global___SampledValues: ...
    def __init__(
        self,
        *,
        decision_variable: ommx.v1.decision_variables_pb2.DecisionVariable | None = ...,
        samples: global___SampledValues | None = ...,
    ) -> None: ...
    def HasField(
        self,
        field_name: typing.Literal[
            "decision_variable", b"decision_variable", "samples", b"samples"
        ],
    ) -> builtins.bool: ...
    def ClearField(
        self,
        field_name: typing.Literal[
            "decision_variable", b"decision_variable", "samples", b"samples"
        ],
    ) -> None: ...

global___SampledDecisionVariable = SampledDecisionVariable

@typing.final
class SampledConstraints(google.protobuf.message.Message):
    """Evaluated constraint for samples"""

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

    @typing.final
    class RemovedReasonParametersEntry(google.protobuf.message.Message):
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
    NAME_FIELD_NUMBER: builtins.int
    SUBSCRIPTS_FIELD_NUMBER: builtins.int
    PARAMETERS_FIELD_NUMBER: builtins.int
    DESCRIPTION_FIELD_NUMBER: builtins.int
    REMOVED_REASON_FIELD_NUMBER: builtins.int
    REMOVED_REASON_PARAMETERS_FIELD_NUMBER: builtins.int
    EVALUATED_VALUES_FIELD_NUMBER: builtins.int
    id: builtins.int
    """Constraint ID"""
    equality: ommx.v1.constraint_pb2.Equality.ValueType
    name: builtins.str
    """Name of the constraint."""
    description: builtins.str
    """Detail human-readable description of the constraint."""
    removed_reason: builtins.str
    """Short removed reason of the constraint. This field exists only if this message is evaluated from a removed constraint."""
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

    @property
    def removed_reason_parameters(
        self,
    ) -> google.protobuf.internal.containers.ScalarMap[builtins.str, builtins.str]:
        """Detailed parameters why the constraint is removed. This field exists only if this message is evaluated from a removed constraint."""

    @property
    def evaluated_values(self) -> global___SampledValues:
        """Evaluated values of constraint for each sample"""

    def __init__(
        self,
        *,
        id: builtins.int = ...,
        equality: ommx.v1.constraint_pb2.Equality.ValueType = ...,
        name: builtins.str | None = ...,
        subscripts: collections.abc.Iterable[builtins.int] | None = ...,
        parameters: collections.abc.Mapping[builtins.str, builtins.str] | None = ...,
        description: builtins.str | None = ...,
        removed_reason: builtins.str | None = ...,
        removed_reason_parameters: collections.abc.Mapping[builtins.str, builtins.str]
        | None = ...,
        evaluated_values: global___SampledValues | None = ...,
    ) -> None: ...
    def HasField(
        self,
        field_name: typing.Literal[
            "_description",
            b"_description",
            "_name",
            b"_name",
            "_removed_reason",
            b"_removed_reason",
            "description",
            b"description",
            "evaluated_values",
            b"evaluated_values",
            "name",
            b"name",
            "removed_reason",
            b"removed_reason",
        ],
    ) -> builtins.bool: ...
    def ClearField(
        self,
        field_name: typing.Literal[
            "_description",
            b"_description",
            "_name",
            b"_name",
            "_removed_reason",
            b"_removed_reason",
            "description",
            b"description",
            "equality",
            b"equality",
            "evaluated_values",
            b"evaluated_values",
            "id",
            b"id",
            "name",
            b"name",
            "parameters",
            b"parameters",
            "removed_reason",
            b"removed_reason",
            "removed_reason_parameters",
            b"removed_reason_parameters",
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
    @typing.overload
    def WhichOneof(
        self, oneof_group: typing.Literal["_removed_reason", b"_removed_reason"]
    ) -> typing.Literal["removed_reason"] | None: ...

global___SampledConstraints = SampledConstraints

@typing.final
class SampleSet(google.protobuf.message.Message):
    """Output of the sampling process."""

    DESCRIPTOR: google.protobuf.descriptor.Descriptor

    OBJECTIVES_FIELD_NUMBER: builtins.int
    DECISION_VARIABLES_FIELD_NUMBER: builtins.int
    CONSTRAINTS_FIELD_NUMBER: builtins.int
    @property
    def objectives(self) -> global___SampledValues: ...
    @property
    def decision_variables(
        self,
    ) -> google.protobuf.internal.containers.RepeatedCompositeFieldContainer[
        global___SampledDecisionVariable
    ]: ...
    @property
    def constraints(self) -> global___SampledConstraints: ...
    def __init__(
        self,
        *,
        objectives: global___SampledValues | None = ...,
        decision_variables: collections.abc.Iterable[global___SampledDecisionVariable]
        | None = ...,
        constraints: global___SampledConstraints | None = ...,
    ) -> None: ...
    def HasField(
        self,
        field_name: typing.Literal[
            "constraints", b"constraints", "objectives", b"objectives"
        ],
    ) -> builtins.bool: ...
    def ClearField(
        self,
        field_name: typing.Literal[
            "constraints",
            b"constraints",
            "decision_variables",
            b"decision_variables",
            "objectives",
            b"objectives",
        ],
    ) -> None: ...

global___SampleSet = SampleSet
