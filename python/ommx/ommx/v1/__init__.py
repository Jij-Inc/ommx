from __future__ import annotations
from typing import Optional, Iterable, overload, Mapping
from typing_extensions import deprecated, TypeAlias, Union
from dataclasses import dataclass, field
from pandas import DataFrame, NA, Series
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
from .sample_set_pb2 import (
    SampleSet as _SampleSet,
    Samples,
    SampledValues as _SampledValues,
    SampledConstraint as _SampledConstraint,
)
from .annotation import (
    UserAnnotationBase,
    str_annotation_property,
    str_list_annotation_property,
    datetime_annotation_property,
    json_annotation_property,
    int_annotation_property,
)

from .. import _ommx_rust

__all__ = [
    "Instance",
    "ParametricInstance",
    "Solution",
    "Constraint",
    "SampleSet",
    # Function and its bases
    "DecisionVariable",
    "Parameter",
    "Linear",
    "Quadratic",
    "Polynomial",
    "Function",
    # Imported from protobuf
    "State",
    "Samples",
    "Parameters",
    "Optimality",
    "Relaxation",
    "Bound",
    # Utility
    "SampledValues",
    # Type Alias
    "ToState",
    "ToSamples",
]

ToState: TypeAlias = Union[State, Mapping[int, float]]
"""
Type alias for convertible types to :class:`State`.
"""


def to_state(state: ToState) -> State:
    if isinstance(state, State):
        return state
    return State(entries=state)


ToSamples: TypeAlias = Union[Samples, Mapping[int, ToState], list[ToState]]
"""
Type alias for convertible types to :class:`Samples`.
"""


def to_samples(samples: ToSamples) -> Samples:
    if isinstance(samples, list):
        samples = {i: state for i, state in enumerate(samples)}
    if not isinstance(samples, Samples):
        # Do not compress the samples
        samples = Samples(
            entries=[
                Samples.SamplesEntry(state=to_state(state), ids=[i])
                for i, state in samples.items()
            ]
        )
    return samples


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
        df = DataFrame(v._as_pandas_entry() for v in self.get_decision_variables())
        if not df.empty:
            df = df.set_index("id")
        return df

    @property
    def constraints(self) -> DataFrame:
        df = DataFrame(c._as_pandas_entry() for c in self.get_constraints())
        if not df.empty:
            df = df.set_index("id")
        return df

    @property
    def removed_constraints(self) -> DataFrame:
        df = DataFrame(rc._as_pandas_entry() for rc in self.get_removed_constraints())
        if not df.empty:
            df = df.set_index("id")
        return df


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
    annotations: dict[str, str] = field(default_factory=dict)
    """
    Arbitrary annotations stored in OMMX artifact. Use :py:attr:`title` or other specific attributes if possible.
    """
    annotation_namespace = "org.ommx.v1.instance"
    title = str_annotation_property("title")
    "The title of the instance, stored as ``org.ommx.v1.instance.title`` annotation in OMMX artifact."
    license = str_annotation_property("license")
    "License of this instance in the SPDX license identifier. This is stored as ``org.ommx.v1.instance.license`` annotation in OMMX artifact."
    dataset = str_annotation_property("dataset")
    "Dataset name which this instance belongs to, stored as ``org.ommx.v1.instance.dataset`` annotation in OMMX artifact."
    authors = str_list_annotation_property("authors")
    "Authors of this instance, stored as ``org.ommx.v1.instance.authors`` annotation in OMMX artifact."
    num_variables = int_annotation_property("variables")
    "Number of variables in this instance, stored as ``org.ommx.v1.instance.variables`` annotation in OMMX artifact."
    num_constraints = int_annotation_property("constraints")
    "Number of constraints in this instance, stored as ``org.ommx.v1.instance.constraints`` annotation in OMMX artifact."
    created = datetime_annotation_property("created")
    "The creation date of the instance, stored as ``org.ommx.v1.instance.created`` annotation in RFC3339 format in OMMX artifact."

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

    @staticmethod
    def load_qplib(path: str) -> Instance:
        bytes = _ommx_rust.load_qplib_bytes(path)
        return Instance.from_bytes(bytes)

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

    def evaluate(self, state: ToState) -> Solution:
        out, _ = _ommx_rust.evaluate_instance(
            self.to_bytes(), to_state(state).SerializeToString()
        )
        return Solution.from_bytes(out)

    def partial_evaluate(self, state: ToState) -> Instance:
        out, _ = _ommx_rust.partial_evaluate_instance(
            self.to_bytes(), to_state(state).SerializeToString()
        )
        return Instance.from_bytes(out)

    def as_minimization_problem(self):
        """
        Convert the instance to a minimization problem.

        If the instance is already a minimization problem, this does nothing.

        Examples
        =========

        .. doctest::

            >>> from ommx.v1 import Instance, DecisionVariable
            >>> x = [DecisionVariable.binary(i) for i in range(3)]
            >>> instance = Instance.from_components(
            ...     decision_variables=x,
            ...     objective=sum(x),
            ...     constraints=[sum(x) == 1],
            ...     sense=Instance.MAXIMIZE,
            ... )
            >>> instance.sense == Instance.MAXIMIZE
            True
            >>> instance.objective
            Function(x0 + x1 + x2)

            Convert to a minimization problem

            >>> instance.as_minimization_problem()
            >>> instance.sense == Instance.MINIMIZE
            True
            >>> instance.objective
            Function(-x0 - x1 - x2)

        """
        if self.raw.sense == Instance.MINIMIZE:
            return
        self.raw.sense = Instance.MINIMIZE
        obj = -self.objective
        self.raw.objective.CopyFrom(obj.raw)

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

    def evaluate_samples(self, samples: ToSamples) -> SampleSet:
        """
        Evaluate the instance with multiple states.
        """
        instance = _ommx_rust.Instance.from_bytes(self.to_bytes())
        samples_ = _ommx_rust.Samples.from_bytes(
            to_samples(samples).SerializeToString()
        )
        return SampleSet.from_bytes(instance.evaluate_samples(samples_).to_bytes())

    def relax_constraint(self, constraint_id: int, reason: str, **parameters):
        """
        Remove a constraint from the instance. The removed constraint is stored in :py:attr:`~Instance.removed_constraints`, and can be restored by :py:meth:`restore_constraint`.

        :param constraint_id: The ID of the constraint to remove.
        :param reason: The reason why the constraint is removed.
        :param parameters: Additional parameters to describe the reason.

        Examples
        =========

        Relax constraint, and restore it.

        .. doctest::

            >>> from ommx.v1 import Instance, DecisionVariable
            >>> x = [DecisionVariable.binary(i) for i in range(3)]
            >>> instance = Instance.from_components(
            ...     decision_variables=x,
            ...     objective=sum(x),
            ...     constraints=[(sum(x) == 3).set_id(1)],
            ...     sense=Instance.MAXIMIZE,
            ... )
            >>> instance.get_constraints()
            [Constraint(Function(x0 + x1 + x2 - 3) == 0)]

            >>> instance.relax_constraint(1, "manual relaxation")
            >>> instance.get_constraints()
            []
            >>> instance.get_removed_constraints()
            [RemovedConstraint(Function(x0 + x1 + x2 - 3) == 0, reason=manual relaxation)]

            >>> instance.restore_constraint(1)
            >>> instance.get_constraints()
            [Constraint(Function(x0 + x1 + x2 - 3) == 0)]
            >>> instance.get_removed_constraints()
            []

        Evaluate relaxed instance, and show :py:attr:`~Solution.feasible_unrelaxed`.

        .. doctest::

            >>> from ommx.v1 import Instance, DecisionVariable
            >>> x = [DecisionVariable.binary(i) for i in range(3)]
            >>> instance = Instance.from_components(
            ...     decision_variables=x,
            ...     objective=sum(x),
            ...     constraints=[
            ...         (x[0] + x[1] == 2).set_id(0),
            ...         (x[1] + x[2] == 2).set_id(1),
            ...     ],
            ...     sense=Instance.MINIMIZE,
            ... )

            For x0=0, x1=1, x2=1
            - x0 + x1 == 2 is not feasible
            - x1 + x2 == 2 is feasible

            >>> solution = instance.evaluate({0: 0, 1: 1, 2: 1})
            >>> solution.feasible_relaxed
            False
            >>> solution.feasible_unrelaxed
            False

            Relax the constraint: x0 + x1 == 2

            >>> instance.relax_constraint(0, "testing")
            >>> solution = instance.evaluate({0: 0, 1: 1, 2: 1})
            >>> solution.feasible_relaxed
            True
            >>> solution.feasible_unrelaxed
            False

        """
        instance = _ommx_rust.Instance.from_bytes(self.to_bytes())
        instance.relax_constraint(constraint_id, reason, parameters)
        self.raw.ParseFromString(instance.to_bytes())

    def restore_constraint(self, constraint_id: int):
        """
        Restore a removed constraint to the instance.

        :param constraint_id: The ID of the constraint to restore.

        Note that this drops the removed reason and associated parameters. See :py:meth:`relax_constraint` for details.
        """
        instance = _ommx_rust.Instance.from_bytes(self.to_bytes())
        instance.restore_constraint(constraint_id)
        self.raw.ParseFromString(instance.to_bytes())


@dataclass
class ParametricInstance(InstanceBase, UserAnnotationBase):
    """
    Idiomatic wrapper of ``ommx.v1.ParametricInstance`` protobuf message.

    Examples
    =========

    Create an instance for KnapSack Problem with parameters

    .. doctest::

        >>> from ommx.v1 import ParametricInstance, DecisionVariable, Parameter

        Decision variables

        >>> x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(6)]

        Profit and weight of items as parameters

        >>> p = [Parameter.new(id=i+6, name="Profit", subscripts=[i]) for i in range(6)]
        >>> w = [Parameter.new(id=i+12, name="Weight", subscripts=[i]) for i in range(6)]
        >>> W = Parameter.new(id=18, name="Capacity")

        Objective and constraint

        >>> objective = sum(p[i] * x[i] for i in range(6))
        >>> constraint = sum(w[i] * x[i] for i in range(6)) <= W

        Compose as an instance

        >>> parametric_instance = ParametricInstance.from_components(
        ...     decision_variables=x,
        ...     parameters=p + w + [W],
        ...     objective=objective,
        ...     constraints=[constraint],
        ...     sense=Instance.MAXIMIZE,
        ... )

        Substitute parameters to get an instance

        >>> p_values = { x.id: value for x, value in zip(p, [10, 13, 18, 31, 7, 15]) }
        >>> w_values = { x.id: value for x, value in zip(w, [11, 15, 20, 35, 10, 33]) }
        >>> W_value = { W.id: 47 }
        >>> instance = parametric_instance.with_parameters({**p_values, **w_values, **W_value})

    """

    raw: _ParametricInstance

    annotations: dict[str, str] = field(default_factory=dict)
    annotation_namespace = "org.ommx.v1.parametric-instance"
    title = str_annotation_property("title")
    "The title of the instance, stored as ``org.ommx.v1.parametric-instance.title`` annotation in OMMX artifact."
    license = str_annotation_property("license")
    "License of this instance in the SPDX license identifier. This is stored as ``org.ommx.v1.parametric-instance.license`` annotation in OMMX artifact."
    dataset = str_annotation_property("dataset")
    "Dataset name which this instance belongs to, stored as ``org.ommx.v1.parametric-instance.dataset`` annotation in OMMX artifact."
    authors = str_list_annotation_property("authors")
    "Authors of this instance, stored as ``org.ommx.v1.parametric-instance.authors`` annotation in OMMX artifact."
    num_variables = int_annotation_property("variables")
    "Number of variables in this instance, stored as ``org.ommx.v1.parametric-instance.variables`` annotation in OMMX artifact."
    num_constraints = int_annotation_property("constraints")
    "Number of constraints in this instance, stored as ``org.ommx.v1.parametric-instance.constraints`` annotation in OMMX artifact."
    created = datetime_annotation_property("created")
    "The creation date of the instance, stored as ``org.ommx.v1.parametric-instance.created`` annotation in RFC3339 format in OMMX artifact."

    @property
    def _annotations(self) -> dict[str, str]:
        return self.annotations

    @staticmethod
    def empty() -> ParametricInstance:
        """
        Create trivial empty instance of minimization with zero objective, no constraints, and no decision variables and parameters.
        """
        return ParametricInstance.from_components(
            objective=0,
            constraints=[],
            sense=Instance.MINIMIZE,
            decision_variables=[],
            parameters=[],
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
        parameters: Iterable[Parameter | _Parameter],
        description: Optional[_Instance.Description] = None,
    ) -> ParametricInstance:
        return ParametricInstance(
            _ParametricInstance(
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
                parameters=[
                    p.raw if isinstance(p, Parameter) else p for p in parameters
                ],
            )
        )

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
        df = DataFrame(p._as_pandas_entry() for p in self.get_parameters())
        if not df.empty:
            df = df.set_index("id")
        return df

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
    def new(
        id: int,
        *,
        name: Optional[str] = None,
        subscripts: Iterable[int] = [],
        description: Optional[str] = None,
    ):
        return Parameter(
            _Parameter(
                id=id,
                name=name,
                subscripts=subscripts,
                description=description,
            )
        )

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

    annotation_namespace = "org.ommx.v1.solution"
    instance = str_annotation_property("instance")
    """
    The digest of the instance layer, stored as ``org.ommx.v1.solution.instance`` annotation in OMMX artifact.

    This ``Solution`` is the solution of the mathematical programming problem described by the instance.
    """
    solver = json_annotation_property("solver")
    """The solver which generated this solution, stored as ``org.ommx.v1.solution.solver`` annotation as a JSON in OMMX artifact."""
    parameters = json_annotation_property("parameters")
    """The parameters used in the optimization, stored as ``org.ommx.v1.solution.parameters`` annotation as a JSON in OMMX artifact."""
    start = datetime_annotation_property("start")
    """When the optimization started, stored as ``org.ommx.v1.solution.start`` annotation in RFC3339 format in OMMX artifact."""
    end = datetime_annotation_property("end")
    """When the optimization ended, stored as ``org.ommx.v1.solution.end`` annotation in RFC3339 format in OMMX artifact."""
    annotations: dict[str, str] = field(default_factory=dict)
    """Arbitrary annotations stored in OMMX artifact. Use :py:attr:`parameters` or other specific attributes if possible."""

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
        df = DataFrame(
            DecisionVariable(v)._as_pandas_entry()
            | {"value": self.raw.state.entries[v.id]}
            for v in self.raw.decision_variables
        )
        if not df.empty:
            df = df.set_index("id")
        return df

    @property
    def constraints(self) -> DataFrame:
        df = DataFrame(
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
        )
        if not df.empty:
            df = df.set_index("id")
        return df

    def extract_decision_variables(self, name: str) -> dict[tuple[int, ...], float]:
        """
        Extract the values of decision variables based on the `name` with `subscripts` key.

        :raises ValueError: If the decision variable with parameters is found, or if the same subscript is found.

        Examples
        =========

        .. doctest::

            >>> from ommx.v1 import Instance, DecisionVariable
            >>> x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
            >>> instance = Instance.from_components(
            ...     decision_variables=x,
            ...     objective=sum(x),
            ...     constraints=[sum(x) == 1],
            ...     sense=Instance.MAXIMIZE,
            ... )
            >>> solution = instance.evaluate({i: 1 for i in range(3)})
            >>> solution.extract_decision_variables("x")
            {(0,): 1.0, (1,): 1.0, (2,): 1.0}

        """
        out = {}
        for v in self.raw.decision_variables:
            if v.name != name:
                continue
            if v.parameters:
                raise ValueError("Decision variable with parameters is not supported")
            key = tuple(v.subscripts)
            if key in out:
                raise ValueError(f"Duplicate subscript: {key}")
            out[key] = self.state.entries[v.id]
        return out

    def extract_constraints(self, name: str) -> dict[tuple[int, ...], float]:
        """
        Extract the values of constraints based on the `name` with `subscripts` key.

        :raises ValueError: If the constraint with parameters is found, or if the same subscript is found.

        Examples
        =========

        .. doctest::

            >>> from ommx.v1 import Instance, DecisionVariable
            >>> x = [DecisionVariable.binary(i) for i in range(3)]
            >>> c0 = (x[0] + x[1] == 1).add_name("c").add_subscripts([0])
            >>> c1 = (x[1] + x[2] == 1).add_name("c").add_subscripts([1])
            >>> instance = Instance.from_components(
            ...     decision_variables=x,
            ...     objective=sum(x),
            ...     constraints=[c0, c1],
            ...     sense=Instance.MAXIMIZE,
            ... )
            >>> solution = instance.evaluate({0: 1, 1: 0, 2: 1})
            >>> solution.extract_constraints("c")
            {(0,): 0.0, (1,): 0.0}

        """
        out = {}
        for c in self.raw.evaluated_constraints:
            if c.name != name:
                continue
            if c.parameters:
                raise ValueError("Constraint with parameters is not supported")
            key = tuple(c.subscripts)
            if key in out:
                raise ValueError(f"Duplicate subscript: {key}")
            out[key] = c.evaluated_value
        return out

    @property
    def feasible(self) -> bool:
        """
        Feasibility of the solution in terms of all constraints, including :py:attr:`~Instance.removed_constraints`.

        This is an alias for :py:attr:`~Solution.feasible_unrelaxed`.

        Compatibility
        -------------
        The meaning of this property has changed from Python SDK 1.7.0.
        Previously, this property represents the feasibility of the remaining constraints only, i.e. excluding relaxed constraints.
        From Python SDK 1.7.0, this property represents the feasibility of all constraints, including relaxed constraints.
        """
        return self.feasible_unrelaxed

    @property
    def feasible_relaxed(self) -> bool:
        """
        Feasibility of the solution in terms of remaining constraints, not including relaxed (removed) constraints.
        """
        # For compatibility: object created by 1.6.0 only contains `feasible` field and does not contain `feasible_relaxed` field.
        if self.raw.HasField("feasible_relaxed"):
            return self.raw.feasible_relaxed
        else:
            return self.raw.feasible

    @property
    def feasible_unrelaxed(self) -> bool:
        """
        Feasibility of the solution in terms of all constraints, including relaxed (removed) constraints.
        """
        if self.raw.HasField("feasible_relaxed"):
            return self.raw.feasible
        else:
            return self.raw.feasible_unrelaxed

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

    def evaluate(self, state: ToState) -> tuple[float, set]:
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
        return _ommx_rust.evaluate_linear(
            self.to_bytes(), to_state(state).SerializeToString()
        )

    def partial_evaluate(self, state: ToState) -> tuple[Linear, set]:
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
        new, used_ids = _ommx_rust.partial_evaluate_linear(
            self.to_bytes(), to_state(state).SerializeToString()
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

    def evaluate(self, state: ToState) -> tuple[float, set]:
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
        return _ommx_rust.evaluate_quadratic(
            self.to_bytes(), to_state(state).SerializeToString()
        )

    def partial_evaluate(self, state: ToState) -> tuple[Quadratic, set]:
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
        new, used_ids = _ommx_rust.partial_evaluate_quadratic(
            self.to_bytes(), to_state(state).SerializeToString()
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

    def evaluate(self, state: ToState) -> tuple[float, set]:
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
        return _ommx_rust.evaluate_polynomial(
            self.to_bytes(), to_state(state).SerializeToString()
        )

    def partial_evaluate(self, state: ToState) -> tuple[Polynomial, set]:
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
        new, used_ids = _ommx_rust.partial_evaluate_polynomial(
            self.to_bytes(), to_state(state).SerializeToString()
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

    def evaluate(self, state: ToState) -> tuple[float, set]:
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
        return _ommx_rust.evaluate_function(
            self.to_bytes(), to_state(state).SerializeToString()
        )

    def partial_evaluate(self, state: ToState) -> tuple[Function, set]:
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
        new, used_ids = _ommx_rust.partial_evaluate_function(
            self.to_bytes(), to_state(state).SerializeToString()
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


@dataclass
class SampleSet(UserAnnotationBase):
    r"""
    The output of sampling-based optimization algorithms, e.g. simulated annealing (SA).

    - Similar to :class:`Solution` rather than :class:`solution_pb2.State`.
      This class contains the sampled values of decision variables with the objective value, constraint violations,
      feasibility, and metadata of constraints and decision variables.
    - This class is usually created via :meth:`Instance.evaluate_samples`.

    Examples
    =========

    Let's consider a simple optimization problem:

    .. math::

        \begin{align*}
            \max &\quad x_1 + 2 x_2 + 3 x_3 \\
            \text{s.t.} &\quad x_1 + x_2 + x_3 = 1 \\
            &\quad x_1, x_2, x_3 \in \{0, 1\}
        \end{align*}

    .. doctest::

        >>> x = [DecisionVariable.binary(i) for i in range(3)]
        >>> instance = Instance.from_components(
        ...     decision_variables=x,
        ...     objective=x[0] + 2*x[1] + 3*x[2],
        ...     constraints=[sum(x) == 1],
        ...     sense=Instance.MAXIMIZE,
        ... )

    with three samples:

    .. doctest::

        >>> samples = {
        ...     0: {0: 1, 1: 0, 2: 0},  # x1 = 1, x2 = x3 = 0
        ...     1: {0: 0, 1: 0, 2: 1},  # x3 = 1, x1 = x2 = 0
        ...     2: {0: 1, 1: 1, 2: 0},  # x1 = x2 = 1, x3 = 0 (infeasible)
        ... } # ^ sample ID

    Note that this will be done by sampling-based solvers, but we do it manually here.
    We can evaluate the samples with via :meth:`Instance.evaluate_samples`:

    .. doctest::

        >>> sample_set = instance.evaluate_samples(samples)
        >>> sample_set.summary  # doctest: +NORMALIZE_WHITESPACE
                   objective  feasible
        sample_id                     
        1                3.0      True
        0                1.0      True
        2                3.0     False

    The :attr:`summary` attribute shows the objective value, feasibility of each sample.
    Note that this `feasible` column represents the feasibility of the original constraints, not the relaxed constraints.
    You can get each samples by :meth:`get` as a :class:`Solution` format:

    .. doctest::

        >>> solution = sample_set.get(sample_id=0)
        >>> solution.objective
        1.0
        >>> solution.decision_variables  # doctest: +NORMALIZE_WHITESPACE
              kind  lower  upper  name subscripts description substituted_value  value
        id
        0   binary    0.0    1.0  <NA>         []        <NA>              <NA>    1.0
        1   binary    0.0    1.0  <NA>         []        <NA>              <NA>    0.0
        2   binary    0.0    1.0  <NA>         []        <NA>              <NA>    0.0

    :meth:`best_feasible` returns the best feasible sample, i.e. the largest objective value among feasible samples:

    .. doctest::

        >>> solution = sample_set.best_feasible()
        >>> solution.objective
        3.0
        >>> solution.decision_variables  # doctest: +NORMALIZE_WHITESPACE
              kind  lower  upper  name subscripts description substituted_value  value
        id                                                                            
        0   binary    0.0    1.0  <NA>         []        <NA>              <NA>    0.0
        1   binary    0.0    1.0  <NA>         []        <NA>              <NA>    0.0
        2   binary    0.0    1.0  <NA>         []        <NA>              <NA>    1.0

    Of course, the sample of smallest objective value is returned for minimization problems.

    """

    raw: _SampleSet

    annotation_namespace = "org.ommx.v1.sample-set"
    instance = str_annotation_property("instance")
    """The digest of the instance layer, stored as ``org.ommx.v1.sample-set.instance`` annotation in OMMX artifact."""
    solver = json_annotation_property("solver")
    """The solver which generated this sample set, stored as ``org.ommx.v1.sample-set.solver`` annotation as a JSON in OMMX artifact."""
    parameters = json_annotation_property("parameters")
    """The parameters used in the optimization, stored as ``org.ommx.v1.sample-set.parameters`` annotation as a JSON in OMMX artifact."""
    start = datetime_annotation_property("start")
    """When the optimization started, stored as ``org.ommx.v1.sample-set.start`` annotation in RFC3339 format in OMMX artifact."""
    end = datetime_annotation_property("end")
    """When the optimization ended, stored as ``org.ommx.v1.sample-set.end`` annotation in RFC3339 format in OMMX artifact."""
    annotations: dict[str, str] = field(default_factory=dict)
    """Arbitrary annotations stored in OMMX artifact. Use :py:attr:`parameters` or other specific attributes if possible."""

    @property
    def _annotations(self) -> dict[str, str]:
        return self.annotations

    @staticmethod
    def from_bytes(data: bytes) -> SampleSet:
        new = SampleSet(_SampleSet())
        new.raw.ParseFromString(data)
        return new

    def to_bytes(self) -> bytes:
        return self.raw.SerializeToString()

    @property
    def summary(self) -> DataFrame:
        feasible = self.feasible
        df = DataFrame(
            {
                "sample_id": id,
                "objective": value,
                "feasible": feasible[id],
            }
            for id, value in self.objectives.items()
        )
        if df.empty:
            return df

        return df.sort_values(
            by=["feasible", "objective"],
            ascending=[False, self.raw.sense == Instance.MINIMIZE],
        ).set_index("sample_id")

    @property
    def summary_with_constraints(self) -> DataFrame:
        def _constraint_label(c: _SampledConstraint) -> str:
            name = ""
            if c.HasField("name"):
                name += c.name
            else:
                return f"{c.id}"
            if c.subscripts:
                name += f"{c.subscripts}"
            if c.parameters:
                name += f"{c.parameters}"
            return name

        feasible = self.feasible
        df = DataFrame(
            {
                "sample_id": id,
                "objective": value,
                "feasible": feasible[id],
            }
            | {_constraint_label(c): c.feasible[id] for c in self.raw.constraints}
            for id, value in self.objectives.items()
        )

        if df.empty:
            return df
        df = df.sort_values(
            by=["feasible", "objective"],
            ascending=[False, self.raw.sense == Instance.MINIMIZE],
        ).set_index("sample_id")
        return df

    @property
    def feasible(self) -> dict[int, bool]:
        """
        Feasibility in terms of the original constraints, an alias to :attr:`feasible_unrelaxed`.

        Compatibility
        -------------
        The meaning of this property has changed from Python SDK 1.7.0.
        Previously, this property represents the feasibility of the remaining constraints only, i.e. excluding relaxed constraints.
        From Python SDK 1.7.0, this property represents the feasibility of all constraints, including relaxed constraints.
        """
        return self.feasible_unrelaxed

    @property
    def feasible_relaxed(self) -> dict[int, bool]:
        """
        Feasibility in terms of the remaining (non-removed) constraints.

        For each `sample_id`, this property shows whether the sample is feasible for the all :attr:`Instance.constraints`
        """
        if len(self.raw.feasible_relaxed) > 0:
            return dict(self.raw.feasible_relaxed)
        else:
            return dict(self.raw.feasible)

    @property
    def feasible_unrelaxed(self) -> dict[int, bool]:
        """
        Feasibility in terms of the original constraints without relaxation.

        For each `sample_id`, this property shows whether the sample is feasible
        both for the all :attr:`Instance.constraints` and all :attr:`Instance.removed_constraints`.
        """
        if len(self.raw.feasible_relaxed) > 0:
            # After 1.7.0
            return dict(self.raw.feasible)
        else:
            # Before 1.7.0
            return dict(self.raw.feasible_unrelaxed)

    @property
    def objectives(self) -> dict[int, float]:
        return dict(SampledValues(self.raw.objectives))

    @property
    def sample_ids(self) -> list[int]:
        return self.summary.index.tolist()  # type: ignore[attr-defined]

    @property
    def decision_variables(self) -> DataFrame:
        df = DataFrame(
            DecisionVariable(v.decision_variable)._as_pandas_entry()
            | {id: value for id, value in SampledValues(v.samples)}
            for v in self.raw.decision_variables
        )
        if not df.empty:
            return df.set_index("id")
        return df

    @property
    def constraints(self) -> DataFrame:
        df = DataFrame(
            {
                "id": c.id,
                "equality": _equality(c.equality),
                "used_ids": set(c.used_decision_variable_ids),
                "name": c.name if c.HasField("name") else NA,
                "subscripts": c.subscripts,
                "description": c.description if c.HasField("description") else NA,
                "removed_reason": c.removed_reason
                if c.HasField("removed_reason")
                else NA,
            }
            | {
                f"removed_reason.{key}": value
                for key, value in c.removed_reason_parameters.items()
            }
            | {f"value.{id}": value for id, value in SampledValues(c.evaluated_values)}
            | {f"feasible.{id}": value for id, value in c.feasible.items()}
            for c in self.raw.constraints
        )
        if not df.empty:
            return df.set_index("id")
        return df

    def extract_decision_variables(
        self, name: str, sample_id: int
    ) -> dict[tuple[int, ...], float]:
        """
        Extract sampled decision variable values for a given name and sample ID.
        """
        out = {}
        for sampled_decision_variable in self.raw.decision_variables:
            v = sampled_decision_variable.decision_variable
            if v.name != name:
                continue
            key = tuple(v.subscripts)
            if key in out:
                raise ValueError(
                    f"Duplicate decision variable subscript: {v.subscripts}"
                )

            if v.HasField("substituted_value"):
                out[key] = v.substituted_value
                continue
            out[key] = SampledValues(sampled_decision_variable.samples)[sample_id]
        return out

    def extract_constraints(
        self, name: str, sample_id: int
    ) -> dict[tuple[int, ...], float]:
        """
        Extract evaluated constraint violations for a given constraint name and sample ID.
        """
        out = {}
        for c in self.raw.constraints:
            if c.name != name:
                continue
            key = tuple(c.subscripts)
            if key in out:
                raise ValueError(f"Duplicate constraint subscript: {c.subscripts}")
            out[key] = SampledValues(c.evaluated_values)[sample_id]
        return out

    def get(self, sample_id: int) -> Solution:
        """
        Get a sample for a given ID as a solution format
        """
        solution = _ommx_rust.SampleSet.from_bytes(self.to_bytes()).get(sample_id)
        return Solution.from_bytes(solution.to_bytes())

    def best_feasible(self) -> Solution:
        """
        Get the best feasible solution
        """
        solution = _ommx_rust.SampleSet.from_bytes(self.to_bytes()).best_feasible()
        return Solution.from_bytes(solution.to_bytes())

    def best_feasible_unrelaxed(self) -> Solution:
        """
        Get the best feasible solution without relaxation
        """
        solution = _ommx_rust.SampleSet.from_bytes(
            self.to_bytes()
        ).best_feasible_unrelaxed()
        return Solution.from_bytes(solution.to_bytes())


@dataclass
class SampledValues:
    raw: _SampledValues

    def as_series(self) -> Series:
        return Series(dict(self))

    def __iter__(self):
        for entry in self.raw.entries:
            for id in entry.ids:
                yield id, entry.value

    def __getitem__(self, sample_id: int) -> float:
        for entry in self.raw.entries:
            if sample_id in entry.ids:
                return entry.value
        raise KeyError(f"Sample ID {sample_id} not found")

    def __repr__(self) -> str:
        return self.as_series().__repr__()
