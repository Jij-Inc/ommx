from __future__ import annotations
from typing import Optional, Iterable, overload, Mapping
from typing_extensions import deprecated
from datetime import datetime
from dataclasses import dataclass, field
from pandas import DataFrame, NA
from abc import ABC, abstractmethod

from .solution_pb2 import State, Optimality, Relaxation, Solution as _Solution
from .instance_pb2 import Instance as _Instance, Parameters
from .function_pb2 import Function as _Function
from .quadratic_pb2 import Quadratic as _Quadratic
from .polynomial_pb2 import Polynomial as _Polynomial, Monomial as _Monomial
from .linear_pb2 import Linear as _Linear
from .constraint_pb2 import (
    Equality,
    Constraint as _Constraint,
    RemovedConstraint as _RemovedConstraint,
)
from .decision_variables_pb2 import DecisionVariable as _DecisionVariable, Bound
from .parametric_instance_pb2 import (
    ParametricInstance as _ParametricInstance,
    Parameter as _Parameter,
)

from .. import _ommx_rust

__all__ = [
    "Instance",
    "ParametricInstance",
    "Solution",
    "Constraint",
    # Function and its bases
    "DecisionVariable",
    "Parameter",
    "Linear",
    "Quadratic",
    "Polynomial",
    "Function",
    # Imported from protobuf
    "State",
    "Parameters",
    "Optimality",
    "Relaxation",
    "Bound",
]


class InstanceBase(ABC):
    @abstractmethod
    def get_decision_variables(self) -> list[DecisionVariable]: ...
    @abstractmethod
    def get_constraints(self) -> list[Constraint]: ...
    @abstractmethod
    def get_removed_constraints(self) -> list[RemovedConstraint]: ...

    def get_decision_variable(self, variable_id: int) -> DecisionVariable:
        """
        Get a decision variable by ID.
        """
        for v in self.get_decision_variables():
            if v.id == variable_id:
                return v
        raise ValueError(f"Decision variable ID {variable_id} is not found")

    def get_constraint(self, constraint_id: int) -> Constraint:
        """
        Get a constraint by ID.
        """
        for c in self.get_constraints():
            if c.id == constraint_id:
                return c
        raise ValueError(f"Constraint ID {constraint_id} is not found")

    def get_removed_constraint(self, removed_constraint_id: int) -> RemovedConstraint:
        """
        Get a removed constraint by ID.
        """
        for rc in self.get_removed_constraints():
            if rc.id == removed_constraint_id:
                return rc
        raise ValueError(f"Removed constraint ID {removed_constraint_id} is not found")

    @property
    def decision_variables(self) -> DataFrame:
        return DataFrame(
            v._as_pandas_entry() for v in self.get_decision_variables()
        ).set_index("id")

    @property
    def constraints(self) -> DataFrame:
        return DataFrame(
            c._as_pandas_entry() for c in self.get_constraints()
        ).set_index("id")

    @property
    def removed_constraints(self) -> DataFrame:
        return DataFrame(
            rc._as_pandas_entry() for rc in self.get_removed_constraints()
        ).set_index("id")


class UserAnnotationBase(ABC):
    @property
    @abstractmethod
    def _annotations(self) -> dict[str, str]: ...

    def add_user_annotation(
        self, key: str, value: str, *, annotation_namespace: str = "org.ommx.user."
    ):
        if not annotation_namespace.endswith("."):
            annotation_namespace += "."
        self._annotations[annotation_namespace + key] = value

    def add_user_annotations(
        self,
        annotations: dict[str, str],
        *,
        annotation_namespace: str = "org.ommx.user.",
    ):
        for key, value in annotations.items():
            self.add_user_annotation(
                key, value, annotation_namespace=annotation_namespace
            )

    def get_user_annotation(
        self, key: str, *, annotation_namespace: str = "org.ommx.user."
    ):
        if not annotation_namespace.endswith("."):
            annotation_namespace += "."
        return self._annotations[annotation_namespace + key]

    def get_user_annotations(
        self, *, annotation_namespace: str = "org.ommx.user."
    ) -> dict[str, str]:
        if not annotation_namespace.endswith("."):
            annotation_namespace += "."
        return {
            key[len(annotation_namespace) :]: value
            for key, value in self._annotations.items()
            if key.startswith(annotation_namespace)
        }


@dataclass
class Instance(InstanceBase, UserAnnotationBase):
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

    @property
    def _annotations(self) -> dict[str, str]:
        return self.annotations

    # Re-export some enums
    MAXIMIZE = _Instance.SENSE_MAXIMIZE
    MINIMIZE = _Instance.SENSE_MINIMIZE

    Description = _Instance.Description

    @staticmethod
    def empty() -> Instance:
        """
        Create trivial empty instance of minimization with zero objective, no constraints, and no decision variables.
        """
        return Instance.from_components(
            objective=0, constraints=[], sense=Instance.MINIMIZE, decision_variables=[]
        )

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
        """
        Outputs the instance as an MPS file.

        - The outputted file is compressed by gzip.
        - Only linear problems are supported.
        - Various forms of metadata, like problem description and variable/constraint names, are not preserved.
        """
        _ommx_rust.write_mps_file(self.to_bytes(), path)

    def add_user_annotation(
        self, key: str, value: str, *, annotation_namespace: str = "org.ommx.user."
    ):
        """
        Add a user annotation to the instance.

        Examples
        =========

        .. doctest::

                >>> instance = Instance.empty()
                >>> instance.add_user_annotation("author", "Alice")
                >>> instance.get_user_annotations()
                {'author': 'Alice'}
                >>> instance.annotations
                {'org.ommx.user.author': 'Alice'}

        """
        if not annotation_namespace.endswith("."):
            annotation_namespace += "."
        self.annotations[annotation_namespace + key] = value

    def get_user_annotation(
        self, key: str, *, annotation_namespace: str = "org.ommx.user."
    ):
        """
        Get a user annotation from the instance.

        Examples
        =========

        .. doctest::

                >>> instance = Instance.empty()
                >>> instance.add_user_annotation("author", "Alice")
                >>> instance.get_user_annotation("author")
                'Alice'

        """
        if not annotation_namespace.endswith("."):
            annotation_namespace += "."
        return self.annotations[annotation_namespace + key]

    def get_user_annotations(
        self, *, annotation_namespace: str = "org.ommx.user."
    ) -> dict[str, str]:
        """
        Get user annotations from the instance.

        See also :py:meth:`add_user_annotation`.
        """
        if not annotation_namespace.endswith("."):
            annotation_namespace += "."
        return {
            key[len(annotation_namespace) :]: value
            for key, value in self.annotations.items()
            if key.startswith(annotation_namespace)
        }

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
    def objective(self) -> Function:
        return Function(self.raw.objective)

    @property
    def sense(self) -> _Instance.Sense.ValueType:
        return self.raw.sense

    def get_decision_variables(self) -> list[DecisionVariable]:
        """
        Get decision variables as a list of :class:`DecisionVariable` instances.
        """
        return [DecisionVariable(raw) for raw in self.raw.decision_variables]

    def get_constraints(self) -> list[Constraint]:
        """
        Get constraints as a list of :class:`Constraint` instances.
        """
        return [Constraint.from_raw(raw) for raw in self.raw.constraints]

    def get_removed_constraints(self) -> list[RemovedConstraint]:
        """
        Get removed constraints as a list of :class:`RemovedConstraint` instances.
        """
        return [RemovedConstraint(raw) for raw in self.raw.removed_constraints]

    def evaluate(self, state: State | Mapping[int, float]) -> Solution:
        if not isinstance(state, State):
            state = State(entries=state)
        out, _ = _ommx_rust.evaluate_instance(
            self.to_bytes(), state.SerializeToString()
        )
        return Solution.from_bytes(out)

    def partial_evaluate(self, state: State | Mapping[int, float]) -> Instance:
        if not isinstance(state, State):
            state = State(entries=state)
        out, _ = _ommx_rust.partial_evaluate_instance(
            self.to_bytes(), state.SerializeToString()
        )
        return Instance.from_bytes(out)

    def as_qubo_format(self) -> tuple[dict[tuple[int, int], float], float]:
        """
        Convert unconstrained quadratic instance to PyQUBO-style format.

        This method is designed for better composability rather than easy-to-use.
        This does not execute any conversion of the instance, only translates the data format.
        """
        instance = _ommx_rust.Instance.from_bytes(self.to_bytes())
        return instance.as_qubo_format()

    def as_pubo_format(self) -> dict[tuple[int, ...], float]:
        """
        Convert unconstrained polynomial instance to simple PUBO format.

        This method is designed for better composability rather than easy-to-use.
        This does not execute any conversion of the instance, only translates the data format.
        """
        instance = _ommx_rust.Instance.from_bytes(self.to_bytes())
        return instance.as_pubo_format()

    def penalty_method(self) -> ParametricInstance:
        r"""
        Convert to a parametric unconstrained instance by penalty method.

        Roughly, this converts a constrained problem

        .. math::

            \begin{align*}
                \min_x & \space f(x) & \\
                \text{ s.t. } & \space g_i(x) = 0 & (\forall i)
            \end{align*}

        to an unconstrained problem with parameters

        .. math::

            \min_x f(x) + \sum_i \lambda_i g_i(x)^2

        where :math:`\lambda_i` are the penalty weight parameters for each constraint.
        If you want to use single weight parameter, use :py:meth:`uniform_penalty_method` instead.

        The removed constrains are stored in :py:attr:`~ParametricInstance.removed_constraints`.

        :raises RuntimeError: If the instance contains inequality constraints.

        Examples
        =========

        .. doctest::

            >>> from ommx.v1 import Instance, DecisionVariable, Constraint
            >>> x = [DecisionVariable.binary(i) for i in range(3)]
            >>> instance = Instance.from_components(
            ...     decision_variables=x,
            ...     objective=sum(x),
            ...     constraints=[x[0] + x[1] == 1, x[1] + x[2] == 1],
            ...     sense=Instance.MAXIMIZE,
            ... )
            >>> instance.objective
            Function(x0 + x1 + x2)

            >>> pi = instance.penalty_method()

            The constraint is put in `removed_constraints`

            >>> pi.get_constraints()
            []
            >>> len(pi.get_removed_constraints())
            2
            >>> pi.get_removed_constraints()[0]
            RemovedConstraint(Function(x0 + x1 - 1) == 0, reason=penalty_method, parameter_id=3)
            >>> pi.get_removed_constraints()[1]
            RemovedConstraint(Function(x1 + x2 - 1) == 0, reason=penalty_method, parameter_id=4)

            There are two parameters corresponding to the two constraints

            >>> len(pi.get_parameters())
            2
            >>> p1 = pi.get_parameters()[0]
            >>> p1.id, p1.name
            (3, 'penalty_weight')
            >>> p2 = pi.get_parameters()[1]
            >>> p2.id, p2.name
            (4, 'penalty_weight')

            Substitute all parameters to zero to get the original objective

            >>> instance0 = pi.with_parameters({p1.id: 0.0, p2.id: 0.0})
            >>> instance0.objective
            Function(x0 + x1 + x2)

            Substitute all parameters to one

            >>> instance1 = pi.with_parameters({p1.id: 1.0, p2.id: 1.0})
            >>> instance1.objective
            Function(x0*x0 + 2*x0*x1 + 2*x1*x1 + 2*x1*x2 + x2*x2 - x0 - 3*x1 - x2 + 2)

        """
        instance = _ommx_rust.Instance.from_bytes(self.to_bytes())
        return ParametricInstance.from_bytes(instance.penalty_method().to_bytes())

    def uniform_penalty_method(self) -> ParametricInstance:
        r"""
        Convert to a parametric unconstrained instance by penalty method with uniform weight.

        Roughly, this converts a constrained problem

        .. math::

            \begin{align*}
                \min_x & \space f(x) & \\
                \text{ s.t. } & \space g_i(x) = 0 & (\forall i)
            \end{align*}

        to an unconstrained problem with a parameter

        .. math::

            \min_x f(x) + \lambda \sum_i g_i(x)^2

        where :math:`\lambda` is the uniform penalty weight parameter for all constraints.

        The removed constrains are stored in :py:attr:`~ParametricInstance.removed_constraints`.

        :raises RuntimeError: If the instance contains inequality constraints.

        Examples
        =========

        .. doctest::

            >>> from ommx.v1 import Instance, DecisionVariable
            >>> x = [DecisionVariable.binary(i) for i in range(3)]
            >>> instance = Instance.from_components(
            ...     decision_variables=x,
            ...     objective=sum(x),
            ...     constraints=[sum(x) == 3],
            ...     sense=Instance.MAXIMIZE,
            ... )
            >>> instance.objective
            Function(x0 + x1 + x2)

            >>> pi = instance.uniform_penalty_method()

            The constraint is put in `removed_constraints`

            >>> pi.get_constraints()
            []
            >>> len(pi.get_removed_constraints())
            1
            >>> pi.get_removed_constraints()[0]
            RemovedConstraint(Function(x0 + x1 + x2 - 3) == 0, reason=uniform_penalty_method)

            There is only one parameter in the instance

            >>> len(pi.get_parameters())
            1
            >>> p = pi.get_parameters()[0]
            >>> p.id
            3
            >>> p.name
            'uniform_penalty_weight'

            Substitute `p = 0` to get the original objective

            >>> instance0 = pi.with_parameters({p.id: 0.0})
            >>> instance0.objective
            Function(x0 + x1 + x2)

            Substitute `p = 1`

            >>> instance1 = pi.with_parameters({p.id: 1.0})
            >>> instance1.objective
            Function(x0*x0 + 2*x0*x1 + 2*x0*x2 + x1*x1 + 2*x1*x2 + x2*x2 - 5*x0 - 5*x1 - 5*x2 + 9)

        """
        instance = _ommx_rust.Instance.from_bytes(self.to_bytes())
        return ParametricInstance.from_bytes(
            instance.uniform_penalty_method().to_bytes()
        )

    def as_parametric_instance(self) -> ParametricInstance:
        """
        Convert the instance to a :class:`ParametricInstance`.
        """
        instance = _ommx_rust.Instance.from_bytes(self.to_bytes())
        return ParametricInstance.from_bytes(
            instance.as_parametric_instance().to_bytes()
        )


@dataclass
class ParametricInstance(InstanceBase):
    """
    Idiomatic wrapper of ``ommx.v1.ParametricInstance`` protobuf message.
    """

    raw: _ParametricInstance

    @staticmethod
    def from_bytes(data: bytes) -> ParametricInstance:
        raw = _ParametricInstance()
        raw.ParseFromString(data)
        return ParametricInstance(raw)

    def to_bytes(self) -> bytes:
        return self.raw.SerializeToString()

    def get_decision_variables(self) -> list[DecisionVariable]:
        """
        Get decision variables as a list of :class:`DecisionVariable` instances.
        """
        return [DecisionVariable(raw) for raw in self.raw.decision_variables]

    def get_constraints(self) -> list[Constraint]:
        """
        Get constraints as a list of :class:`Constraint
        """
        return [Constraint.from_raw(raw) for raw in self.raw.constraints]

    def get_removed_constraints(self) -> list[RemovedConstraint]:
        """
        Get removed constraints as a list of :class:`RemovedConstraint` instances.
        """
        return [RemovedConstraint(raw) for raw in self.raw.removed_constraints]

    def get_parameters(self) -> list[Parameter]:
        """
        Get parameters as a list of :class:`Parameter`.
        """
        return [Parameter(raw) for raw in self.raw.parameters]

    def get_parameter(self, parameter_id: int) -> Parameter:
        """
        Get a parameter by ID.
        """
        for p in self.raw.parameters:
            if p.id == parameter_id:
                return Parameter(p)
        raise ValueError(f"Parameter ID {parameter_id} is not found")

    @property
    def parameters(self) -> DataFrame:
        return DataFrame(p._as_pandas_entry() for p in self.get_parameters()).set_index(
            "id"
        )

    def with_parameters(self, parameters: Parameters | Mapping[int, float]) -> Instance:
        """
        Substitute parameters to yield an instance.
        """
        if not isinstance(parameters, Parameters):
            parameters = Parameters(entries=parameters)
        pi = _ommx_rust.ParametricInstance.from_bytes(self.to_bytes())
        ps = _ommx_rust.Parameters.from_bytes(parameters.SerializeToString())
        instance = pi.with_parameters(ps)
        return Instance.from_bytes(instance.to_bytes())


class VariableBase(ABC):
    @property
    @abstractmethod
    def id(self) -> int: ...

    def __add__(self, other: int | float | VariableBase) -> Linear:
        if isinstance(other, float) or isinstance(other, int):
            return Linear(terms={self.id: 1}, constant=other)
        if isinstance(other, VariableBase):
            if self.id == other.id:
                return Linear(terms={self.id: 2})
            else:
                return Linear(terms={self.id: 1, other.id: 1})
        return NotImplemented

    def __sub__(self, other) -> Linear:
        return self + (-other)

    def __neg__(self) -> Linear:
        return Linear(terms={self.id: -1})

    def __radd__(self, other) -> Linear:
        return self + other

    def __rsub__(self, other) -> Linear:
        return -self + other

    @overload
    def __mul__(self, other: int | float) -> Linear: ...

    @overload
    def __mul__(self, other: VariableBase) -> Quadratic: ...

    def __mul__(self, other: int | float | VariableBase) -> Linear | Quadratic:
        if isinstance(other, float) or isinstance(other, int):
            return Linear(terms={self.id: other})
        if isinstance(other, VariableBase):
            return Quadratic(columns=[self.id], rows=[other.id], values=[1.0])
        return NotImplemented

    def __rmul__(self, other):
        return self * other

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
class Parameter(VariableBase):
    """
    Idiomatic wrapper of ``ommx.v1.Parameter`` protobuf message.
    """

    raw: _Parameter

    @staticmethod
    def from_bytes(data: bytes) -> Parameter:
        raw = _Parameter()
        raw.ParseFromString(data)
        return Parameter(raw)

    def to_bytes(self) -> bytes:
        return self.raw.SerializeToString()

    @property
    def id(self) -> int:
        return self.raw.id

    @property
    def name(self) -> str:
        return self.raw.name

    @property
    def subscripts(self) -> list[int]:
        return list(self.raw.subscripts)

    @property
    def description(self) -> str:
        return self.raw.description

    @property
    def parameters(self) -> dict[str, str]:
        return dict(self.raw.parameters)

    def equals_to(self, other: Parameter) -> bool:
        """
        Alternative to ``==`` operator to compare two decision variables.
        """
        return self.raw == other.raw

    # The special function __eq__ cannot be inherited from VariableBase
    def __eq__(self, other) -> Constraint:  # type: ignore[reportIncompatibleMethodOverride]
        return Constraint(
            function=self - other, equality=Equality.EQUALITY_EQUAL_TO_ZERO
        )

    def _as_pandas_entry(self) -> dict:
        p = self.raw
        return {
            "id": p.id,
            "name": p.name if p.HasField("name") else NA,
            "subscripts": p.subscripts,
            "description": p.description if p.HasField("description") else NA,
            **{f"parameters.{key}": value for key, value in p.parameters.items()},
        }


@dataclass
class Solution(UserAnnotationBase):
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

    @property
    def _annotations(self) -> dict[str, str]:
        return self.annotations

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
        return DataFrame(
            DecisionVariable(v)._as_pandas_entry()
            | {"value": self.raw.state.entries[v.id]}
            for v in self.raw.decision_variables
        ).set_index("id")

    @property
    def constraints(self) -> DataFrame:
        return DataFrame(
            {
                "id": c.id,
                "equality": _equality(c.equality),
                "value": c.evaluated_value,
                "used_ids": set(c.used_decision_variable_ids),
                "name": c.name if c.HasField("name") else NA,
                "subscripts": c.subscripts,
                "description": c.description if c.HasField("description") else NA,
                "dual_variable": c.dual_variable if c.HasField("dual_variable") else NA,
                "removed_reason": c.removed_reason
                if c.HasField("removed_reason")
                else NA,
            }
            | {
                f"removed_reason.{key}": value
                for key, value in c.removed_reason_parameters.items()
            }
            for c in self.raw.evaluated_constraints
        ).set_index("id")

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
class DecisionVariable(VariableBase):
    """
    Idiomatic wrapper of ``ommx.v1.DecisionVariable`` protobuf message.

    Note that this object overloads `==` for creating a constraint, not for equality comparison for better integration to mathematical programming.

    >>> x = DecisionVariable.integer(1)
    >>> x == 1
    Constraint(...)

    To compare two objects, use :py:meth:`equals_to` method.

    >>> y = DecisionVariable.integer(2)
    >>> x.equals_to(y)
    False

    """

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

    # The special function __eq__ cannot be inherited from VariableBase
    def __eq__(self, other) -> Constraint:  # type: ignore[reportIncompatibleMethodOverride]
        return Constraint(
            function=self - other, equality=Equality.EQUALITY_EQUAL_TO_ZERO
        )

    def _as_pandas_entry(self) -> dict:
        v = self.raw
        return {
            "id": v.id,
            "kind": _kind(v.kind),
            "lower": v.bound.lower if v.HasField("bound") else NA,
            "upper": v.bound.upper if v.HasField("bound") else NA,
            "name": v.name if v.HasField("name") else NA,
            "subscripts": v.subscripts,
            "description": v.description if v.HasField("description") else NA,
            "substituted_value": v.substituted_value
            if v.HasField("substituted_value")
            else NA,
        } | {f"parameters.{key}": value for key, value in v.parameters.items()}


class AsConstraint(ABC):
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
class Linear(AsConstraint):
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
    def from_raw(raw: _Linear) -> Linear:
        new = Linear(terms={})
        new.raw = raw
        return new

    @property
    def linear_terms(self) -> dict[int, float]:
        """
        Get the terms of the linear function as a dictionary
        """
        out = {}
        for term in self.raw.terms:
            if term.id not in out:
                out[term.id] = term.coefficient
            else:
                out[term.id] += term.coefficient
        return out

    @property
    def terms(self) -> dict[tuple[int, ...], float]:
        """
        Linear terms and constant as a dictionary
        """
        return {(id,): value for id, value in self.linear_terms.items()} | {
            (): self.constant
        }

    @property
    def constant(self) -> float:
        """
        Get the constant term of the linear function
        """
        return self.raw.constant

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

    def evaluate(self, state: State | Mapping[int, float]) -> tuple[float, set]:
        """
        Evaluate the linear function with the given state.

        Examples
        =========

        .. doctest::

            Evaluate `2 x1 + 3 x2 + 1` with `x1 = 3, x2 = 4, x3 = 5`

            >>> f = Linear(terms={1: 2, 2: 3}, constant=1)
            >>> value, used_ids = f.evaluate({1: 3, 2: 4, 3: 5}) # Unused ID `3` can be included

            2*3 + 3*4 + 1 = 19
            >>> value
            19.0

            Since the value of ID `3` of `state` is not used, the it is not included in `used_ids`.
            >>> used_ids
            {1, 2}

            Missing ID raises an error
            >>> f.evaluate({1: 3})
            Traceback (most recent call last):
            ...
            RuntimeError: Variable id (2) is not found in the solution

        """
        if not isinstance(state, State):
            state = State(entries=state)
        return _ommx_rust.evaluate_linear(self.to_bytes(), state.SerializeToString())

    def partial_evaluate(
        self, state: State | Mapping[int, float]
    ) -> tuple[Linear, set]:
        """
        Partially evaluate the linear function with the given state.

        Examples
        =========

        .. doctest::

            Evaluate `2 x1 + 3 x2 + 1` with `x1 = 3`, yielding `3 x2 + 7`

            >>> f = Linear(terms={1: 2, 2: 3}, constant=1)
            >>> new_f, used_ids = f.partial_evaluate({1: 3})
            >>> new_f
            Linear(3*x2 + 7)
            >>> used_ids
            {1}
            >>> new_f.partial_evaluate({2: 4})
            (Linear(19), {2})

        """
        if not isinstance(state, State):
            state = State(entries=state)
        new, used_ids = _ommx_rust.partial_evaluate_linear(
            self.to_bytes(), state.SerializeToString()
        )
        return Linear.from_bytes(new), used_ids

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

    def __eq__(self, other) -> Constraint:  # type: ignore[reportIncompatibleMethodOverride]
        return Constraint(
            function=self - other, equality=Equality.EQUALITY_EQUAL_TO_ZERO
        )


@dataclass
class Quadratic(AsConstraint):
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
    def from_raw(raw: _Quadratic) -> Quadratic:
        new = Quadratic(columns=[], rows=[], values=[])
        new.raw = raw
        return new

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

    def evaluate(self, state: State | Mapping[int, float]) -> tuple[float, set]:
        """
        Evaluate the quadratic function with the given state.

        Examples
        =========

        .. doctest::

            Evaluate `2 x1 x2 + 3 x2 x3 + 1` with `x1 = 3, x2 = 4, x3 = 5`

            >>> x1 = DecisionVariable.integer(1)
            >>> x2 = DecisionVariable.integer(2)
            >>> x3 = DecisionVariable.integer(3)
            >>> f = 2*x1*x2 + 3*x2*x3 + 1
            >>> f
            Quadratic(2*x1*x2 + 3*x2*x3 + 1)

            >>> f.evaluate({1: 3, 2: 4, 3: 5})
            (85.0, {1, 2, 3})

            Missing ID raises an error
            >>> f.evaluate({1: 3})
            Traceback (most recent call last):
            ...
            RuntimeError: Variable id (2) is not found in the solution

        """
        if not isinstance(state, State):
            state = State(entries=state)
        return _ommx_rust.evaluate_quadratic(self.to_bytes(), state.SerializeToString())

    def partial_evaluate(
        self, state: State | Mapping[int, float]
    ) -> tuple[Quadratic, set]:
        """
        Partially evaluate the quadratic function with the given state.

        Examples
        =========

        .. doctest::

            Evaluate `2 x1 x2 + 3 x2 x3 + 1` with `x1 = 3`, yielding `3 x2 x3 + 6 x2 + 1`

            >>> x1 = DecisionVariable.integer(1)
            >>> x2 = DecisionVariable.integer(2)
            >>> x3 = DecisionVariable.integer(3)
            >>> f = 2*x1*x2 + 3*x2*x3 + 1
            >>> f
            Quadratic(2*x1*x2 + 3*x2*x3 + 1)

            >>> f.partial_evaluate({1: 3})
            (Quadratic(3*x2*x3 + 6*x2 + 1), {1})

        """
        if not isinstance(state, State):
            state = State(entries=state)
        new, used_ids = _ommx_rust.partial_evaluate_quadratic(
            self.to_bytes(), state.SerializeToString()
        )
        return Quadratic.from_bytes(new), used_ids

    @property
    def linear(self) -> Linear | None:
        if self.raw.HasField("linear"):
            return Linear.from_raw(self.raw.linear)
        return None

    @property
    def quad_terms(self) -> dict[tuple[int, int], float]:
        assert len(self.raw.columns) == len(self.raw.rows) == len(self.raw.values)
        out = {}
        for column, row, value in zip(self.raw.columns, self.raw.rows, self.raw.values):
            if (column, row) not in out:
                out[(column, row)] = value
            else:
                out[(column, row)] += value
        return out

    @property
    def terms(self) -> dict[tuple[int, ...], float]:
        return self.quad_terms | (self.linear.terms if self.linear else {})

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

    def __eq__(self, other) -> Constraint:  # type: ignore[reportIncompatibleMethodOverride]
        return Constraint(
            function=self - other, equality=Equality.EQUALITY_EQUAL_TO_ZERO
        )


@dataclass
class Polynomial(AsConstraint):
    raw: _Polynomial

    def __init__(self, *, terms: dict[Iterable[int], float | int] = {}):
        self.raw = _Polynomial(
            terms=[
                _Monomial(ids=ids, coefficient=coefficient)
                for ids, coefficient in terms.items()
            ]
        )

    @staticmethod
    def from_raw(raw: _Polynomial) -> Polynomial:
        new = Polynomial()
        new.raw = raw
        return new

    @staticmethod
    def from_bytes(data: bytes) -> Polynomial:
        new = Polynomial()
        new.raw.ParseFromString(data)
        return new

    @property
    def terms(self) -> dict[tuple[int, ...], float]:
        out = {}
        for term in self.raw.terms:
            term.ids.sort()
            key = tuple(term.ids)
            if key in out:
                out[key] += term.coefficient
            else:
                out[key] = term.coefficient
        return out

    def to_bytes(self) -> bytes:
        return self.raw.SerializeToString()

    def almost_equal(self, other: Polynomial, *, atol: float = 1e-10) -> bool:
        """
        Compare two polynomial have almost equal coefficients
        """
        lhs = _ommx_rust.Polynomial.decode(self.raw.SerializeToString())
        rhs = _ommx_rust.Polynomial.decode(other.raw.SerializeToString())
        return lhs.almost_equal(rhs, atol)

    def evaluate(self, state: State | Mapping[int, float]) -> tuple[float, set]:
        """
        Evaluate the polynomial with the given state.

        Examples
        =========

        .. doctest::

            Evaluate `2 x1 x2 x3 + 3 x2 x3 + 1` with `x1 = 3, x2 = 4, x3 = 5`

            >>> x1 = DecisionVariable.integer(1)
            >>> x2 = DecisionVariable.integer(2)
            >>> x3 = DecisionVariable.integer(3)
            >>> f = 2*x1*x2*x3 + 3*x2*x3 + 1
            >>> f
            Polynomial(2*x1*x2*x3 + 3*x2*x3 + 1)

            >>> f.evaluate({1: 3, 2: 4, 3: 5})
            (181.0, {1, 2, 3})

            Missing ID raises an error
            >>> f.evaluate({1: 3})
            Traceback (most recent call last):
            ...
            RuntimeError: Variable id (2) is not found in the solution

        """
        if not isinstance(state, State):
            state = State(entries=state)
        return _ommx_rust.evaluate_polynomial(
            self.to_bytes(), state.SerializeToString()
        )

    def partial_evaluate(
        self, state: State | Mapping[int, float]
    ) -> tuple[Polynomial, set]:
        """
        Partially evaluate the polynomial with the given state.

        Examples
        =========

        .. doctest::

            Evaluate `2 x1 x2 x3 + 3 x2 x3 + 1` with `x1 = 3`, yielding `9 x2 x3 + 1`

            >>> x1 = DecisionVariable.integer(1)
            >>> x2 = DecisionVariable.integer(2)
            >>> x3 = DecisionVariable.integer(3)
            >>> f = 2*x1*x2*x3 + 3*x2*x3 + 1
            >>> f
            Polynomial(2*x1*x2*x3 + 3*x2*x3 + 1)

            >>> f.partial_evaluate({1: 3})
            (Polynomial(9*x2*x3 + 1), {1})

        """
        if not isinstance(state, State):
            state = State(entries=state)
        new, used_ids = _ommx_rust.partial_evaluate_polynomial(
            self.to_bytes(), state.SerializeToString()
        )
        return Polynomial.from_bytes(new), used_ids

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

    def __eq__(self, other) -> Constraint:  # type: ignore[reportIncompatibleMethodOverride]
        return Constraint(
            function=self - other, equality=Equality.EQUALITY_EQUAL_TO_ZERO
        )


def as_function(
    f: int
    | float
    | DecisionVariable
    | Linear
    | Quadratic
    | Polynomial
    | _Function
    | Function,
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
    elif isinstance(f, Function):
        return f.raw
    else:
        raise ValueError(f"Unknown function type: {type(f)}")


@dataclass
class Function(AsConstraint):
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

    @property
    def terms(self) -> dict[tuple[int, ...], float]:
        if self.raw.HasField("constant"):
            return {(): self.raw.constant}
        if self.raw.HasField("linear"):
            return Linear.from_raw(self.raw.linear).terms
        if self.raw.HasField("quadratic"):
            return Quadratic.from_raw(self.raw.quadratic).terms
        if self.raw.HasField("polynomial"):
            return Polynomial.from_raw(self.raw.polynomial).terms
        raise ValueError("Unknown function type")

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

    def evaluate(self, state: State | Mapping[int, float]) -> tuple[float, set]:
        """
        Evaluate the function with the given state.

        Examples
        =========

        .. doctest::

            Evaluate `2 x1 x2 + 3 x2 x3 + 1` with `x1 = 3, x2 = 4, x3 = 5`

            >>> x1 = DecisionVariable.integer(1)
            >>> x2 = DecisionVariable.integer(2)
            >>> x3 = DecisionVariable.integer(3)
            >>> f = Function(2*x1*x2 + 3*x2*x3 + 1)
            >>> f
            Function(2*x1*x2 + 3*x2*x3 + 1)

            >>> f.evaluate({1: 3, 2: 4, 3: 5})
            (85.0, {1, 2, 3})

            Missing ID raises an error
            >>> f.evaluate({1: 3})
            Traceback (most recent call last):
            ...
            RuntimeError: Variable id (2) is not found in the solution

        """
        if not isinstance(state, State):
            state = State(entries=state)
        return _ommx_rust.evaluate_function(self.to_bytes(), state.SerializeToString())

    def partial_evaluate(
        self, state: State | Mapping[int, float]
    ) -> tuple[Function, set]:
        """
        Partially evaluate the function with the given state.

        Examples
        =========

        .. doctest::

            Evaluate `2 x1 x2 + 3 x2 x3 + 1` with `x1 = 3`, yielding `3 x2 x3 + 6 x2 + 1`

            >>> x1 = DecisionVariable.integer(1)
            >>> x2 = DecisionVariable.integer(2)
            >>> x3 = DecisionVariable.integer(3)
            >>> f = Function(2*x1*x2 + 3*x2*x3 + 1)
            >>> f
            Function(2*x1*x2 + 3*x2*x3 + 1)

            >>> f.partial_evaluate({1: 3})
            (Function(3*x2*x3 + 6*x2 + 1), {1})

        """
        if not isinstance(state, State):
            state = State(entries=state)
        new, used_ids = _ommx_rust.partial_evaluate_function(
            self.to_bytes(), state.SerializeToString()
        )
        return Function.from_bytes(new), used_ids

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

    def __eq__(self, other) -> Constraint:  # type: ignore[reportIncompatibleMethodOverride]
        return Constraint(
            function=self - other, equality=Equality.EQUALITY_EQUAL_TO_ZERO
        )


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
        function: int
        | float
        | DecisionVariable
        | Linear
        | Quadratic
        | Polynomial
        | Function,
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
    def from_raw(raw: _Constraint) -> Constraint:
        new = Constraint(function=0, equality=Equality.EQUALITY_UNSPECIFIED)
        new.raw = raw
        Constraint._counter = max(Constraint._counter, raw.id + 1)
        return new

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
    def equality(self) -> Equality.ValueType:
        return self.raw.equality

    @property
    def name(self) -> str | None:
        return self.raw.name if self.raw.HasField("name") else None

    @property
    def description(self) -> str | None:
        return self.raw.description if self.raw.HasField("description") else None

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

    def _as_pandas_entry(self) -> dict:
        c = self.raw
        return {
            "id": c.id,
            "equality": _equality(c.equality),
            "type": _function_type(c.function),
            "used_ids": _ommx_rust.used_decision_variable_ids(
                c.function.SerializeToString()
            ),
            "name": c.name if c.HasField("name") else NA,
            "subscripts": c.subscripts,
            "description": c.description if c.HasField("description") else NA,
        } | {f"parameters.{key}": value for key, value in c.parameters.items()}


@dataclass
class RemovedConstraint:
    """
    Constraints removed while preprocessing
    """

    raw: _RemovedConstraint

    def __repr__(self) -> str:
        reason = f"reason={self.removed_reason}"
        if self.removed_reason_parameters:
            reason += ", " + ", ".join(
                f"{key}={value}"
                for key, value in self.removed_reason_parameters.items()
            )
        if self.equality == Equality.EQUALITY_EQUAL_TO_ZERO:
            return f"RemovedConstraint({self.function.__repr__()} == 0, {reason})"
        if self.equality == Equality.EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO:
            return f"RemovedConstraint({self.function.__repr__()} <= 0, {reason})"
        return self.raw.__repr__()

    @property
    def equality(self) -> Equality.ValueType:
        return self.raw.constraint.equality

    @property
    def id(self) -> int:
        return self.raw.constraint.id

    @property
    def function(self) -> Function:
        return Function(self.raw.constraint.function)

    @property
    def name(self) -> str | None:
        return (
            self.raw.constraint.name if self.raw.constraint.HasField("name") else None
        )

    @property
    def description(self) -> str | None:
        return (
            self.raw.constraint.description
            if self.raw.constraint.HasField("description")
            else None
        )

    @property
    def subscripts(self) -> list[int]:
        return list(self.raw.constraint.subscripts)

    @property
    def parameters(self) -> dict[str, str]:
        return dict(self.raw.constraint.parameters)

    @property
    def removed_reason(self) -> str:
        return self.raw.removed_reason

    @property
    def removed_reason_parameters(self) -> dict[str, str]:
        return dict(self.raw.removed_reason_parameters)

    def _as_pandas_entry(self) -> dict:
        return (
            Constraint.from_raw(self.raw.constraint)._as_pandas_entry()
            | {"removed_reason": self.removed_reason}
            | {
                f"removed_reason.{key}": value
                for key, value in self.removed_reason_parameters.items()
            }
        )
