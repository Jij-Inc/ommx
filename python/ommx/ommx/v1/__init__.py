from __future__ import annotations
from typing import Optional, Iterable
from datetime import datetime
from dataclasses import dataclass, field
from pandas import DataFrame, concat, MultiIndex

from .solution_pb2 import State, Solution as _Solution
from .instance_pb2 import Instance as _Instance
from .function_pb2 import Function as _Function
from .quadratic_pb2 import Quadratic as _Quadratic
from .polynomial_pb2 import Polynomial as _Polynomial, Monomial as _Monomial
from .linear_pb2 import Linear as _Linear
from .constraint_pb2 import Equality, Constraint as _Constraint
from .decision_variables_pb2 import DecisionVariable as _DecisionVariable, Bound

from .._ommx_rust import evaluate_instance, used_decision_variable_ids


@dataclass
class Instance:
    """
    Idiomatic wrapper of ``ommx.v1.Instance`` protobuf message.

    Note that this class also contains annotations like :py:attr:`title` which are not contained in protobuf message but stored in OMMX artifact.
    These annotations are loaded from annotations while reading from OMMX artifact.

    Examples
    =========

    Create an instance for KnapSack Problem

    .. doctest::

        >>> from ommx.v1 import Instance, DecisionVariable

        Profit and weight of items

        >>> p = [10, 13, 18, 31, 7, 15]
        >>> w = [11, 15, 20, 35, 10, 33]

        Decision variables

        >>> x = [DecisionVariable.binary(i) for i in range(6)]

        Objective and constraint

        >>> objective = sum(p[i] * x[i] for i in range(6))
        >>> constraint = sum(w[i] * x[i] for i in range(6)) <= 47

        Compose as an instance

        >>> instance = Instance.from_components(
        ...     decision_variables=x,
        ...     objective=objective,
        ...     constraints=[constraint],
        ...     sense=Instance.MAXIMIZE,
        ... )

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
        objective: int
        | float
        | DecisionVariable
        | Linear
        | Quadratic
        | Polynomial
        | _Function,
        constraints: Iterable[Constraint | _Constraint],
        sense: _Instance.Sense.ValueType,
        decision_variables: Iterable[DecisionVariable | _DecisionVariable],
        description: Optional[_Instance.Description] = None,
    ) -> Instance:
        return Instance(
            _Instance(
                description=description,
                decision_variables=[
                    v.raw if isinstance(v, DecisionVariable) else v
                    for v in decision_variables
                ],
                objective=as_function(objective),
                constraints=[
                    c.raw if isinstance(c, Constraint) else c for c in constraints
                ],
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
        decision_variables = self.raw.decision_variables
        parameters = DataFrame(dict(v.parameters) for v in decision_variables)
        parameters.columns = MultiIndex.from_product(
            [["parameters"], parameters.columns]
        )
        df = DataFrame(
            {
                "id": v.id,
                "kind": _kind(v.kind),
                "lower": v.bound.lower,
                "upper": v.bound.upper,
                "name": v.name,
                "subscripts": v.subscripts,
                "description": v.description,
            }
            for v in decision_variables
        )
        df.columns = MultiIndex.from_product([df.columns, [""]])
        return concat([df, parameters], axis=1).set_index("id")

    @property
    def constraints(self) -> DataFrame:
        constraints = self.raw.constraints
        parameters = DataFrame(dict(v.parameters) for v in constraints)
        parameters.columns = MultiIndex.from_product(
            [["parameters"], parameters.columns]
        )
        df = DataFrame(
            {
                "id": c.id,
                "equality": _equality(c.equality),
                "type": _function_type(c.function),
                "used_ids": used_decision_variable_ids(c.function.SerializeToString()),
                "name": c.name,
                "subscripts": c.subscripts,
                "description": c.description,
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
        decision_variables = self.raw.decision_variables
        parameters = DataFrame(dict(v.parameters) for v in decision_variables)
        parameters.columns = MultiIndex.from_product(
            [["parameters"], parameters.columns]
        )
        df = DataFrame(
            {
                "id": v.id,
                "kind": _kind(v.kind),
                "value": self.raw.state.entries[v.id],
                "lower": v.bound.lower,
                "upper": v.bound.upper,
                "name": v.name,
                "subscripts": v.subscripts,
                "description": v.description,
            }
            for v in decision_variables
        )
        df.columns = MultiIndex.from_product([df.columns, [""]])
        return concat([df, parameters], axis=1).set_index("id")

    @property
    def constraints(self) -> DataFrame:
        evaluation = self.raw.evaluated_constraints
        parameters = DataFrame(dict(v.parameters) for v in evaluation)
        parameters.columns = MultiIndex.from_product(
            [["parameters"], parameters.columns]
        )
        df = DataFrame(
            {
                "id": v.id,
                "equality": _equality(v.equality),
                "value": v.evaluated_value,
                "used_ids": set(v.used_decision_variable_ids),
                "name": v.name,
                "subscripts": v.subscripts,
                "description": v.description,
            }
            for v in evaluation
        )
        df.columns = MultiIndex.from_product([df.columns, [""]])
        return concat([df, parameters], axis=1).set_index("id")


def _function_type(function: _Function) -> str:
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
        name: Optional[str] = None,
        subscripts: Optional[list[int]] = None,
        parameters: Optional[dict[str, str]] = None,
        description: Optional[str] = None,
    ) -> DecisionVariable:
        return DecisionVariable(
            _DecisionVariable(
                id=id,
                kind=kind,
                bound=Bound(lower=lower, upper=upper),
                name=name,
                subscripts=subscripts,
                parameters=parameters,
                description=description,
            )
        )

    @staticmethod
    def binary(
        id: int,
        *,
        name: Optional[str] = None,
        subscripts: Optional[list[int]] = None,
        parameters: Optional[dict[str, str]] = None,
        description: Optional[str] = None,
    ) -> DecisionVariable:
        return DecisionVariable(
            _DecisionVariable(
                id=id,
                kind=_DecisionVariable.Kind.KIND_BINARY,
                name=name,
                bound=Bound(lower=0, upper=1),
                subscripts=subscripts,
                parameters=parameters,
                description=description,
            )
        )

    @staticmethod
    def integer(
        id: int,
        *,
        lower: float = float("-inf"),
        upper: float = float("inf"),
        name: Optional[str] = None,
        subscripts: Optional[list[int]] = None,
        parameters: Optional[dict[str, str]] = None,
        description: Optional[str] = None,
    ) -> DecisionVariable:
        return DecisionVariable(
            _DecisionVariable(
                id=id,
                kind=_DecisionVariable.Kind.KIND_INTEGER,
                bound=Bound(lower=lower, upper=upper),
                name=name,
                subscripts=subscripts,
                parameters=parameters,
                description=description,
            )
        )

    @staticmethod
    def continuous(
        id: int,
        *,
        lower: float = float("-inf"),
        upper: float = float("inf"),
        name: Optional[str] = None,
        subscripts: Optional[list[int]] = None,
        parameters: Optional[dict[str, str]] = None,
        description: Optional[str] = None,
    ) -> DecisionVariable:
        return DecisionVariable(
            _DecisionVariable(
                id=id,
                kind=_DecisionVariable.Kind.KIND_CONTINUOUS,
                bound=Bound(lower=lower, upper=upper),
                name=name,
                subscripts=subscripts,
                parameters=parameters,
                description=description,
            )
        )

    @staticmethod
    def semi_integer(
        id: int,
        *,
        lower: float = float("-inf"),
        upper: float = float("inf"),
        name: Optional[str] = None,
        subscripts: Optional[list[int]] = None,
        parameters: Optional[dict[str, str]] = None,
        description: Optional[str] = None,
    ) -> DecisionVariable:
        return DecisionVariable(
            _DecisionVariable(
                id=id,
                kind=_DecisionVariable.Kind.KIND_SEMI_INTEGER,
                bound=Bound(lower=lower, upper=upper),
                name=name,
                subscripts=subscripts,
                parameters=parameters,
                description=description,
            )
        )

    @staticmethod
    def semi_continuous(
        id: int,
        *,
        lower: float = float("-inf"),
        upper: float = float("inf"),
        name: Optional[str] = None,
        subscripts: Optional[list[int]] = None,
        parameters: Optional[dict[str, str]] = None,
        description: Optional[str] = None,
    ) -> DecisionVariable:
        return DecisionVariable(
            _DecisionVariable(
                id=id,
                kind=_DecisionVariable.Kind.KIND_SEMI_CONTINUOUS,
                bound=Bound(lower=lower, upper=upper),
                name=name,
                subscripts=subscripts,
                parameters=parameters,
                description=description,
            )
        )

    @property
    def kind(self) -> Kind:
        return self.raw.kind

    @property
    def bound(self) -> Bound:
        return self.raw.bound

    def equals_to(self, other: DecisionVariable) -> bool:
        """
        Alternative to ``==`` operator to compare two decision variables.
        """
        return self.raw == other.raw

    def __add__(self, other: int | float | DecisionVariable) -> Linear:
        if isinstance(other, float) or isinstance(other, int):
            return Linear(terms={self.raw.id: 1}, constant=other)
        if isinstance(other, DecisionVariable):
            if self.raw.id == other.raw.id:
                return Linear(terms={self.raw.id: 2})
            else:
                return Linear(terms={self.raw.id: 1, other.raw.id: 1})
        return NotImplemented

    def __sub__(self, other) -> Linear:
        return self + (-other)

    def __neg__(self) -> Linear:
        return Linear(terms={self.raw.id: -1})

    def __radd__(self, other) -> Linear:
        return self + other

    def __rsub__(self, other) -> Linear:
        return -self + other

    def __mul__(self, other: int | float) -> Linear:
        if isinstance(other, float) or isinstance(other, int):
            return Linear(terms={self.raw.id: other})
        return NotImplemented

    def __rmul__(self, other) -> Linear:
        return self * other

    def __eq__(self, other) -> Constraint:  # type: ignore[reportGeneralTypeIssues]
        """
        Create a constraint that this decision variable is equal to another decision variable or a constant.

        To compare two objects, use :py:meth:`equals_to` method.

        Examples
        ========

        >>> x = DecisionVariable.integer(1)
        >>> x == 1
        Constraint(...)

        """
        return Constraint(
            function=self - other, equality=Equality.EQUALITY_EQUAL_TO_ZERO
        )

    def __le__(self, other) -> Constraint:
        return Constraint(
            function=self - other, equality=Equality.EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO
        )

    def __ge__(self, other) -> Constraint:
        return Constraint(
            function=other - self, equality=Equality.EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO
        )

    def __req__(self, other) -> Constraint:
        return self == other

    def __rle__(self, other) -> Constraint:
        return self.__ge__(other)

    def __rge__(self, other) -> Constraint:
        return self.__le__(other)


@dataclass
class Linear:
    raw: _Linear

    def equals_to(self, other: Linear) -> bool:
        """
        Alternative to ``==`` operator to compare two linear functions.
        """
        return self.raw == other.raw

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

    def __sub__(self, other) -> Linear:
        return self + (-other)

    def __radd__(self, other) -> Linear:
        return self + other

    def __rsub__(self, other) -> Linear:
        return -self + other

    def __mul__(self, other: int | float) -> Linear:
        if isinstance(other, float) or isinstance(other, int):
            return Linear(
                terms={term.id: term.coefficient * other for term in self.raw.terms},
                constant=self.raw.constant * other,
            )
        return NotImplemented

    def __rmul__(self, other) -> Linear:
        return self * other

    def __neg__(self) -> Linear:
        return -1 * self

    def __eq__(self, other) -> Constraint:  # type: ignore[reportGeneralTypeIssues]
        """
        Create a constraint that this linear function is equal to the right-hand side.

        Examples
        ========

        >>> x = DecisionVariable.integer(1)
        >>> y = DecisionVariable.integer(2)
        >>> x + y == 1
        Constraint(...)

        To compare two objects, use :py:meth:`equals_to` method.

        >>> assert (x + y).equals_to(Linear(terms={1: 1, 2: 1}))

        """
        return Constraint(
            function=self - other, equality=Equality.EQUALITY_EQUAL_TO_ZERO
        )

    def __le__(self, other) -> Constraint:
        return Constraint(
            function=self - other, equality=Equality.EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO
        )

    def __ge__(self, other) -> Constraint:
        return Constraint(
            function=other - self, equality=Equality.EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO
        )

    def __req__(self, other) -> Constraint:
        return self == other

    def __rle__(self, other) -> Constraint:
        return self.__ge__(other)

    def __rge__(self, other) -> Constraint:
        return self.__le__(other)


@dataclass
class Quadratic:
    raw: _Quadratic

    def __init__(
        self,
        *,
        columns: Iterable[int],
        raws: Iterable[int],
        values: Iterable[float | int],
        linear: Optional[Linear] = None,
    ):
        self.raw = _Quadratic(
            columns=columns,
            rows=raws,
            values=values,
            linear=linear.raw if linear else None,
        )

    # TODO: Implement __add__, __radd__, __mul__, __rmul__


@dataclass
class Polynomial:
    raw: _Polynomial

    def __init__(self, *, coefficients: Iterable[tuple[Iterable[int], float | int]]):
        self.raw = _Polynomial(
            terms=[
                _Monomial(ids=ids, coefficient=coefficient)
                for ids, coefficient in coefficients
            ]
        )

    # TODO: Implement __add__, __radd__, __mul__, __rmul__


def as_function(
    f: int | float | DecisionVariable | Linear | Quadratic | Polynomial | _Function,
) -> _Function:
    if isinstance(f, (int, float)):
        return _Function(constant=f)
    elif isinstance(f, DecisionVariable):
        return _Function(linear=Linear(terms={f.raw.id: 1}).raw)
    elif isinstance(f, Linear):
        return _Function(linear=f.raw)
    elif isinstance(f, Quadratic):
        return _Function(quadratic=f.raw)
    elif isinstance(f, Polynomial):
        return _Function(polynomial=f.raw)
    elif isinstance(f, _Function):
        return f
    else:
        raise ValueError(f"Unknown function type: {type(f)}")


@dataclass
class Constraint:
    """
    Constraints

    Examples
    =========

    .. doctest::

        >>> x = DecisionVariable.integer(1)
        >>> y = DecisionVariable.integer(2)
        >>> x + y == 1
        Constraint(...)

        To set the name or other attributes, use methods like :py:meth:`add_name`.

        >>> (x + y <= 5).add_name("constraint 1")
        Constraint(...
        name: "constraint 1"
        )

    """

    raw: _Constraint
    _counter = 0

    EQUAL_TO_ZERO = Equality.EQUALITY_EQUAL_TO_ZERO
    LESS_THAN_OR_EQUAL_TO_ZERO = Equality.EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO

    def __init__(
        self,
        *,
        function: int | float | DecisionVariable | Linear | Quadratic | Polynomial,
        equality: Equality.ValueType,
        name: Optional[str] = None,
        description: Optional[str] = None,
        subscripts: Optional[list[int]] = None,
        parameters: Optional[dict[str, str]] = None,
    ):
        self.raw = _Constraint(
            id=Constraint._counter,
            function=as_function(function),
            equality=equality,
            name=name,
            description=description,
            subscripts=subscripts,
            parameters=parameters,
        )
        Constraint._counter += 1

    def set_id(self, id: int) -> Constraint:
        """
        Overwrite the constraint ID.
        """
        self.raw.id = id
        return self

    def add_name(self, name: str) -> Constraint:
        self.raw.name = name
        return self

    def add_description(self, description: str) -> Constraint:
        self.raw.description = description
        return self

    def add_subscripts(self, subscripts: list[int]) -> Constraint:
        self.raw.subscripts.extend(subscripts)
        return self

    def add_parameters(self, parameters: dict[str, str]) -> Constraint:
        self.raw.parameters.update(parameters)
        return self
