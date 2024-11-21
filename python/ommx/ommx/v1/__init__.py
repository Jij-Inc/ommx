from __future__ import annotations
from typing import Optional, Iterable, overload
from typing_extensions import deprecated
from datetime import datetime
from dataclasses import dataclass, field
from pandas import DataFrame, concat, MultiIndex

from .solution_pb2 import State, Optimality, Relaxation, Solution as _Solution
from .instance_pb2 import Instance as _Instance
from .function_pb2 import Function as _Function
from .quadratic_pb2 import Quadratic as _Quadratic
from .polynomial_pb2 import Polynomial as _Polynomial, Monomial as _Monomial
from .linear_pb2 import Linear as _Linear
from .constraint_pb2 import Equality, Constraint as _Constraint
from .decision_variables_pb2 import DecisionVariable as _DecisionVariable, Bound

from .. import _ommx_rust


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

    # Annotations
    title: Optional[str] = None
    """
    The title of the instance, stored as ``org.ommx.v1.instance.title`` annotation in OMMX artifact.
    """
    created: Optional[datetime] = None
    """
    The creation date of the instance, stored as ``org.ommx.v1.instance.created`` annotation in RFC3339 format in OMMX artifact.
    """
    authors: list[str] = field(default_factory=list)
    """
    Authors of this instance. This is stored as ``org.ommx.v1.instance.authors`` annotation in OMMX artifact.
    """
    license: Optional[str] = None
    """
    License of this instance in the SPDX license identifier. This is stored as ``org.ommx.v1.instance.license`` annotation in OMMX artifact.
    """
    dataset: Optional[str] = None
    """
    Dataset name which this instance belongs to, stored as ``org.ommx.v1.instance.dataset`` annotation in OMMX artifact.
    """
    num_variables: Optional[int] = None
    """
    Number of variables in this instance, stored as ``org.ommx.v1.instance.variables`` annotation in OMMX artifact.
    """
    num_constraints: Optional[int] = None
    """
    Number of constraints in this instance, stored as ``org.ommx.v1.instance.constraints`` annotation in OMMX artifact.
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
    def load_mps(path: str) -> Instance:
        bytes = _ommx_rust.load_mps_bytes(path)
        return Instance.from_bytes(bytes)

    def write_mps(self, path: str):
        _ommx_rust.write_mps_file(self.to_bytes(), path)

    @staticmethod
    def from_bytes(data: bytes) -> Instance:
        instance = _Instance()
        instance.ParseFromString(data)
        return Instance(instance)

    def to_bytes(self) -> bytes:
        return self.raw.SerializeToString()

    @property
    def description(self) -> _Instance.Description:
        return self.raw.description

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
    def objective(self) -> Function:
        return Function(self.raw.objective)

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
                "used_ids": _ommx_rust.used_decision_variable_ids(
                    c.function.SerializeToString()
                ),
                "name": c.name,
                "subscripts": c.subscripts,
                "description": c.description,
            }
            for c in constraints
        )
        df.columns = MultiIndex.from_product([df.columns, [""]])
        return concat([df, parameters], axis=1).set_index("id")

    @property
    def sense(self) -> _Instance.Sense.ValueType:
        return self.raw.sense

    def evaluate(self, state: State) -> Solution:
        out, _ = _ommx_rust.evaluate_instance(
            self.to_bytes(), state.SerializeToString()
        )
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
    def state(self) -> State:
        return self.raw.state

    @property
    def objective(self) -> float:
        return self.raw.objective

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

    @property
    def feasible(self) -> bool:
        return self.raw.feasible

    @property
    def optimality(self) -> Optimality.ValueType:
        return self.raw.optimality

    @property
    def relaxation(self) -> Relaxation.ValueType:
        return self.raw.relaxation


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
    def from_bytes(data: bytes) -> DecisionVariable:
        new = DecisionVariable(_DecisionVariable())
        new.raw.ParseFromString(data)
        return new

    def to_bytes(self) -> bytes:
        return self.raw.SerializeToString()

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
    def id(self) -> int:
        return self.raw.id

    @property
    def name(self) -> str:
        return self.raw.name

    @property
    def kind(self) -> Kind:
        return self.raw.kind

    @property
    def bound(self) -> Bound:
        return self.raw.bound

    @property
    def subscripts(self) -> list[int]:
        return list(self.raw.subscripts)

    @property
    def parameters(self) -> dict[str, str]:
        return dict(self.raw.parameters)

    @property
    def description(self) -> str:
        return self.raw.description

    def equals_to(self, other: DecisionVariable) -> bool:
        """
        Alternative to ``==`` operator to compare two decision variables.
        """
        return self.raw == other.raw

    def __repr__(self) -> str:
        return f"x{self.raw.id}"

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

    @overload
    def __mul__(self, other: int | float) -> Linear: ...

    @overload
    def __mul__(self, other: DecisionVariable) -> Quadratic: ...

    def __mul__(self, other: int | float | DecisionVariable) -> Linear | Quadratic:
        if isinstance(other, float) or isinstance(other, int):
            return Linear(terms={self.raw.id: other})
        if isinstance(other, DecisionVariable):
            return Quadratic(columns=[self.raw.id], rows=[other.raw.id], values=[1.0])
        return NotImplemented

    def __rmul__(self, other):
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
    """
    Modeler API for linear function

    This is a wrapper of :class:`linear_pb2.Linear` protobuf message.

    Examples
    =========

    .. doctest::

        Create a linear function :math:`f(x_1, x_2) = 2 x_1 + 3 x_2 + 1`
        >>> f = Linear(terms={1: 2, 2: 3}, constant=1)

        Or create via DecisionVariable
        >>> x1 = DecisionVariable.integer(1)
        >>> x2 = DecisionVariable.integer(2)
        >>> g = 2*x1 + 3*x2 + 1

        Compare two linear functions are equal in terms of a polynomial with tolerance
        >>> assert f.almost_equal(g, atol=1e-12)

        Note that `f == g` becomes an equality `Constraint`
        >>> assert isinstance(f == g, Constraint)

    """

    raw: _Linear

    def __init__(self, *, terms: dict[int, float | int], constant: float | int = 0):
        self.raw = _Linear(
            terms=[
                _Linear.Term(id=id, coefficient=coefficient)
                for id, coefficient in terms.items()
            ],
            constant=constant,
        )

    @staticmethod
    def from_bytes(data: bytes) -> Linear:
        new = Linear(terms={})
        new.raw.ParseFromString(data)
        return new

    def to_bytes(self) -> bytes:
        return self.raw.SerializeToString()

    @deprecated("Use almost_equal method instead.")
    def equals_to(self, other: Linear) -> bool:
        """
        Alternative to ``==`` operator to compare two linear functions.
        """
        return self.raw == other.raw

    def almost_equal(self, other: Linear, *, atol: float = 1e-10) -> bool:
        """
        Compare two linear functions have almost equal coefficients and constant.
        """
        lhs = _ommx_rust.Linear.decode(self.raw.SerializeToString())
        rhs = _ommx_rust.Linear.decode(other.raw.SerializeToString())
        return lhs.almost_equal(rhs, atol)

    def __repr__(self) -> str:
        return f"Linear({_ommx_rust.Linear.decode(self.raw.SerializeToString()).__repr__()})"

    def __add__(self, other: int | float | DecisionVariable | Linear) -> Linear:
        if isinstance(other, float) or isinstance(other, int):
            self.raw.constant += other
            return self
        if isinstance(other, DecisionVariable):
            new = _ommx_rust.Linear.decode(self.raw.SerializeToString())
            rhs = _ommx_rust.Linear.single_term(other.raw.id, 1)
            return Linear.from_bytes((new + rhs).encode())
        if isinstance(other, Linear):
            new = _ommx_rust.Linear.decode(self.raw.SerializeToString())
            rhs = _ommx_rust.Linear.decode(other.raw.SerializeToString())
            return Linear.from_bytes((new + rhs).encode())
        return NotImplemented

    def __radd__(self, other):
        return self + other

    def __sub__(self, other: int | float | DecisionVariable | Linear) -> Linear:
        if isinstance(other, (int, float, DecisionVariable, Linear)):
            return self + (-other)
        return NotImplemented

    def __rsub__(self, other):
        return -self + other

    @overload
    def __mul__(self, other: int | float) -> Linear: ...

    @overload
    def __mul__(self, other: DecisionVariable | Linear) -> Quadratic: ...

    def __mul__(
        self, other: int | float | DecisionVariable | Linear
    ) -> Linear | Quadratic:
        if isinstance(other, float) or isinstance(other, int):
            new = _ommx_rust.Linear.decode(self.raw.SerializeToString())
            return Linear.from_bytes(new.mul_scalar(other).encode())
        if isinstance(other, DecisionVariable):
            new = _ommx_rust.Linear.decode(self.raw.SerializeToString())
            rhs = _ommx_rust.Linear.single_term(other.raw.id, 1)
            return Quadratic.from_bytes((new * rhs).encode())
        if isinstance(other, Linear):
            new = _ommx_rust.Linear.decode(self.raw.SerializeToString())
            rhs = _ommx_rust.Linear.decode(other.raw.SerializeToString())
            return Quadratic.from_bytes((new * rhs).encode())
        return NotImplemented

    def __rmul__(self, other):
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

        To compare two objects, use :py:meth:`almost_equal` method.

        >>> assert (x + y).almost_equal(Linear(terms={1: 1, 2: 1}))

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
        rows: Iterable[int],
        values: Iterable[float | int],
        linear: Optional[Linear] = None,
    ):
        self.raw = _Quadratic(
            columns=columns,
            rows=rows,
            values=values,
            linear=linear.raw if linear else None,
        )

    @staticmethod
    def from_bytes(data: bytes) -> Quadratic:
        new = Quadratic(columns=[], rows=[], values=[])
        new.raw.ParseFromString(data)
        return new

    def to_bytes(self) -> bytes:
        return self.raw.SerializeToString()

    def almost_equal(self, other: Quadratic, *, atol: float = 1e-10) -> bool:
        """
        Compare two quadratic functions have almost equal coefficients
        """
        lhs = _ommx_rust.Quadratic.decode(self.raw.SerializeToString())
        rhs = _ommx_rust.Quadratic.decode(other.raw.SerializeToString())
        return lhs.almost_equal(rhs, atol)

    def __repr__(self) -> str:
        return f"Quadratic({_ommx_rust.Quadratic.decode(self.raw.SerializeToString()).__repr__()})"

    def __add__(
        self, other: int | float | DecisionVariable | Linear | Quadratic
    ) -> Quadratic:
        if isinstance(other, float) or isinstance(other, int):
            self.raw.linear.constant += other
            return self
        if isinstance(other, DecisionVariable):
            new = _ommx_rust.Quadratic.decode(self.raw.SerializeToString())
            rhs = _ommx_rust.Linear.single_term(other.raw.id, 1)
            return Quadratic.from_bytes((new.add_linear(rhs)).encode())
        if isinstance(other, Linear):
            new = _ommx_rust.Quadratic.decode(self.raw.SerializeToString())
            rhs = _ommx_rust.Linear.decode(other.raw.SerializeToString())
            return Quadratic.from_bytes((new.add_linear(rhs)).encode())
        if isinstance(other, Quadratic):
            new = _ommx_rust.Quadratic.decode(self.raw.SerializeToString())
            rhs = _ommx_rust.Quadratic.decode(other.raw.SerializeToString())
            return Quadratic.from_bytes((new + rhs).encode())
        return NotImplemented

    def __radd__(self, other):
        return self + other

    def __sub__(
        self, other: int | float | DecisionVariable | Linear | Quadratic
    ) -> Quadratic:
        if isinstance(other, (int, float, DecisionVariable, Linear, Quadratic)):
            return self + (-other)
        return NotImplemented

    def __rsub__(self, other):
        return -self + other

    @overload
    def __mul__(self, other: int | float) -> Quadratic: ...

    @overload
    def __mul__(self, other: DecisionVariable | Linear | Quadratic) -> Polynomial: ...

    def __mul__(
        self, other: int | float | DecisionVariable | Linear | Quadratic
    ) -> Quadratic | Polynomial:
        if isinstance(other, float) or isinstance(other, int):
            new = _ommx_rust.Quadratic.decode(self.raw.SerializeToString())
            return Quadratic.from_bytes(new.mul_scalar(other).encode())
        if isinstance(other, DecisionVariable):
            new = _ommx_rust.Quadratic.decode(self.raw.SerializeToString())
            rhs = _ommx_rust.Linear.single_term(other.raw.id, 1)
            return Polynomial.from_bytes(new.mul_linear(rhs).encode())
        if isinstance(other, Linear):
            new = _ommx_rust.Quadratic.decode(self.raw.SerializeToString())
            rhs = _ommx_rust.Linear.decode(other.raw.SerializeToString())
            return Polynomial.from_bytes((new.mul_linear(rhs)).encode())
        if isinstance(other, Quadratic):
            new = _ommx_rust.Quadratic.decode(self.raw.SerializeToString())
            rhs = _ommx_rust.Quadratic.decode(other.raw.SerializeToString())
            return Polynomial.from_bytes((new * rhs).encode())
        return NotImplemented

    def __rmul__(self, other):
        return self * other

    def __neg__(self) -> Linear:
        return -1 * self


@dataclass
class Polynomial:
    raw: _Polynomial

    def __init__(self, *, terms: dict[Iterable[int], float | int] = {}):
        self.raw = _Polynomial(
            terms=[
                _Monomial(ids=ids, coefficient=coefficient)
                for ids, coefficient in terms.items()
            ]
        )

    @staticmethod
    def from_bytes(data: bytes) -> Polynomial:
        new = Polynomial()
        new.raw.ParseFromString(data)
        return new

    def to_bytes(self) -> bytes:
        return self.raw.SerializeToString()

    def almost_equal(self, other: Polynomial, *, atol: float = 1e-10) -> bool:
        """
        Compare two polynomial have almost equal coefficients
        """
        lhs = _ommx_rust.Polynomial.decode(self.raw.SerializeToString())
        rhs = _ommx_rust.Polynomial.decode(other.raw.SerializeToString())
        return lhs.almost_equal(rhs, atol)

    def __repr__(self) -> str:
        return f"Polynomial({_ommx_rust.Polynomial.decode(self.raw.SerializeToString()).__repr__()})"

    def __add__(
        self, other: int | float | DecisionVariable | Linear | Quadratic | Polynomial
    ) -> Polynomial:
        if isinstance(other, float) or isinstance(other, int):
            new = _ommx_rust.Polynomial.decode(self.raw.SerializeToString())
            return Polynomial.from_bytes(new.add_scalar(other).encode())
        if isinstance(other, DecisionVariable):
            new = _ommx_rust.Polynomial.decode(self.raw.SerializeToString())
            rhs = _ommx_rust.Linear.single_term(other.raw.id, 1)
            return Polynomial.from_bytes(new.add_linear(rhs).encode())
        if isinstance(other, Linear):
            new = _ommx_rust.Polynomial.decode(self.raw.SerializeToString())
            rhs = _ommx_rust.Linear.decode(other.raw.SerializeToString())
            return Polynomial.from_bytes(new.add_linear(rhs).encode())
        if isinstance(other, Quadratic):
            new = _ommx_rust.Polynomial.decode(self.raw.SerializeToString())
            rhs = _ommx_rust.Quadratic.decode(other.raw.SerializeToString())
            return Polynomial.from_bytes(new.add_quadratic(rhs).encode())
        if isinstance(other, Polynomial):
            new = _ommx_rust.Polynomial.decode(self.raw.SerializeToString())
            rhs = _ommx_rust.Polynomial.decode(other.raw.SerializeToString())
            return Polynomial.from_bytes((new + rhs).encode())
        return NotImplemented

    def __radd__(self, other):
        return self + other

    def __sub__(
        self, other: int | float | DecisionVariable | Linear | Quadratic | Polynomial
    ) -> Polynomial:
        if isinstance(
            other, (int, float, DecisionVariable, Linear, Quadratic, Polynomial)
        ):
            return self + (-other)
        return NotImplemented

    def __rsub__(self, other):
        return -self + other

    def __mul__(
        self, other: int | float | DecisionVariable | Linear | Quadratic | Polynomial
    ) -> Polynomial:
        if isinstance(other, float) or isinstance(other, int):
            new = _ommx_rust.Polynomial.decode(self.raw.SerializeToString())
            return Polynomial.from_bytes(new.mul_scalar(other).encode())
        if isinstance(other, DecisionVariable):
            new = _ommx_rust.Polynomial.decode(self.raw.SerializeToString())
            rhs = _ommx_rust.Linear.single_term(other.raw.id, 1)
            return Polynomial.from_bytes(new.mul_linear(rhs).encode())
        if isinstance(other, Linear):
            new = _ommx_rust.Polynomial.decode(self.raw.SerializeToString())
            rhs = _ommx_rust.Linear.decode(other.raw.SerializeToString())
            return Polynomial.from_bytes(new.mul_linear(rhs).encode())
        if isinstance(other, Quadratic):
            new = _ommx_rust.Polynomial.decode(self.raw.SerializeToString())
            rhs = _ommx_rust.Quadratic.decode(other.raw.SerializeToString())
            return Polynomial.from_bytes(new.mul_quadratic(rhs).encode())
        if isinstance(other, Polynomial):
            new = _ommx_rust.Polynomial.decode(self.raw.SerializeToString())
            rhs = _ommx_rust.Polynomial.decode(other.raw.SerializeToString())
            return Polynomial.from_bytes((new * rhs).encode())
        return NotImplemented

    def __rmul__(self, other):
        return self * other

    def __neg__(self) -> Linear:
        return -1 * self


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
class Function:
    raw: _Function

    def __init__(
        self,
        inner: int
        | float
        | DecisionVariable
        | Linear
        | Quadratic
        | Polynomial
        | _Function,
    ):
        self.raw = as_function(inner)

    @staticmethod
    def from_bytes(data: bytes) -> Function:
        new = Function(0)
        new.raw.ParseFromString(data)
        return new

    def to_bytes(self) -> bytes:
        return self.raw.SerializeToString()

    def almost_equal(self, other: Function, *, atol: float = 1e-10) -> bool:
        """
        Compare two functions have almost equal coefficients as a polynomial
        """
        lhs = _ommx_rust.Function.decode(self.raw.SerializeToString())
        rhs = _ommx_rust.Function.decode(other.raw.SerializeToString())
        return lhs.almost_equal(rhs, atol)

    def __repr__(self) -> str:
        return f"Function({_ommx_rust.Function.decode(self.raw.SerializeToString()).__repr__()})"

    def __add__(
        self,
        other: int
        | float
        | DecisionVariable
        | Linear
        | Quadratic
        | Polynomial
        | Function,
    ) -> Function:
        if isinstance(other, float) or isinstance(other, int):
            rhs = _ommx_rust.Function.from_scalar(other)
        elif isinstance(other, DecisionVariable):
            rhs = _ommx_rust.Function.from_linear(
                _ommx_rust.Linear.single_term(other.raw.id, 1)
            )
        elif isinstance(other, Linear):
            rhs = _ommx_rust.Function.from_linear(
                _ommx_rust.Linear.decode(other.raw.SerializeToString())
            )
        elif isinstance(other, Quadratic):
            rhs = _ommx_rust.Function.from_quadratic(
                _ommx_rust.Quadratic.decode(other.raw.SerializeToString())
            )
        elif isinstance(other, Polynomial):
            rhs = _ommx_rust.Function.from_polynomial(
                _ommx_rust.Polynomial.decode(other.raw.SerializeToString())
            )
        elif isinstance(other, Function):
            rhs = _ommx_rust.Function.decode(other.raw.SerializeToString())
        else:
            return NotImplemented
        new = _ommx_rust.Function.decode(self.raw.SerializeToString())
        return Function.from_bytes((new + rhs).encode())

    def __radd__(self, other):
        return self + other

    def __sub__(
        self,
        other: int
        | float
        | DecisionVariable
        | Linear
        | Quadratic
        | Polynomial
        | Function,
    ) -> Function:
        return self + (-other)

    def __rsub__(self, other):
        return -self + other

    def __mul__(
        self,
        other: int
        | float
        | DecisionVariable
        | Linear
        | Quadratic
        | Polynomial
        | Function,
    ) -> Function:
        if isinstance(other, float) or isinstance(other, int):
            rhs = _ommx_rust.Function.from_scalar(other)
        elif isinstance(other, DecisionVariable):
            rhs = _ommx_rust.Function.from_linear(
                _ommx_rust.Linear.single_term(other.raw.id, 1)
            )
        elif isinstance(other, Linear):
            rhs = _ommx_rust.Function.from_linear(
                _ommx_rust.Linear.decode(other.raw.SerializeToString())
            )
        elif isinstance(other, Quadratic):
            rhs = _ommx_rust.Function.from_quadratic(
                _ommx_rust.Quadratic.decode(other.raw.SerializeToString())
            )
        elif isinstance(other, Polynomial):
            rhs = _ommx_rust.Function.from_polynomial(
                _ommx_rust.Polynomial.decode(other.raw.SerializeToString())
            )
        elif isinstance(other, Function):
            rhs = _ommx_rust.Function.decode(other.raw.SerializeToString())
        else:
            return NotImplemented
        new = _ommx_rust.Function.decode(self.raw.SerializeToString())
        return Function.from_bytes((new * rhs).encode())

    def __rmul__(self, other):
        return self * other

    def __neg__(self) -> Function:
        return -1 * self


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
        Constraint(Function(x1 + x2 - 1) == 0)

        To set the name or other attributes, use methods like :py:meth:`add_name`.

        >>> (x + y <= 5).add_name("constraint 1")
        Constraint(Function(x1 + x2 - 5) <= 0)

    """

    raw: _Constraint
    _counter: int = 0

    EQUAL_TO_ZERO = Equality.EQUALITY_EQUAL_TO_ZERO
    LESS_THAN_OR_EQUAL_TO_ZERO = Equality.EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO

    def __init__(
        self,
        *,
        function: int | float | DecisionVariable | Linear | Quadratic | Polynomial,
        equality: Equality.ValueType,
        id: Optional[int] = None,
        name: Optional[str] = None,
        description: Optional[str] = None,
        subscripts: Optional[list[int]] = None,
        parameters: Optional[dict[str, str]] = None,
    ):
        if id is None:
            id = Constraint._counter
            Constraint._counter += 1
        if id > Constraint._counter:
            Constraint._counter = id + 1

        self.raw = _Constraint(
            id=id,
            function=as_function(function),
            equality=equality,
            name=name,
            description=description,
            subscripts=subscripts,
            parameters=parameters,
        )

    @staticmethod
    def from_bytes(data: bytes) -> Constraint:
        raw = _Constraint()
        raw.ParseFromString(data)
        new = Constraint(function=0, equality=Equality.EQUALITY_EQUAL_TO_ZERO)
        new.raw = raw
        Constraint._counter = max(Constraint._counter, raw.id + 1)
        return new

    def to_bytes(self) -> bytes:
        return self.raw.SerializeToString()

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

    @property
    def function(self) -> Function:
        return Function(self.raw.function)

    @property
    def id(self) -> int:
        return self.raw.id

    @property
    def name(self) -> str:
        return self.raw.name

    @property
    def equality(self) -> Equality.ValueType:
        return self.raw.equality

    @property
    def description(self) -> str:
        return self.raw.description

    @property
    def subscripts(self) -> list[int]:
        return list(self.raw.subscripts)

    @property
    def parameters(self) -> dict[str, str]:
        return dict(self.raw.parameters)

    def __repr__(self) -> str:
        if self.raw.equality == Equality.EQUALITY_EQUAL_TO_ZERO:
            return f"Constraint({self.function.__repr__()} == 0)"
        if self.raw.equality == Equality.EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO:
            return f"Constraint({self.function.__repr__()} <= 0)"
        return self.raw.__repr__()
