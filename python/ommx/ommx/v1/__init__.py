from __future__ import annotations
from typing import Optional, Iterable
from datetime import datetime
from dataclasses import dataclass, field
from pandas import DataFrame, concat, MultiIndex

from .solution_pb2 import State, Solution as _Solution
from .instance_pb2 import Instance as _Instance
from .function_pb2 import Function
from .linear_pb2 import Linear as _Linear
from .constraint_pb2 import Equality, Constraint
from .decision_variables_pb2 import DecisionVariable as _DecisionVariable, Bound

from .._ommx_rust import evaluate_instance, used_decision_variable_ids


@dataclass
class Instance:
    """
    Idiomatic wrapper of ``ommx.v1.Instance`` protobuf message.

    Note that this class also contains annotations like :py:attr:`title` which are not contained in protobuf message but stored in OMMX artifact.
    These annotations are loaded from annotations while reading from OMMX artifact.
    """

    raw: _Instance
    """The raw protobuf message."""

    title: Optional[str] = None
    """
    The title of the instance, stored as ``org.ommx.v1.instance.title`` annotation in OMMX artifact.
    """
    created: Optional[datetime] = None
    """
    The creation date of the instance, stored as ``org.ommx.v1.instance.created`` annotation in RFC3339 format in OMMX artifact.
    """
    annotations: dict[str, str] = field(default_factory=dict)
    """
    Arbitrary annotations stored in OMMX artifact. Use :py:attr:`title` or other specific attributes if possible.
    """

    # Re-export some enums
    MAXIMIZE = _Instance.SENSE_MAXIMIZE
    MINIMIZE = _Instance.SENSE_MINIMIZE

    Description = _Instance.Description

    @staticmethod
    def from_components(
        *,
        objective: Function,
        constraints: Iterable[Constraint],
        sense: _Instance.Sense.ValueType,
        decision_variables: Iterable[DecisionVariable],
        description: Optional[_Instance.Description] = None,
    ) -> Instance:
        return Instance(
            _Instance(
                description=description,
                decision_variables=[v.raw for v in decision_variables],
                objective=objective,
                constraints=constraints,
                sense=sense,
            )
        )

    @staticmethod
    def from_bytes(data: bytes) -> Instance:
        instance = _Instance()
        instance.ParseFromString(data)
        return Instance(instance)

    def to_bytes(self) -> bytes:
        return self.raw.SerializeToString()

    @property
    def decision_variables(self) -> DataFrame:
        return _decision_variables(self.raw)

    @property
    def constraints(self) -> DataFrame:
        constraints = self.raw.constraints
        parameters = DataFrame(dict(v.description.parameters) for v in constraints)
        parameters.columns = MultiIndex.from_product(
            [["parameters"], parameters.columns]
        )
        df = DataFrame(
            {
                "id": c.id,
                "equality": _equality(c.equality),
                "type": _function_type(c.function),
                "used_ids": used_decision_variable_ids(c.function.SerializeToString()),
                "name": c.description.name,
            }
            for c in constraints
        )
        df.columns = MultiIndex.from_product([df.columns, [""]])
        return concat([df, parameters], axis=1).set_index("id")

    def evaluate(self, state: State) -> Solution:
        out, _ = evaluate_instance(self.to_bytes(), state.SerializeToString())
        return Solution.from_bytes(out)


@dataclass
class Solution:
    """
    Idiomatic wrapper of ``ommx.v1.Solution`` protobuf message.

    This also contains annotations not contained in protobuf message, and will be stored in OMMX artifact.
    """

    raw: _Solution
    """The raw protobuf message."""

    instance: Optional[str] = None
    """
    The digest of the instance layer, stored as ``org.ommx.v1.solution.instance`` annotation in OMMX artifact.

    This ``Solution`` is the solution of the mathematical programming problem described by the instance.
    """

    solver: Optional[object] = None
    """
    The solver which generated this solution, stored as ``org.ommx.v1.solution.solver`` annotation as a JSON in OMMX artifact.
    """

    parameters: Optional[object] = None
    """
    The parameters used in the optimization, stored as ``org.ommx.v1.solution.parameters`` annotation as a JSON in OMMX artifact.
    """

    start: Optional[datetime] = None
    """
    When the optimization started, stored as ``org.ommx.v1.solution.start`` annotation in RFC3339 format in OMMX artifact.
    """

    end: Optional[datetime] = None
    """
    When the optimization ended, stored as ``org.ommx.v1.solution.end`` annotation in RFC3339 format in OMMX artifact.
    """

    annotations: dict[str, str] = field(default_factory=dict)
    """
    Arbitrary annotations stored in OMMX artifact. Use :py:attr:`parameters` or other specific attributes if possible.
    """

    @staticmethod
    def from_bytes(data: bytes) -> Solution:
        raw = _Solution()
        raw.ParseFromString(data)
        return Solution(raw)

    def to_bytes(self) -> bytes:
        return self.raw.SerializeToString()

    @property
    def decision_variables(self) -> DataFrame:
        return _decision_variables(self.raw)

    @property
    def constraints(self) -> DataFrame:
        evaluation = self.raw.evaluated_constraints
        parameters = DataFrame(dict(v.description.parameters) for v in evaluation)
        parameters.columns = MultiIndex.from_product(
            [["parameters"], parameters.columns]
        )
        df = DataFrame(
            {
                "id": v.id,
                "equality": _equality(v.equality),
                "value": v.evaluated_value,
                "used_ids": set(v.used_decision_variable_ids),
                "name": v.description.name,
            }
            for v in evaluation
        )
        df.columns = MultiIndex.from_product([df.columns, [""]])
        return concat([df, parameters], axis=1).set_index("id")


def _decision_variables(obj: _Instance | _Solution) -> DataFrame:
    decision_variables = obj.decision_variables
    parameters = DataFrame(dict(v.description.parameters) for v in decision_variables)
    parameters.columns = MultiIndex.from_product([["parameters"], parameters.columns])
    df = DataFrame(
        {
            "id": v.id,
            "kind": _kind(v.kind),
            "lower": v.bound.lower,
            "upper": v.bound.upper,
            "name": v.description.name,
        }
        for v in decision_variables
    )
    df.columns = MultiIndex.from_product([df.columns, [""]])
    return concat([df, parameters], axis=1).set_index("id")


def _function_type(function: Function) -> str:
    if function.HasField("constant"):
        return "constant"
    if function.HasField("linear"):
        return "linear"
    if function.HasField("quadratic"):
        return "quadratic"
    if function.HasField("polynomial"):
        return "polynomial"
    raise ValueError("Unknown function type")


def _kind(kind: _DecisionVariable.Kind.ValueType) -> str:
    if kind == _DecisionVariable.Kind.KIND_UNSPECIFIED:
        return "unspecified"
    if kind == _DecisionVariable.Kind.KIND_BINARY:
        return "binary"
    if kind == _DecisionVariable.Kind.KIND_INTEGER:
        return "integer"
    if kind == _DecisionVariable.Kind.KIND_CONTINUOUS:
        return "continuous"
    if kind == _DecisionVariable.Kind.KIND_SEMI_INTEGER:
        return "semi-integer"
    if kind == _DecisionVariable.Kind.KIND_SEMI_CONTINUOUS:
        return "semi-continuous"
    raise ValueError("Unknown kind")


def _equality(equality: Equality.ValueType) -> str:
    if equality == Equality.EQUALITY_EQUAL_TO_ZERO:
        return "=0"
    if equality == Equality.EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO:
        return "<=0"
    raise ValueError("Unknown equality")


@dataclass
class DecisionVariable:
    raw: _DecisionVariable

    Kind = _DecisionVariable.Kind.ValueType
    Description = _DecisionVariable.Description

    BINARY = _DecisionVariable.Kind.KIND_BINARY
    INTEGER = _DecisionVariable.Kind.KIND_INTEGER
    CONTINUOUS = _DecisionVariable.Kind.KIND_CONTINUOUS
    SEMI_INTEGER = _DecisionVariable.Kind.KIND_SEMI_INTEGER
    SEMI_CONTINUOUS = _DecisionVariable.Kind.KIND_SEMI_CONTINUOUS

    @staticmethod
    def of_type(
        kind: Kind,
        id: int,
        *,
        lower: float,
        upper: float,
        description: Optional[Description] = None,
    ) -> DecisionVariable:
        return DecisionVariable(
            _DecisionVariable(
                id=id,
                kind=kind,
                bound=Bound(lower=lower, upper=upper),
                description=description,
            )
        )

    @staticmethod
    def binary(
        id: int, *, description: Optional[Description] = None
    ) -> DecisionVariable:
        return DecisionVariable(
            _DecisionVariable(
                id=id,
                kind=_DecisionVariable.Kind.KIND_BINARY,
                description=description,
            )
        )

    @staticmethod
    def integer(
        id: int,
        *,
        lower: float = float("-inf"),
        upper: float = float("inf"),
        description: Optional[Description] = None,
    ) -> DecisionVariable:
        return DecisionVariable(
            _DecisionVariable(
                id=id,
                kind=_DecisionVariable.Kind.KIND_INTEGER,
                bound=Bound(lower=lower, upper=upper),
                description=description,
            )
        )

    @staticmethod
    def continuous(
        id: int,
        *,
        lower: float = float("-inf"),
        upper: float = float("inf"),
        description: Optional[Description] = None,
    ) -> DecisionVariable:
        return DecisionVariable(
            _DecisionVariable(
                id=id,
                kind=_DecisionVariable.Kind.KIND_CONTINUOUS,
                bound=Bound(lower=lower, upper=upper),
                description=description,
            )
        )

    @staticmethod
    def semi_integer(
        id: int,
        *,
        lower: float = float("-inf"),
        upper: float = float("inf"),
        description: Optional[Description] = None,
    ) -> DecisionVariable:
        return DecisionVariable(
            _DecisionVariable(
                id=id,
                kind=_DecisionVariable.Kind.KIND_SEMI_INTEGER,
                bound=Bound(lower=lower, upper=upper),
                description=description,
            )
        )

    @staticmethod
    def semi_continuous(
        id: int,
        *,
        lower: float = float("-inf"),
        upper: float = float("inf"),
        description: Optional[Description] = None,
    ) -> DecisionVariable:
        return DecisionVariable(
            _DecisionVariable(
                id=id,
                kind=_DecisionVariable.Kind.KIND_SEMI_CONTINUOUS,
                bound=Bound(lower=lower, upper=upper),
                description=description,
            )
        )

    @property
    def kind(self) -> Kind:
        return self.raw.kind

    @property
    def bound(self) -> Bound:
        return self.raw.bound

    def __add__(self, other: int | float | DecisionVariable) -> Linear:
        if isinstance(other, float) or isinstance(other, int):
            return Linear(terms={self.raw.id: 1}, constant=other)
        if isinstance(other, DecisionVariable):
            if self.raw.id == other.raw.id:
                return Linear(terms={self.raw.id: 2})
            else:
                return Linear(terms={self.raw.id: 1, other.raw.id: 1})
        return NotImplemented

    def __radd__(self, other) -> Linear:
        return self + other

    def __mul__(self, other: int | float) -> Linear:
        if isinstance(other, float) or isinstance(other, int):
            return Linear(terms={self.raw.id: other})
        return NotImplemented

    def __rmul__(self, other) -> Linear:
        return self * other


@dataclass
class Linear:
    raw: _Linear

    def __init__(self, *, terms: dict[int, float | int], constant: float | int = 0):
        self.raw = _Linear(
            terms=[
                _Linear.Term(id=id, coefficient=coefficient)
                for id, coefficient in terms.items()
            ],
            constant=constant,
        )

    def __add__(self, other: int | float | DecisionVariable | Linear) -> Linear:
        if isinstance(other, float) or isinstance(other, int):
            self.raw.constant += other
            return self
        if isinstance(other, DecisionVariable):
            terms = {term.id: term.coefficient for term in self.raw.terms}
            terms[other.raw.id] = terms.get(other.raw.id, 0) + 1
            return Linear(terms=terms, constant=self.raw.constant)
        if isinstance(other, Linear):
            terms = {term.id: term.coefficient for term in self.raw.terms}
            for term in other.raw.terms:
                terms[term.id] = terms.get(term.id, 0) + term.coefficient
            return Linear(terms=terms, constant=self.raw.constant + other.raw.constant)
        return NotImplemented

    def __radd__(self, other) -> Linear:
        return self + other

    def __mul__(self, other: int | float) -> Linear:
        if isinstance(other, float) or isinstance(other, int):
            return Linear(
                terms={term.id: term.coefficient * other for term in self.raw.terms},
                constant=self.raw.constant * other,
            )
        return NotImplemented

    def __rmul__(self, other) -> Linear:
        return self * other
