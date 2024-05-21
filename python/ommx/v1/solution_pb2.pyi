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
import typing

DESCRIPTOR: google.protobuf.descriptor.FileDescriptor

@typing.final
class RawSolution(google.protobuf.message.Message):
    """Pure solution state without any evaluation, even the feasiblity of the solution."""

    DESCRIPTOR: google.protobuf.descriptor.Descriptor

    @typing.final
    class EntriesEntry(google.protobuf.message.Message):
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

    ENTRIES_FIELD_NUMBER: builtins.int
    @property
    def entries(
        self,
    ) -> google.protobuf.internal.containers.ScalarMap[builtins.int, builtins.float]:
        """The value of the solution for each variable ID."""

    def __init__(
        self,
        *,
        entries: collections.abc.Mapping[builtins.int, builtins.float] | None = ...,
    ) -> None: ...
    def ClearField(self, field_name: typing.Literal["entries", b"entries"]) -> None: ...

global___RawSolution = RawSolution

@typing.final
class RawSolutionList(google.protobuf.message.Message):
    """List of RawSolution"""

    DESCRIPTOR: google.protobuf.descriptor.Descriptor

    SOLUTIONS_FIELD_NUMBER: builtins.int
    @property
    def solutions(
        self,
    ) -> google.protobuf.internal.containers.RepeatedCompositeFieldContainer[
        global___RawSolution
    ]: ...
    def __init__(
        self,
        *,
        solutions: collections.abc.Iterable[global___RawSolution] | None = ...,
    ) -> None: ...
    def ClearField(
        self, field_name: typing.Literal["solutions", b"solutions"]
    ) -> None: ...

global___RawSolutionList = RawSolutionList

@typing.final
class Solution(google.protobuf.message.Message):
    """Solution with evaluated objective and constraints"""

    DESCRIPTOR: google.protobuf.descriptor.Descriptor

    RAW_SOLUTION_FIELD_NUMBER: builtins.int
    OBJECTIVE_FIELD_NUMBER: builtins.int
    DECISION_VARIABLES_FIELD_NUMBER: builtins.int
    EVALUATED_CONSTRAINTS_FIELD_NUMBER: builtins.int
    FEASIBLE_FIELD_NUMBER: builtins.int
    OPTIMAL_FIELD_NUMBER: builtins.int
    objective: builtins.float
    feasible: builtins.bool
    """Whether the solution is feasible, i.e. all constraints are satisfied or not."""
    optimal: builtins.bool
    """Whether the solution is optimal. This field is optional and should be used only by the solvers which can guarantee the optimality."""
    @property
    def raw_solution(self) -> global___RawSolution: ...
    @property
    def decision_variables(
        self,
    ) -> google.protobuf.internal.containers.RepeatedCompositeFieldContainer[
        ommx.v1.decision_variables_pb2.DecisionVariable
    ]: ...
    @property
    def evaluated_constraints(
        self,
    ) -> google.protobuf.internal.containers.RepeatedCompositeFieldContainer[
        ommx.v1.constraint_pb2.EvaluatedConstraint
    ]: ...
    def __init__(
        self,
        *,
        raw_solution: global___RawSolution | None = ...,
        objective: builtins.float = ...,
        decision_variables: collections.abc.Iterable[
            ommx.v1.decision_variables_pb2.DecisionVariable
        ]
        | None = ...,
        evaluated_constraints: collections.abc.Iterable[
            ommx.v1.constraint_pb2.EvaluatedConstraint
        ]
        | None = ...,
        feasible: builtins.bool = ...,
        optimal: builtins.bool | None = ...,
    ) -> None: ...
    def HasField(
        self,
        field_name: typing.Literal[
            "_optimal",
            b"_optimal",
            "optimal",
            b"optimal",
            "raw_solution",
            b"raw_solution",
        ],
    ) -> builtins.bool: ...
    def ClearField(
        self,
        field_name: typing.Literal[
            "_optimal",
            b"_optimal",
            "decision_variables",
            b"decision_variables",
            "evaluated_constraints",
            b"evaluated_constraints",
            "feasible",
            b"feasible",
            "objective",
            b"objective",
            "optimal",
            b"optimal",
            "raw_solution",
            b"raw_solution",
        ],
    ) -> None: ...
    def WhichOneof(
        self, oneof_group: typing.Literal["_optimal", b"_optimal"]
    ) -> typing.Literal["optimal"] | None: ...

global___Solution = Solution

@typing.final
class SolutionList(google.protobuf.message.Message):
    """List of Solution"""

    DESCRIPTOR: google.protobuf.descriptor.Descriptor

    SOLUTIONS_FIELD_NUMBER: builtins.int
    @property
    def solutions(
        self,
    ) -> google.protobuf.internal.containers.RepeatedCompositeFieldContainer[
        global___Solution
    ]: ...
    def __init__(
        self,
        *,
        solutions: collections.abc.Iterable[global___Solution] | None = ...,
    ) -> None: ...
    def ClearField(
        self, field_name: typing.Literal["solutions", b"solutions"]
    ) -> None: ...

global___SolutionList = SolutionList
