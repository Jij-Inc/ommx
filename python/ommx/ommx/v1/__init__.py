from __future__ import annotations
from typing import Optional, Iterable, overload, Mapping
from typing_extensions import deprecated, TypeAlias, Union, Sequence
from dataclasses import dataclass, field
from pandas import DataFrame, NA
from abc import ABC, abstractmethod
import copy


from .instance_pb2 import Instance as _Instance, Parameters
from .function_pb2 import Function as _Function
from .constraint_pb2 import (
    Constraint as _Constraint,
    RemovedConstraint as _RemovedConstraint,
)
from .decision_variables_pb2 import DecisionVariable as _DecisionVariable
from .parametric_instance_pb2 import (
    ParametricInstance as _ParametricInstance,
    Parameter as _Parameter,
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

# Define PyO3 types
State = _ommx_rust.State
Samples = _ommx_rust.Samples

# Import PyO3 enums
Sense = _ommx_rust.Sense
Equality = _ommx_rust.Equality
Kind = _ommx_rust.Kind
Optimality = _ommx_rust.Optimality
Relaxation = _ommx_rust.Relaxation

# Import constraint hints classes directly
OneHot = _ommx_rust.OneHot
Sos1 = _ommx_rust.Sos1
ConstraintHints = _ommx_rust.ConstraintHints

# Import Rng class
Rng = _ommx_rust.Rng

# Import evaluated classes
EvaluatedDecisionVariable = _ommx_rust.EvaluatedDecisionVariable
EvaluatedConstraint = _ommx_rust.EvaluatedConstraint
SampledDecisionVariable = _ommx_rust.SampledDecisionVariable
SampledConstraint = _ommx_rust.SampledConstraint

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
    # Constraint hints
    "OneHot",
    "Sos1",
    "ConstraintHints",
    # Imported from protobuf
    "State",
    "Samples",
    "Parameters",
    "Optimality",
    "Relaxation",
    "Bound",
    # Enums
    "Sense",
    "Equality",
    "Kind",
    # Utility
    "Rng",
    # Evaluated types
    "EvaluatedDecisionVariable",
    "EvaluatedConstraint",
    "SampledDecisionVariable",
    "SampledConstraint",
    # Type Alias
    "ToState",
    "ToSamples",
]

ToState: TypeAlias = Union[State, Mapping[int, float]]
"""
Type alias for convertible types to :class:`State`.
"""


ToSamples: TypeAlias = Union[Samples, Mapping[int, ToState], Sequence[ToState]]
"""
Type alias for convertible types to :class:`Samples`.
"""


@dataclass
class Instance(UserAnnotationBase):
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

    raw: _ommx_rust.Instance
    """The raw Rust instance."""

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
    MAXIMIZE = _ommx_rust.Sense.Maximize
    MINIMIZE = _ommx_rust.Sense.Minimize

    # Expose InstanceDescription as Instance.Description for consistency
    Description = _ommx_rust.InstanceDescription

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
        | Function
        | _Function
        | _ommx_rust.Function,
        constraints: Iterable[Constraint | _Constraint],
        sense: _ommx_rust.Sense,
        decision_variables: Iterable[DecisionVariable | _DecisionVariable],
        description: Optional["Instance.Description | _Instance.Description"] = None,
        constraint_hints: Optional[ConstraintHints] = None,
    ) -> Instance:
        if not isinstance(objective, Function):
            objective = Function(objective)

        # Convert decision variables to _ommx_rust.DecisionVariable
        rust_decision_variables = {}
        for v in decision_variables:
            if isinstance(v, DecisionVariable):
                rust_decision_variables[v.id] = v.raw
            else:
                # Convert protobuf to DecisionVariable first
                dv = DecisionVariable.from_protobuf(v)
                rust_decision_variables[dv.id] = dv.raw

        # Convert constraints to _ommx_rust.Constraint
        rust_constraints = {}
        for c in constraints:
            if isinstance(c, Constraint):
                rust_constraints[c.id] = c.raw
            else:
                # Convert protobuf to Constraint first
                constraint = Constraint.from_protobuf(c)
                rust_constraints[constraint.id] = constraint.raw

        # Convert description if provided
        rust_description = None
        if description is not None:
            if isinstance(description, _ommx_rust.InstanceDescription):
                # Already a Rust InstanceDescription
                rust_description = description
            else:
                # Convert Protocol Buffer Description to Rust InstanceDescription
                rust_description = _ommx_rust.InstanceDescription(
                    name=description.name if description.HasField("name") else None,
                    description=description.description
                    if description.HasField("description")
                    else None,
                    authors=list(description.authors) if description.authors else None,
                    created_by=description.created_by
                    if description.HasField("created_by")
                    else None,
                )

        # Convert constraint hints if provided
        rust_constraint_hints = constraint_hints

        # Create Rust instance
        rust_instance = _ommx_rust.Instance.from_components(
            sense=sense,
            objective=objective.raw,
            decision_variables=rust_decision_variables,
            constraints=rust_constraints,
            description=rust_description,
            constraint_hints=rust_constraint_hints,
        )

        return Instance(rust_instance)

    @staticmethod
    def load_mps(path: str) -> Instance:
        raw = _ommx_rust.Instance.load_mps(path)
        return Instance(raw)

    @deprecated("Renamed to `save_mps`")
    def write_mps(self, path: str):
        self.save_mps(path)

    def save_mps(self, path: str, *, compress=True):
        """
        Outputs the instance as an MPS file.

        - The outputted file is optionally compressed by gzip, depending on the value of the `compress` parameter (default: True).
        - Only linear problems are supported.
        - Various forms of metadata, like problem description and variable/constraint names, are not preserved.
        """
        self.raw.save_mps(path, compress=compress)

    @staticmethod
    def load_qplib(path: str) -> Instance:
        raw = _ommx_rust.Instance.load_qplib(path)
        return Instance(raw)

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
        rust_instance = _ommx_rust.Instance.from_bytes(data)
        return Instance(rust_instance)

    def to_bytes(self) -> bytes:
        return self.raw.to_bytes()

    @property
    def description(self) -> "Instance.Description | None":
        return self.raw.description

    @property
    def objective(self) -> Function:
        return Function.from_raw(self.raw.objective)

    @objective.setter
    def objective(
        self,
        value: int
        | float
        | DecisionVariable
        | Linear
        | Quadratic
        | Polynomial
        | Function,
    ):
        """
        Set the objective function.

        Examples
        ---------


        """
        if not isinstance(value, Function):
            value = Function(value)
        self.raw.objective = value.raw

    @property
    def sense(self) -> _ommx_rust.Sense:
        return self.raw.sense

    @property
    def constraint_hints(self) -> ConstraintHints:
        """Get constraint hints that provide additional information to solvers."""
        return self.raw.constraint_hints

    @property
    def decision_variables(self) -> list[DecisionVariable]:
        """
        Get decision variables as a list of :class:`DecisionVariable` instances sorted by their IDs.
        """
        return [DecisionVariable(dv) for dv in self.raw.decision_variables]

    @property
    def decision_variable_names(self) -> set[str]:
        """
        Get all unique decision variable names in this instance.

        Returns a set of all unique variable names. Variables without names are not included.

        Examples
        =========

        .. doctest::

            >>> from ommx.v1 import Instance, DecisionVariable
            >>> x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
            >>> y = [DecisionVariable.binary(i+3, name="y", subscripts=[i]) for i in range(2)]
            >>> instance = Instance.from_components(
            ...     decision_variables=x + y,
            ...     objective=sum(x) + sum(y),
            ...     constraints=[],
            ...     sense=Instance.MAXIMIZE,
            ... )
            >>> sorted(instance.decision_variable_names)
            ['x', 'y']

        """
        return self.raw.decision_variable_names

    @property
    def constraints(self) -> list[Constraint]:
        """
        Get constraints as a list of :class:`Constraint` instances sorted by their IDs.
        """
        return [Constraint.from_raw(c) for c in self.raw.constraints]

    @property
    def removed_constraints(self) -> list[RemovedConstraint]:
        """
        Get removed constraints as a list of :class:`RemovedConstraint` instances.
        """
        return [RemovedConstraint.from_raw(rc) for rc in self.raw.removed_constraints]

    def get_decision_variable_by_id(self, variable_id: int) -> DecisionVariable:
        """
        Get a decision variable by ID.
        """
        return DecisionVariable(self.raw.get_decision_variable_by_id(variable_id))

    def get_constraint_by_id(self, constraint_id: int) -> Constraint:
        """
        Get a constraint by ID.
        """
        return Constraint.from_raw(self.raw.get_constraint_by_id(constraint_id))

    def get_removed_constraint_by_id(
        self, removed_constraint_id: int
    ) -> RemovedConstraint:
        """
        Get a removed constraint by ID.
        """
        return RemovedConstraint.from_raw(
            self.raw.get_removed_constraint_by_id(removed_constraint_id)
        )

    @property
    def decision_variables_df(self) -> DataFrame:
        df = DataFrame(v._as_pandas_entry() for v in self.decision_variables)
        if not df.empty:
            df = df.set_index("id")
        return df

    @property
    def constraints_df(self) -> DataFrame:
        df = DataFrame(c._as_pandas_entry() for c in self.constraints)
        if not df.empty:
            df = df.set_index("id")
        return df

    @property
    def removed_constraints_df(self) -> DataFrame:
        df = DataFrame(rc._as_pandas_entry() for rc in self.removed_constraints)
        if not df.empty:
            df = df.set_index("id")
        return df

    def evaluate(self, state: ToState, *, atol: float | None = None) -> Solution:
        r"""
        Evaluate the given :class:`State` into a :class:`Solution`.
        
        This method evaluates the problem instance using the provided state (a map from decision variable IDs to their values),
        and returns a :class:`Solution` object containing objective value, evaluated constraint values, and feasibility information.
        
        Examples
        =========
        
        Create a simple instance with three binary variables and evaluate a solution:

        .. math::
            \begin{align*}
                \max & \space x_0 + x_1 + x_2 & \\
                \text{ s.t. } & \space x_0 + x_1 \leq 1 & \\
                & \space x_0, x_1, x_2 \in \{0, 1\}
            \end{align*}
            
        >>> from ommx.v1 import Instance, DecisionVariable
        >>> x = [DecisionVariable.binary(i) for i in range(3)]
        >>> instance = Instance.from_components(
        ...     decision_variables=x,
        ...     objective=sum(x),
        ...     constraints=[(x[0] + x[1] <= 1).set_id(0)],
        ...     sense=Instance.MAXIMIZE,
        ... )

        Evaluate it with a state :math:`x_0 = 1, x_1 = 0, x_2 = 0`, and show the objective and constraints:

        >>> solution = instance.evaluate({0: 1, 1: 0, 2: 0})
        >>> solution.objective
        1.0
        >>> solution.constraints_df.dropna(axis=1, how="all")  # doctest: +NORMALIZE_WHITESPACE
           equality  value used_ids subscripts
        id                                    
        0       <=0    0.0   {0, 1}         []

        The values of decision variables are also stored in the solution:

        >>> solution.decision_variables_df.dropna(axis=1, how="all")  # doctest: +NORMALIZE_WHITESPACE
              kind  lower  upper subscripts  value
        id                                        
        0   Binary    0.0    1.0         []    1.0
        1   Binary    0.0    1.0         []    0.0
        2   Binary    0.0    1.0         []    0.0

        If the value is out of the range, the solution is infeasible:

        >>> solution = instance.evaluate({0: 1, 1: 0, 2: 2})
        >>> solution.feasible
        False

        If some of the decision variables are not set, this raises an error:

        >>> instance.evaluate({0: 1, 1: 0})
        Traceback (most recent call last):
            ...
        RuntimeError: The state does not contain some required IDs: {VariableID(2)}

        Irrelevant decision variables
        -----------------------------

        Sometimes, the instance contains decision variables that are not used in the objective or constraints.

        >>> x = [DecisionVariable.binary(i) for i in range(3)]
        >>> instance = Instance.from_components(
        ...     decision_variables=x,
        ...     objective=x[0],
        ...     constraints=[(x[0] + x[1] == 1).set_id(0)],
        ...     sense=Instance.MAXIMIZE,
        ... )

        This instance does not contain the decision variable :math:`x_2` in the objective or constraints.
        We call such variables "irrelevant". This is mathematically meaningless,
        but sometimes useful in data science application.
        Since the irrelevant variables cannot be determined from the instance, solvers will ignore them,
        and do not return their values. This function works as well for such cases:

        >>> solution = instance.evaluate({0: 1, 1: 0})
        >>> solution.objective
        1.0
        >>> solution.constraints_df.dropna(axis=1, how="all")  # doctest: +NORMALIZE_WHITESPACE
           equality  value used_ids subscripts
        id                                    
        0        =0    0.0   {0, 1}         []

        The irrelevant decision variable :math:`x_2` is also stored in the :class:`Solution` with the value
        nearest to ``0`` within its bound. For example,

        * When the bound is :math:`[-1, 1]` or :math:`(-\infty, \infty)`, the value is ``0``
        * When the bound is :math:`[2, 5]`, the value is ``2``
        * When the bound is :math:`[-3, -1]`, the value is ``-1``

        >>> solution.decision_variables_df.dropna(axis=1, how="all")  # doctest: +NORMALIZE_WHITESPACE
              kind  lower  upper subscripts  value
        id                                        
        0   Binary    0.0    1.0         []    1.0
        1   Binary    0.0    1.0         []    0.0
        2   Binary    0.0    1.0         []    0.0
        
        """
        out = self.raw.evaluate(State(state).to_bytes(), atol=atol)
        return Solution(out)

    def partial_evaluate(
        self, state: ToState, *, atol: float | None = None
    ) -> Instance:
        """
        Creates a new instance with specific decision variables fixed to given values.

        This method substitutes the specified decision variables with their provided values,
        creating a new problem instance where these variables are fixed. This is useful for
        scenarios such as:

        - Creating simplified sub-problems with some variables fixed
        - Incrementally solving a problem by fixing some variables and optimizing the rest
        - Testing specific configurations of a problem

        :param state: Maps decision variable IDs to their fixed values.
                     Can be a :class:`~ommx.v1.State` object or a dictionary mapping variable IDs to values.
        :type state: :class:`~ommx.v1.ToState`
        :param atol: Absolute tolerance for floating point comparisons. If None, uses the default tolerance.
        :type atol: float | None
        :return: A new instance with the specified decision variables fixed to their given values.
        :rtype: :class:`~ommx.v1.Instance`

        Examples
        =========

        >>> from ommx.v1 import Instance, DecisionVariable
        >>> x = DecisionVariable.binary(1)
        >>> y = DecisionVariable.binary(2)
        >>> instance = Instance.from_components(
        ...     decision_variables=[x, y],
        ...     objective=x + y,
        ...     constraints=[x + y <= 1],
        ...     sense=Instance.MINIMIZE
        ... )
        >>> new_instance = instance.partial_evaluate({1: 1})
        >>> new_instance.objective
        Function(x2 + 1)

        Substituted value is stored in the decision variable:

        >>> x = new_instance.get_decision_variable_by_id(1)
        >>> x.substituted_value
        1.0

        It appears in the decision variables DataFrame:

        >>> new_instance.decision_variables_df.dropna(axis=1, how="all")  # doctest: +NORMALIZE_WHITESPACE
              kind  lower  upper subscripts substituted_value
        id
        1   Binary    0.0    1.0         []               1.0
        2   Binary    0.0    1.0         []              <NA>

        """
        # Create a copy of the instance and call partial_evaluate on it
        # Note: partial_evaluate modifies the instance in place and returns bytes
        temp_instance = copy.deepcopy(self.raw)
        temp_instance.partial_evaluate(State(state).to_bytes(), atol=atol)
        return Instance(temp_instance)

    def used_decision_variable_ids(self) -> set[int]:
        """
        Get the set of decision variable IDs used in the objective and remaining constraints.

        Examples
        =========

        >>> x = [DecisionVariable.binary(i) for i in range(3)]
        >>> instance = Instance.from_components(
        ...     decision_variables=x,
        ...     objective=sum(x),
        ...     constraints=[],
        ...     sense=Instance.MAXIMIZE,
        ... )
        >>> instance.used_decision_variable_ids()
        {0, 1, 2}

        >>> instance = Instance.from_components(
        ...     decision_variables=x,
        ...     objective=x[0],
        ...     constraints=[(x[1] == 1).set_id(0)],
        ...     sense=Instance.MAXIMIZE,
        ... )
        >>> instance.used_decision_variable_ids()
        {0, 1}

        >>> instance.relax_constraint(0, "testing")
        >>> instance.used_decision_variable_ids()
        {0}

        """
        return self.raw.required_ids()

    @property
    def used_decision_variables(self) -> list[DecisionVariable]:
        """
        Get a list of only the decision variables used in the objective and remaining constraints.

        Returns a list of :class:`DecisionVariable` instancess sorted by their IDs.

        Decision variables defined in the instance but not actually present in the objective function and constraints are excluded from the list.
        """
        return [DecisionVariable(dv) for dv in self.raw.used_decision_variables]

    def stats(self) -> dict:
        """
        Get statistics about the instance.

        Returns a dictionary containing counts of decision variables and constraints
        categorized by kind, usage, and status.

        Returns
        -------
        dict
            A dictionary with the following structure::

                {
                    "decision_variables": {
                        "total": int,
                        "by_kind": {
                            "binary": int,
                            "integer": int,
                            "continuous": int,
                            "semi_integer": int,
                            "semi_continuous": int
                        },
                        "by_usage": {
                            "used_in_objective": int,
                            "used_in_constraints": int,
                            "used": int,
                            "fixed": int,
                            "dependent": int,
                            "irrelevant": int
                        }
                    },
                    "constraints": {
                        "total": int,
                        "active": int,
                        "removed": int
                    }
                }

        Examples
        --------
        >>> instance = Instance.empty()
        >>> stats = instance.stats()
        >>> stats["decision_variables"]["total"]
        0
        >>> stats["constraints"]["total"]
        0
        """
        return self.raw.stats()

    def to_qubo(
        self,
        *,
        uniform_penalty_weight: Optional[float] = None,
        penalty_weights: dict[int, float] = {},
        inequality_integer_slack_max_range: int = 31,
    ) -> tuple[dict[tuple[int, int], float], float]:
        r"""
        Convert the instance to a QUBO format

        This is a **Driver API** for QUBO conversion calling single-purpose methods in order:

        1. Convert the instance to a minimization problem by :py:meth:`as_minimization_problem`.
        2. Check continuous variables and raise error if exists.
        3. Convert inequality constraints

            * Try :py:meth:`convert_inequality_to_equality_with_integer_slack` first with given ``inequality_integer_slack_max_range``.
            * If failed, :py:meth:`add_integer_slack_to_inequality`

        4. Convert to QUBO with (uniform) penalty method

            * If ``penalty_weights`` is given (in ``dict[constraint_id, weight]`` form), use :py:meth:`penalty_method` with the given weights.
            * If ``uniform_penalty_weight`` is given, use :py:meth:`uniform_penalty_method` with the given weight.
            * If both are None, defaults to ``uniform_penalty_weight = 1.0``.

        5. Log-encode integer variables by :py:meth:`log_encode`.
        6. Finally convert to QUBO format by :py:meth:`as_qubo_format`.

        Please see the document of each method for details.
        If you want to customize the conversion, use the methods above manually.

        .. important::

            The above process is not stable, and subject to change for better QUBO generation in the future versions.
            If you wish to keep the compatibility, please use the methods above manually.

        Examples
        ========

        Let's consider a maximization problem with two integer variables :math:`x_0, x_1 \in [0, 2]` subject to an inequality:

        .. math::

            \begin{align*}
                \max_{x_0, x_1} & \space x_0 + x_1 & \\
                \text{ s.t. } & \space x_0 + 2x_1 \leq 3
            \end{align*}

        >>> from ommx.v1 import Instance, DecisionVariable
        >>> x = [DecisionVariable.integer(i, lower=0, upper=2, name = "x", subscripts=[i]) for i in range(2)]
        >>> instance = Instance.from_components(
        ...     decision_variables=x,
        ...     objective=sum(x),
        ...     constraints=[(x[0] + 2*x[1] <= 3).set_id(0)],
        ...     sense=Instance.MAXIMIZE,
        ... )

        Convert into QUBO format

        >>> qubo, offset = instance.to_qubo()
        >>> qubo
        {(3, 3): -6.0, (3, 4): 2.0, (3, 5): 4.0, (3, 6): 4.0, (3, 7): 2.0, (3, 8): 4.0, (4, 4): -6.0, (4, 5): 4.0, (4, 6): 4.0, (4, 7): 2.0, (4, 8): 4.0, (5, 5): -9.0, (5, 6): 8.0, (5, 7): 4.0, (5, 8): 8.0, (6, 6): -9.0, (6, 7): 4.0, (6, 8): 8.0, (7, 7): -5.0, (7, 8): 4.0, (8, 8): -8.0}
        >>> offset
        9.0

        The ``instance`` object stores how converted:

        * For the maximization problem, the sense is converted to minimization for generating QUBO, and then converted back to maximization.

        >>> instance.sense == Instance.MAXIMIZE
        True

        * Two types of decision variables are added

            * ``ommx.slack`` integer slack variable :math:`x_2` by :py:meth:`convert_inequality_to_equality_with_integer_slack`

            * ``ommx.log_encode`` binary variables :math:`x_3, \ldots, x_8` introduced by :py:meth:`log_encode`.

        >>> instance.decision_variables_df.dropna(axis=1, how="all")  # doctest: +NORMALIZE_WHITESPACE
               kind  lower  upper             name subscripts
        id
        0   Integer    0.0    2.0                x        [0]
        1   Integer    0.0    2.0                x        [1]
        2   Integer    0.0    3.0       ommx.slack        [0]
        3    Binary    0.0    1.0  ommx.log_encode     [0, 0]
        4    Binary    0.0    1.0  ommx.log_encode     [0, 1]
        5    Binary    0.0    1.0  ommx.log_encode     [1, 0]
        6    Binary    0.0    1.0  ommx.log_encode     [1, 1]
        7    Binary    0.0    1.0  ommx.log_encode     [2, 0]
        8    Binary    0.0    1.0  ommx.log_encode     [2, 1]

        * The yielded :attr:`objective` and :attr:`removed_constraints` only has these binary variables.

        >>> instance.objective
        Function(-x3*x3 - 2*x3*x4 - 4*x3*x5 - 4*x3*x6 - 2*x3*x7 - 4*x3*x8 - x4*x4 - 4*x4*x5 - 4*x4*x6 - 2*x4*x7 - 4*x4*x8 - 4*x5*x5 - 8*x5*x6 - 4*x5*x7 - 8*x5*x8 - 4*x6*x6 - 4*x6*x7 - 8*x6*x8 - x7*x7 - 4*x7*x8 - 4*x8*x8 + 7*x3 + 7*x4 + 13*x5 + 13*x6 + 6*x7 + 12*x8 - 9)
        >>> instance.get_removed_constraint_by_id(0)
        RemovedConstraint(x3 + x4 + 2*x5 + 2*x6 + x7 + 2*x8 - 3 == 0, reason=uniform_penalty_method)

        Solvers will return solutions which only contain log-encoded binary variables like:

        >>> state = {
        ...     3: 1, 4: 1,  # x0 = 0 + (2-1)*1 = 2
        ...     5: 0, 6: 0,  # x1 = 0 + (2-1)*0 = 0
        ...     7: 1, 8: 0   # x3 = 1 + 2*0 = 1
        ... }

        This can be evaluated by :py:meth:`evaluate` method.

        >>> solution = instance.evaluate(state)

        The log-encoded integer variables are automatically evaluated from the binary variables.

        >>> solution.decision_variables_df.dropna(axis=1, how="all")  # doctest: +NORMALIZE_WHITESPACE
               kind  lower  upper             name subscripts  value
        id                                                          
        0   Integer    0.0    2.0                x        [0]    2.0
        1   Integer    0.0    2.0                x        [1]    0.0
        2   Integer    0.0    3.0       ommx.slack        [0]    1.0
        3    Binary    0.0    1.0  ommx.log_encode     [0, 0]    1.0
        4    Binary    0.0    1.0  ommx.log_encode     [0, 1]    1.0
        5    Binary    0.0    1.0  ommx.log_encode     [1, 0]    0.0
        6    Binary    0.0    1.0  ommx.log_encode     [1, 1]    0.0
        7    Binary    0.0    1.0  ommx.log_encode     [2, 0]    1.0
        8    Binary    0.0    1.0  ommx.log_encode     [2, 1]    0.0

        >>> solution.objective
        2.0

        >>> solution.constraints_df.dropna(axis=1, how="all")  # doctest: +NORMALIZE_WHITESPACE
           equality  value            used_ids subscripts          removed_reason
        id                                                                       
        0        =0    0.0  {3, 4, 5, 6, 7, 8}         []  uniform_penalty_method

        """
        is_converted_to_minimize = self.as_minimization_problem()

        continuous_variables = [
            var.id
            for var in self.decision_variables
            if var.kind == DecisionVariable.CONTINUOUS
        ]
        if len(continuous_variables) > 0:
            raise ValueError(
                f"Continuous variables are not supported in QUBO conversion: IDs={continuous_variables}"
            )

        # Prepare inequality constraints
        ineq_ids = [
            c.id
            for c in self.constraints
            if c.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO
        ]
        for ineq_id in ineq_ids:
            try:
                self.convert_inequality_to_equality_with_integer_slack(
                    ineq_id, inequality_integer_slack_max_range
                )
            except RuntimeError:
                self.add_integer_slack_to_inequality(
                    ineq_id, inequality_integer_slack_max_range
                )

        # Penalty method
        if self.constraints:
            if uniform_penalty_weight is not None and penalty_weights:
                raise ValueError(
                    "Both uniform_penalty_weight and penalty_weights are specified. Please choose one."
                )
            if penalty_weights:
                pi = self.penalty_method()
                weights = {
                    p.id: penalty_weights[p.subscripts[0]] for p in pi.parameters
                }
                unconstrained = pi.with_parameters(weights)
            else:
                if uniform_penalty_weight is None:
                    # If both are None, defaults to uniform_penalty_weight = 1.0
                    uniform_penalty_weight = 1.0
                pi = self.uniform_penalty_method()
                weight = pi.parameters[0]
                unconstrained = pi.with_parameters({weight.id: uniform_penalty_weight})
            self.raw = unconstrained.raw

        self.log_encode()
        qubo = self.as_qubo_format()

        if is_converted_to_minimize:
            # Convert back to maximization
            self.as_maximization_problem()

        return qubo

    def to_hubo(
        self,
        *,
        uniform_penalty_weight: Optional[float] = None,
        penalty_weights: dict[int, float] = {},
        inequality_integer_slack_max_range: int = 31,
    ) -> tuple[dict[tuple[int, ...], float], float]:
        r"""Convert the instance to a HUBO format

        This is a **Driver API** for HUBO conversion calling single-purpose methods in order:

        1. Convert the instance to a minimization problem by :py:meth:`as_minimization_problem`.
        2. Check continuous variables and raise error if exists.
        3. Convert inequality constraints

            * Try :py:meth:`convert_inequality_to_equality_with_integer_slack` first with given ``inequality_integer_slack_max_range``.
            * If failed, :py:meth:`add_integer_slack_to_inequality`

        4. Convert to HUBO with (uniform) penalty method

            * If ``penalty_weights`` is given (in ``dict[constraint_id, weight]`` form), use :py:meth:`penalty_method` with the given weights.
            * If ``uniform_penalty_weight`` is given, use :py:meth:`uniform_penalty_method` with the given weight.
            * If both are None, defaults to ``uniform_penalty_weight = 1.0``.

        5. Log-encode integer variables by :py:meth:`log_encode`.
        6. Finally convert to HUBO format by :py:meth:`as_hubo_format`.

        Please see the documentation for `to_qubo` for more information, or the
        documentation for each individual method for additional details. The
        difference between this and `to_qubo` is that this method isn't
        restricted to quadratic or linear problems. If you want to customize the
        conversion, use the individual methods above manually.

        .. important::

            The above process is not stable, and subject to change for better HUBO generation in the future versions.
            If you wish to keep the compatibility, please use the methods above manually.

        """
        is_converted_to_minimize = self.as_minimization_problem()

        continuous_variables = [
            var.id
            for var in self.decision_variables
            if var.kind == DecisionVariable.CONTINUOUS
        ]
        if len(continuous_variables) > 0:
            raise ValueError(
                f"Continuous variables are not supported in HUBO conversion: IDs={continuous_variables}"
            )

        # Prepare inequality constraints
        ineq_ids = [
            c.id
            for c in self.constraints
            if c.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO
        ]
        for ineq_id in ineq_ids:
            try:
                self.convert_inequality_to_equality_with_integer_slack(
                    ineq_id, inequality_integer_slack_max_range
                )
            except RuntimeError:
                self.add_integer_slack_to_inequality(
                    ineq_id, inequality_integer_slack_max_range
                )

        # Penalty method
        if self.constraints:
            if uniform_penalty_weight is not None and penalty_weights:
                raise ValueError(
                    "Both uniform_penalty_weight and penalty_weights are specified. Please choose one."
                )
            if penalty_weights:
                pi = self.penalty_method()
                weights = {
                    p.id: penalty_weights[p.subscripts[0]] for p in pi.parameters
                }
                unconstrained = pi.with_parameters(weights)
            else:
                if uniform_penalty_weight is None:
                    # If both are None, defaults to uniform_penalty_weight = 1.0
                    uniform_penalty_weight = 1.0
                pi = self.uniform_penalty_method()
                weight = pi.parameters[0]
                unconstrained = pi.with_parameters({weight.id: uniform_penalty_weight})
            self.raw = unconstrained.raw

        self.log_encode()
        qubo = self.as_hubo_format()

        if is_converted_to_minimize:
            # Convert back to maximization
            self.as_maximization_problem()

        return qubo

    def as_minimization_problem(self) -> bool:
        """
        Convert the instance to a minimization problem.

        If the instance is already a minimization problem, this does nothing.

        :return: ``True`` if the instance is converted, ``False`` if already a minimization problem.

        Examples
        =========

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
        True
        >>> instance.sense == Instance.MINIMIZE
        True
        >>> instance.objective
        Function(-x0 - x1 - x2)

        If the instance is already a minimization problem, this does nothing

        >>> instance.as_minimization_problem()
        False
        >>> instance.sense == Instance.MINIMIZE
        True
        >>> instance.objective
        Function(-x0 - x1 - x2)

        """
        return self.raw.as_minimization_problem()

    def as_maximization_problem(self) -> bool:
        """
        Convert the instance to a maximization problem.

        If the instance is already a maximization problem, this does nothing.

        :return: ``True`` if the instance is converted, ``False`` if already a maximization problem.

        Examples
        =========

        >>> from ommx.v1 import Instance, DecisionVariable
        >>> x = [DecisionVariable.binary(i) for i in range(3)]
        >>> instance = Instance.from_components(
        ...     decision_variables=x,
        ...     objective=sum(x),
        ...     constraints=[sum(x) == 1],
        ...     sense=Instance.MINIMIZE,
        ... )
        >>> instance.sense == Instance.MINIMIZE
        True
        >>> instance.objective
        Function(x0 + x1 + x2)

        Convert to a maximization problem

        >>> instance.as_maximization_problem()
        True
        >>> instance.sense == Instance.MAXIMIZE
        True
        >>> instance.objective
        Function(-x0 - x1 - x2)

        If the instance is already a maximization problem, this does nothing

        >>> instance.as_maximization_problem()
        False
        >>> instance.sense == Instance.MAXIMIZE
        True
        >>> instance.objective
        Function(-x0 - x1 - x2)

        """
        return self.raw.as_maximization_problem()

    def as_qubo_format(self) -> tuple[dict[tuple[int, int], float], float]:
        """
        Convert unconstrained quadratic instance to PyQUBO-style format.

        .. note::
            This is a single-purpose method to only convert the format, not to execute any conversion of the instance.
            Use :py:meth:`to_qubo` driver for the full QUBO conversion.

        """
        return self.raw.as_qubo_format()

    def as_hubo_format(self) -> tuple[dict[tuple[int, ...], float], float]:
        """
        Convert unconstrained quadratic instance to a dictionary-based HUBO format.

        .. note::
            This is a single-purpose method to only convert the format, not to execute any conversion of the instance.
            Use :py:meth:`to_hubo` driver for the full HUBO conversion.

        """
        return self.raw.as_hubo_format()

    def penalty_method(self) -> ParametricInstance:
        r"""
        Convert to a parametric unconstrained instance by penalty method.

        Roughly, this converts a constrained problem

        .. math::

            \begin{align*}
                \min_x & \space f(x) & \\
                \text{ s.t. } & \space g_i(x) = 0 & (\forall i) \\
                & \space h_j(x) \leq 0 & (\forall j)
            \end{align*}

        to an unconstrained problem with parameters

        .. math::

            \min_x f(x) + \sum_i \lambda_i g_i(x)^2 + \sum_j \rho_j h_j(x)^2

        where :math:`\lambda_i` and :math:`\rho_j` are the penalty weight parameters for each constraint.
        If you want to use single weight parameter, use :py:meth:`uniform_penalty_method` instead.

        The removed constrains are stored in :py:attr:`~ParametricInstance.removed_constraints`.

        .. note::

            Note that this method converts inequality constraints :math:`h(x) \leq 0` to :math:`|h(x)|^2` not to :math:`\max(0, h(x))^2`.
            This means the penalty is enforced even for :math:`h(x) < 0` cases, and :math:`h(x) = 0` is unfairly favored.

            This feature is intended to use with :py:meth:`add_integer_slack_to_inequality`.

        Examples
        =========

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

        The constraint is put in :attr:`removed_constraints`

        >>> pi.constraints
        []
        >>> len(pi.removed_constraints)
        2
        >>> pi.removed_constraints[0]
        RemovedConstraint(x0 + x1 - 1 == 0, reason=penalty_method, parameter_id=3)
        >>> pi.removed_constraints[1]
        RemovedConstraint(x1 + x2 - 1 == 0, reason=penalty_method, parameter_id=4)

        There are two parameters corresponding to the two constraints

        >>> len(pi.parameters)
        2
        >>> p1 = pi.parameters[0]
        >>> p1.id, p1.name
        (3, 'penalty_weight')
        >>> p2 = pi.parameters[1]
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
        return ParametricInstance.from_bytes(self.raw.penalty_method().to_bytes())

    def uniform_penalty_method(self) -> ParametricInstance:
        r"""
        Convert to a parametric unconstrained instance by penalty method with uniform weight.

        Roughly, this converts a constrained problem

        .. math::

            \begin{align*}
                \min_x & \space f(x) & \\
                \text{ s.t. } & \space g_i(x) = 0 & (\forall i) \\
                & \space h_j(x) \leq 0 & (\forall j)
            \end{align*}

        to an unconstrained problem with a parameter

        .. math::

            \min_x f(x) + \lambda \left( \sum_i g_i(x)^2 + \sum_j h_j(x)^2 \right)

        where :math:`\lambda` is the uniform penalty weight parameter for all constraints.

        The removed constrains are stored in :py:attr:`~ParametricInstance.removed_constraints`.

        .. note::

            Note that this method converts inequality constraints :math:`h(x) \leq 0` to :math:`|h(x)|^2` not to :math:`\max(0, h(x))^2`.
            This means the penalty is enforced even for :math:`h(x) < 0` cases, and :math:`h(x) = 0` is unfairly favored.

            This feature is intended to use with :py:meth:`add_integer_slack_to_inequality`.

        Examples
        =========

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

        The constraint is put in :attr:`removed_constraints`

        >>> pi.constraints
        []
        >>> len(pi.removed_constraints)
        1
        >>> pi.removed_constraints[0]
        RemovedConstraint(x0 + x1 + x2 - 3 == 0, reason=uniform_penalty_method)

        There is only one parameter in the instance

        >>> len(pi.parameters)
        1
        >>> p = pi.parameters[0]
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
        return ParametricInstance.from_bytes(
            self.raw.uniform_penalty_method().to_bytes()
        )

    def as_parametric_instance(self) -> ParametricInstance:
        """
        Convert the instance to a :class:`ParametricInstance`.
        """
        return ParametricInstance.from_bytes(
            self.raw.as_parametric_instance().to_bytes()
        )

    def evaluate_samples(
        self, samples: ToSamples, *, atol: float | None = None
    ) -> SampleSet:
        """
        Evaluate the instance with multiple states.
        """
        samples_ = Samples(samples)
        return SampleSet(self.raw.evaluate_samples(samples_, atol=atol))

    def random_state(self, rng: _ommx_rust.Rng) -> State:
        """
        Generate a random state for this instance using the provided random number generator.

        This method generates random values only for variables that are actually used in the
        objective function or constraints, as determined by decision variable analysis.
        Generated values respect the bounds of each variable type.

        Parameters
        ----------
        rng : _ommx_rust.Rng
            Random number generator to use for generating the state.

        Returns
        -------
        State
            A randomly generated state that satisfies the variable bounds of this instance.
            Only contains values for variables that are used in the problem.

        Examples
        =========

        ### Generate random state only for used variables

        >>> from ommx.v1 import Instance, DecisionVariable, Rng
        >>> x = [DecisionVariable.binary(i) for i in range(5)]
        >>> instance = Instance.from_components(
        ...     decision_variables=x,
        ...     objective=x[0] + x[1],  # Only x[0] and x[1] are used
        ...     constraints=[],
        ...     sense=Instance.MAXIMIZE,
        ... )

        >>> rng = Rng()
        >>> state = instance.random_state(rng)

        Only used variables have values

        >>> set(state.entries.keys())
        {0, 1}

        Values respect binary bounds

        >>> all(state.entries[i] in [0.0, 1.0] for i in state.entries)
        True

        ### Generate random state respecting variable bounds

        >>> x_bin = DecisionVariable.binary(0)
        >>> x_int = DecisionVariable.integer(1, lower=3, upper=7)
        >>> x_cont = DecisionVariable.continuous(2, lower=-2.5, upper=5.0)

        >>> instance = Instance.from_components(
        ...     decision_variables=[x_bin, x_int, x_cont],
        ...     objective=x_bin + x_int + x_cont,
        ...     constraints=[],
        ...     sense=Instance.MINIMIZE,
        ... )

        >>> rng = Rng()
        >>> state = instance.random_state(rng)

        Values respect their respective bounds

        >>> state.entries[0] in [0.0, 1.0]  # Binary
        True
        >>> 3.0 <= state.entries[1] <= 7.0  # Integer
        True
        >>> -2.5 <= state.entries[2] <= 5.0  # Continuous
        True

        """
        return self.raw.random_state(rng)

    def random_samples(
        self,
        rng: _ommx_rust.Rng,
        *,
        num_different_samples: int = 5,
        num_samples: int = 10,
        max_sample_id: int | None = None,
    ) -> Samples:
        """
        Generate random samples for this instance.

        The generated samples will contain ``num_samples`` sample entries divided into
        ``num_different_samples`` groups, where each group shares the same state but has
        different sample IDs.

        :param rng: Random number generator
        :param num_different_samples: Number of different states to generate
        :param num_samples: Total number of samples to generate
        :param max_sample_id: Maximum sample ID (default: ``num_samples``)
        :return: Samples object

        Examples
        ========

        Generate samples for a simple instance:

        >>> x = [DecisionVariable.binary(i) for i in range(3)]
        >>> instance = Instance.from_components(
        ...     decision_variables=x,
        ...     objective=sum(x),
        ...     constraints=[(sum(x) <= 2).set_id(0)],
        ...     sense=Instance.MAXIMIZE,
        ... )

        >>> rng = Rng()
        >>> samples = instance.random_samples(rng, num_different_samples=2, num_samples=5)
        >>> samples.num_samples()
        5

        Each generated state respects variable bounds:

        >>> for sample_id in samples.sample_ids():
        ...     state = samples.get_state(sample_id)
        ...     for var_id, value in state.entries.items():
        ...         assert value in [0.0, 1.0], f"Binary variable {var_id} has invalid value {value}"

        """
        return self.raw.random_samples(
            rng,
            num_different_samples=num_different_samples,
            num_samples=num_samples,
            max_sample_id=max_sample_id,
        )

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
            >>> instance.constraints
            [Constraint(x0 + x1 + x2 - 3 == 0)]

            >>> instance.relax_constraint(1, "manual relaxation")
            >>> instance.constraints
            []
            >>> instance.removed_constraints
            [RemovedConstraint(x0 + x1 + x2 - 3 == 0, reason=manual relaxation)]

            >>> instance.restore_constraint(1)
            >>> instance.constraints
            [Constraint(x0 + x1 + x2 - 3 == 0)]
            >>> instance.removed_constraints
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
        self.raw.relax_constraint(constraint_id, reason, parameters)

    def restore_constraint(self, constraint_id: int):
        """
        Restore a removed constraint to the instance.

        :param constraint_id: The ID of the constraint to restore.

        Note that this drops the removed reason and associated parameters. See :py:meth:`relax_constraint` for details.
        """
        self.raw.restore_constraint(constraint_id)

    def log_encode(self, decision_variable_ids: set[int] = set({})):
        r"""
        Log-encode the integer decision variables

        Log encoding of an integer variable :math:`x \in [l, u]` is to represent by :math:`m` bits :math:`b_i \in \{0, 1\}` by

        .. math::
            x = \sum_{i=0}^{m-2} 2^i b_i + (u - l - 2^{m-1} + 1) b_{m-1} + l

        where :math:`m = \lceil \log_2(u - l + 1) \rceil`.

        :param decision_variable_ids: The IDs of the integer decision variables to log-encode. If not specified, all integer variables are log-encoded.

        Examples
        =========

        Let's consider a simple integer programming problem with three integer variables :math:`x_0`, :math:`x_1`, and :math:`x_2`.

        >>> from ommx.v1 import Instance, DecisionVariable
        >>> x = [
        ...     DecisionVariable.integer(i, lower=0, upper=3, name="x", subscripts=[i])
        ...     for i in range(3)
        ... ]
        >>> instance = Instance.from_components(
        ...     decision_variables=x,
        ...     objective=sum(x),
        ...     constraints=[],
        ...     sense=Instance.MAXIMIZE,
        ... )
        >>> instance.objective
        Function(x0 + x1 + x2)

        To log-encode the integer variables :math:`x_0` and :math:`x_2` (except :math:`x_1`), call :meth:`log_encode`:

        >>> instance.log_encode({0, 2})

        Integer variable in range :math:`[0, 3]` can be represented by two binary variables:

        .. math::
            x_0 = b_{0, 0} + 2 b_{0, 1}, x_2 = b_{2, 0} + 2 b_{2, 1}

        And these are substituted into the objective and constraint functions.

        >>> instance.objective
        Function(x1 + x3 + 2*x4 + x5 + 2*x6)

        Added binary variables are also appeared in :attr:`decision_variables`

        >>> instance.decision_variables_df.dropna(axis=1, how="all")  # doctest: +NORMALIZE_WHITESPACE
               kind  lower  upper             name subscripts
        id
        0   Integer    0.0    3.0                x        [0]
        1   Integer    0.0    3.0                x        [1]
        2   Integer    0.0    3.0                x        [2]
        3    Binary    0.0    1.0  ommx.log_encode     [0, 0]
        4    Binary    0.0    1.0  ommx.log_encode     [0, 1]
        5    Binary    0.0    1.0  ommx.log_encode     [2, 0]
        6    Binary    0.0    1.0  ommx.log_encode     [2, 1]

        The `subscripts` of the new binary variables must be two elements in form of :math:`[i, j]` where

        - :math:`i` is the decision variable ID of the original integer variable
        - :math:`j` is the index of the binary variable

        After log-encoded, the problem does not contains original integer variables,
        and solver will returns only encoded variables.

        >>> solution = instance.evaluate({
        ...   1: 2,          # x1 = 2
        ...   3: 0, 4: 1,    # x0 = x3 + 2*x4 = 0 + 2*1 = 2
        ...   5: 0, 6: 0     # x2 = x5 + 2*x6 = 0 + 2*0 = 0
        ... })               # x0 and x2 are not contained in the solver result

        x0 and x2 are automatically evaluated:

        >>> solution.extract_decision_variables("x")
        {(0,): 2.0, (1,): 2.0, (2,): 0.0}

        The name of the binary variables are automatically generated as `ommx.log_encode`.

        >>> solution.extract_decision_variables("ommx.log_encode")
        {(0, 0): 0.0, (0, 1): 1.0, (2, 0): 0.0, (2, 1): 0.0}

        """
        if not decision_variable_ids:
            decision_variable_ids = {
                var.id
                for var in self.decision_variables
                if var.kind == DecisionVariable.INTEGER
            }
            if not decision_variable_ids:
                # No integer variables
                return
        self.raw.log_encode(decision_variable_ids)

    def reduce_binary_power(self) -> bool:
        """
        Reduce binary powers in the instance.

        This method replaces binary powers in the instance with their equivalent linear expressions.
        For binary variables, x^n = x for any n >= 1, so we can reduce higher powers to linear terms.

        :return: ``True`` if any reduction was performed, ``False`` otherwise.

        Examples
        =========

        Consider an instance with binary variables and quadratic terms:

        >>> from ommx.v1 import Instance, DecisionVariable
        >>> x = [DecisionVariable.binary(i) for i in range(2)]
        >>> instance = Instance.from_components(
        ...     decision_variables=x,
        ...     objective=x[0] * x[0] + x[0] * x[1],  # x0^2 + x0*x1
        ...     constraints=[],
        ...     sense=Instance.MINIMIZE,
        ... )
        >>> instance.objective
        Function(x0*x0 + x0*x1)

        After reducing binary powers, x0^2 becomes x0:

        >>> changed = instance.reduce_binary_power()
        >>> changed
        True
        >>> instance.objective
        Function(x0*x1 + x0)

        Running it again should not change anything:

        >>> changed = instance.reduce_binary_power()
        >>> changed
        False

        """
        return self.raw.reduce_binary_power()

    def convert_inequality_to_equality_with_integer_slack(
        self, constraint_id: int, max_integer_range: int
    ):
        r"""
        Convert an inequality constraint :math:`f(x) \leq 0` to an equality constraint :math:`f(x) + s/a = 0` with an integer slack variable `s`.

        * Since :math:`a` is determined as the minimal multiplier to make the every coefficient of :math:`af(x)` integer,
          :math:`a` itself and the range of :math:`s` becomes impractically large. `max_integer_range` limits the maximal range of :math:`s`,
          and returns error if the range exceeds it. See also :py:meth:`~Function.content_factor`.

        * Since this method evaluates the bound of :math:`f(x)`, we may find that:

          * The bound :math:`[l, u]` is strictly positive, i.e. :math:`l \gt 0`.
            This means the instance is infeasible because this constraint never be satisfied.
            In this case, an error is raised.

          * The bound :math:`[l, u]` is always negative, i.e. :math:`u \leq 0`.
            This means this constraint is trivially satisfied.
            In this case, the constraint is moved to :py:attr:`~Instance.removed_constraints`,
            and this method returns without introducing slack variable or raising an error.

        Examples
        =========

        Let's consider a simple inequality constraint :math:`x_0 + 2x_1 \leq 5`.

        >>> from ommx.v1 import Instance, DecisionVariable
        >>> x = [
        ...     DecisionVariable.integer(i, lower=0, upper=3, name="x", subscripts=[i])
        ...     for i in range(3)
        ... ]
        >>> instance = Instance.from_components(
        ...     decision_variables=x,
        ...     objective=sum(x),
        ...     constraints=[
        ...         (x[0] + 2*x[1] <= 5).set_id(0)   # Set ID manually to use after
        ...     ],
        ...     sense=Instance.MAXIMIZE,
        ... )
        >>> instance.constraints[0]
        Constraint(x0 + 2*x1 - 5 <= 0)

        Introduce an integer slack variable

        >>> instance.convert_inequality_to_equality_with_integer_slack(
        ...     constraint_id=0,
        ...     max_integer_range=32
        ... )
        >>> instance.constraints[0]
        Constraint(x0 + 2*x1 + x3 - 5 == 0)

        The slack variable is added to the decision variables with name `ommx.slack` and the constraint ID is stored in `subscripts`.

        >>> instance.decision_variables_df.dropna(axis=1, how="all")  # doctest: +NORMALIZE_WHITESPACE
               kind  lower  upper        name subscripts
        id
        0   Integer    0.0    3.0           x        [0]
        1   Integer    0.0    3.0           x        [1]
        2   Integer    0.0    3.0           x        [2]
        3   Integer    0.0    5.0  ommx.slack        [0]

        """
        self.raw.convert_inequality_to_equality_with_integer_slack(
            constraint_id, max_integer_range
        )

    def add_integer_slack_to_inequality(
        self, constraint_id: int, slack_upper_bound: int
    ) -> float | None:
        r"""
        Convert inequality :math:`f(x) \leq 0` to **inequality** :math:`f(x) + b s \leq 0` with an integer slack variable `s`.

        * This should be used when :meth:`convert_inequality_to_equality_with_integer_slack` is not applicable

        * The bound of :math:`s` will be `[0, slack_upper_bound]`, and the coefficients :math:`b` are determined from the lower bound of :math:`f(x)`.

        * Since the slack variable is integer, the yielded inequality has residual error :math:`\min_s f(x) + b s` at most :math:`b`.
          And thus :math:`b` is returned to use scaling the penalty weight or other things.

          * Larger `slack_upper_bound` (i.e. fined-grained slack) yields smaller `b`, and thus smaller the residual error.
            But it needs more bits for the slack variable, and thus the problem size becomes larger.

        * Since this method evaluates the bound of :math:`f(x)`, we may find that:

          * The bound :math:`[l, u]` is strictly positive, i.e. :math:`l \gt 0`.
            This means the instance is infeasible because this constraint never be satisfied.
            In this case, an error is raised.

          * The bound :math:`[l, u]` is always negative, i.e. :math:`u \leq 0`.
            This means this constraint is trivially satisfied.
            In this case, the constraint is moved to :py:attr:`~Instance.removed_constraints`,
            and this method returns without introducing slack variable or raising an error.

        :return: The coefficient :math:`b` of the slack variable. If the constraint is trivially satisfied, this returns `None`.

        Examples
        =========

        Let's consider a simple inequality constraint :math:`x_0 + 2x_1 \leq 4`.

        >>> from ommx.v1 import Instance, DecisionVariable
        >>> x = [
        ...     DecisionVariable.integer(i, lower=0, upper=3, name="x", subscripts=[i])
        ...     for i in range(3)
        ... ]
        >>> instance = Instance.from_components(
        ...     decision_variables=x,
        ...     objective=sum(x),
        ...     constraints=[
        ...         (x[0] + 2*x[1] <= 4).set_id(0)   # Set ID manually to use after
        ...     ],
        ...     sense=Instance.MAXIMIZE,
        ... )
        >>> instance.constraints[0]
        Constraint(x0 + 2*x1 - 4 <= 0)

        Introduce an integer slack variable :math:`s \in [0, 2]`

        >>> b = instance.add_integer_slack_to_inequality(
        ...     constraint_id=0,
        ...     slack_upper_bound=2
        ... )
        >>> b, instance.constraints[0]
        (2.0, Constraint(x0 + 2*x1 + 2*x3 - 4 <= 0))

        The slack variable is added to the decision variables with name `ommx.slack` and the constraint ID is stored in `subscripts`.

        >>> instance.decision_variables_df.dropna(axis=1, how="all")  # doctest: +NORMALIZE_WHITESPACE
               kind  lower  upper        name subscripts
        id
        0   Integer    0.0    3.0           x        [0]
        1   Integer    0.0    3.0           x        [1]
        2   Integer    0.0    3.0           x        [2]
        3   Integer    0.0    2.0  ommx.slack        [0]

        In this case, the slack variable only take :math:`s = \{ 0, 1, 2 \}`,
        and thus the residual error is not disappear for :math:`x_0 = x_1 = 1` case :math:`f(x) + b \cdot x = 1 + 2 \cdot 1 + 2 \cdot s - 4 = 2s - 1`.

        """
        return self.raw.add_integer_slack_to_inequality(
            constraint_id, slack_upper_bound
        )

    def decision_variable_analysis(self) -> "DecisionVariableAnalysis":
        """
        Analyze decision variables in the optimization problem instance.

        Returns a comprehensive analysis of all decision variables including:
        - Kind-based partitioning (binary, integer, continuous, etc.)
        - Usage-based partitioning (used in objective, constraints, fixed, etc.)
        - Variable bounds information

        Returns
        -------
        DecisionVariableAnalysis
            Analysis object containing detailed information about decision variables

        Examples
        --------
        >>> x = [DecisionVariable.binary(i) for i in range(3)]
        >>> instance = Instance.from_components(
        ...     decision_variables=x,
        ...     objective=x[0] + x[1],
        ...     constraints=[(x[1] + x[2] == 1).set_id(0)],
        ...     sense=Instance.MAXIMIZE,
        ... )
        >>> analysis = instance.decision_variable_analysis()
        >>> analysis.used_decision_variable_ids()
        {0, 1, 2}
        >>> analysis.used_in_objective()
        {0, 1}
        >>> analysis.used_in_constraints()
        {0: {1, 2}}
        """
        return DecisionVariableAnalysis(self.raw.decision_variable_analysis())


@dataclass
class ParametricInstance(UserAnnotationBase):
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
            sense=_Instance.Sense.SENSE_MINIMIZE,
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
        | Function,
        constraints: Iterable[Constraint | _Constraint],
        sense: _Instance.Sense.ValueType | Sense,
        decision_variables: Iterable[DecisionVariable | _DecisionVariable],
        parameters: Iterable[Parameter | _Parameter],
        description: Optional[_Instance.Description] = None,
    ) -> ParametricInstance:
        if not isinstance(objective, Function):
            objective = Function(objective)
        raw_objective = _Function()
        raw_objective.ParseFromString(objective.to_bytes())

        if isinstance(sense, Sense):
            sense = _Instance.Sense.ValueType(sense.to_pb())

        return ParametricInstance(
            _ParametricInstance(
                description=description,
                decision_variables=[
                    v.to_protobuf() if isinstance(v, DecisionVariable) else v
                    for v in decision_variables
                ],
                objective=raw_objective,
                constraints=[
                    c.to_protobuf() if isinstance(c, Constraint) else c
                    for c in constraints
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

    @property
    def decision_variables(self) -> list[DecisionVariable]:
        """
        Get decision variables as a list of :class:`DecisionVariable` instances.
        """
        return [
            DecisionVariable.from_protobuf(raw) for raw in self.raw.decision_variables
        ]

    @property
    def constraints(self) -> list[Constraint]:
        """
        Get constraints as a list of :class:`Constraint
        """
        return [Constraint.from_protobuf(raw) for raw in self.raw.constraints]

    @property
    def removed_constraints(self) -> list[RemovedConstraint]:
        """
        Get removed constraints as a list of :class:`RemovedConstraint` instances.
        """
        return [
            RemovedConstraint.from_protobuf(raw) for raw in self.raw.removed_constraints
        ]

    @property
    def parameters(self) -> list[Parameter]:
        """
        Get parameters as a list of :class:`Parameter`.
        """
        return [Parameter(raw) for raw in self.raw.parameters]

    def get_parameter_by_id(self, parameter_id: int) -> Parameter:
        """
        Get a parameter by ID.
        """
        for p in self.raw.parameters:
            if p.id == parameter_id:
                return Parameter(p)
        raise ValueError(f"Parameter ID {parameter_id} is not found")

    def get_decision_variable_by_id(self, variable_id: int) -> DecisionVariable:
        """
        Get a decision variable by ID.
        """
        for v in self.decision_variables:
            if v.id == variable_id:
                return v
        raise ValueError(f"Decision variable ID {variable_id} is not found")

    def get_constraint_by_id(self, constraint_id: int) -> Constraint:
        """
        Get a constraint by ID.
        """
        for c in self.constraints:
            if c.id == constraint_id:
                return c
        raise ValueError(f"Constraint ID {constraint_id} is not found")

    def get_removed_constraint_by_id(
        self, removed_constraint_id: int
    ) -> RemovedConstraint:
        """
        Get a removed constraint by ID.
        """
        for rc in self.removed_constraints:
            if rc.id == removed_constraint_id:
                return rc
        raise ValueError(f"Removed constraint ID {removed_constraint_id} is not found")

    @property
    def decision_variables_df(self) -> DataFrame:
        df = DataFrame(v._as_pandas_entry() for v in self.decision_variables)
        if not df.empty:
            df = df.set_index("id")
        return df

    @property
    def constraints_df(self) -> DataFrame:
        df = DataFrame(c._as_pandas_entry() for c in self.constraints)
        if not df.empty:
            df = df.set_index("id")
        return df

    @property
    def removed_constraints_df(self) -> DataFrame:
        df = DataFrame(rc._as_pandas_entry() for rc in self.removed_constraints)
        if not df.empty:
            df = df.set_index("id")
        return df

    @property
    def parameters_df(self) -> DataFrame:
        df = DataFrame(p._as_pandas_entry() for p in self.parameters)
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
        return Instance(instance)


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
            function=self - other, equality=Constraint.LESS_THAN_OR_EQUAL_TO_ZERO
        )

    def __ge__(self, other) -> Constraint:
        return Constraint(
            function=other - self, equality=Constraint.LESS_THAN_OR_EQUAL_TO_ZERO
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
        return Constraint(function=self - other, equality=Constraint.EQUAL_TO_ZERO)

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

    raw: _ommx_rust.Solution
    """The raw _ommx_rust.Solution object."""

    OPTIMAL = Optimality.Optimal
    NOT_OPTIMAL = Optimality.NotOptimal
    LP_RELAXED = Relaxation.LpRelaxed

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
        raw = _ommx_rust.Solution.from_bytes(data)
        return Solution(raw)

    def to_bytes(self) -> bytes:
        return self.raw.to_bytes()

    @property
    def state(self) -> State:
        return self.raw.state

    @property
    def objective(self) -> float:
        return self.raw.objective

    @property
    def decision_variables_df(self) -> DataFrame:
        # Use the new decision_variables property that returns dict of EvaluatedDecisionVariable
        df = DataFrame(
            {
                "id": v.id,
                "kind": str(v.kind),
                "lower": v.lower_bound,
                "upper": v.upper_bound,
                "name": v.name if v.name else NA,
                "subscripts": v.subscripts,
                "description": v.description if v.description else NA,
                "substituted_value": NA,  # This field is not available in the new API
                "value": v.value,
            }
            for v in self.decision_variables
        )
        if not df.empty:
            df = df.set_index("id")
        return df

    @property
    def constraints_df(self) -> DataFrame:
        # Use the new evaluated_constraints property
        df = DataFrame(
            {
                "id": c.id,
                "equality": str(c.equality),
                "value": c.evaluated_value,
                "used_ids": set(c.used_decision_variable_ids),
                "name": c.name if c.name else NA,
                "subscripts": c.subscripts,
                "description": c.description if c.description else NA,
                "dual_variable": c.dual_variable if c.dual_variable is not None else NA,
                "removed_reason": c.removed_reason if c.removed_reason else NA,
            }
            for c in self.constraints
        )
        if not df.empty:
            df = df.set_index("id")
        return df

    @property
    def decision_variable_ids(self) -> set[int]:
        """
        Get the IDs of decision variables in this solution.
        """
        return self.raw.decision_variable_ids

    @property
    def constraint_ids(self) -> set[int]:
        """
        Get the IDs of constraints in this solution.
        """
        return self.raw.constraint_ids

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
        # Use the extract method from _ommx_rust.Solution
        return self.raw.extract_decision_variables(name)

    @property
    def decision_variable_names(self) -> set[str]:
        """
        Get all unique decision variable names in this solution.

        Returns a set of all unique variable names. Variables without names are not included.

        Examples
        =========

        .. doctest::

            >>> from ommx.v1 import Instance, DecisionVariable
            >>> x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
            >>> y = [DecisionVariable.binary(i+3, name="y", subscripts=[i]) for i in range(2)]
            >>> instance = Instance.from_components(
            ...     decision_variables=x + y,
            ...     objective=sum(x) + sum(y),
            ...     constraints=[],
            ...     sense=Instance.MAXIMIZE,
            ... )
            >>> solution = instance.evaluate({i: 1 for i in range(5)})
            >>> sorted(solution.decision_variable_names)
            ['x', 'y']

        """
        return self.raw.decision_variable_names

    def extract_all_decision_variables(
        self,
    ) -> dict[str, dict[tuple[int, ...], float]]:
        """
        Extract all decision variables grouped by name.

        Returns a mapping from variable name to a mapping from subscripts to values.
        This is useful for extracting all variables at once in a structured format.
        Variables without names are not included in the result.

        :raises ValueError: If a decision variable with parameters is found, or if the same name and subscript combination is found multiple times.

        Examples
        =========

        .. doctest::

            >>> from ommx.v1 import Instance, DecisionVariable
            >>> x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
            >>> y = [DecisionVariable.binary(i+3, name="y", subscripts=[i]) for i in range(2)]
            >>> instance = Instance.from_components(
            ...     decision_variables=x + y,
            ...     objective=sum(x) + sum(y),
            ...     constraints=[],
            ...     sense=Instance.MAXIMIZE,
            ... )
            >>> solution = instance.evaluate({i: 1 for i in range(5)})
            >>> all_vars = solution.extract_all_decision_variables()
            >>> all_vars["x"]
            {(0,): 1.0, (1,): 1.0, (2,): 1.0}
            >>> all_vars["y"]
            {(0,): 1.0, (1,): 1.0}

        """
        return self.raw.extract_all_decision_variables()

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
        # Use the extract method from _ommx_rust.Solution
        return self.raw.extract_constraints(name)

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
        return self.raw.feasible_relaxed

    @property
    def feasible_unrelaxed(self) -> bool:
        """
        Feasibility of the solution in terms of all constraints, including relaxed (removed) constraints.
        """
        return self.raw.feasible

    @property
    def optimality(self) -> _ommx_rust.Optimality:
        # Return the _ommx_rust.Optimality enum directly
        return self.raw.optimality

    @property
    def relaxation(self) -> _ommx_rust.Relaxation:
        # Return the _ommx_rust.Relaxation enum directly
        return self.raw.relaxation

    @property
    def sense(self) -> _ommx_rust.Sense:
        # Return the _ommx_rust.Sense enum directly
        return self.raw.sense

    @optimality.setter
    def optimality(self, value: _ommx_rust.Optimality) -> None:
        """Set the optimality status."""
        self.raw.optimality = value

    @relaxation.setter
    def relaxation(self, value: _ommx_rust.Relaxation) -> None:
        """Set the relaxation status."""
        self.raw.relaxation = value

    def set_dual_variable(self, constraint_id: int, value: float | None) -> None:
        """Set the dual variable value for a specific constraint."""
        self.raw.set_dual_variable(constraint_id, value)

    def get_dual_variable(self, constraint_id: int) -> float | None:
        """Get the dual variable value for a specific constraint."""
        return self.raw.get_constraint_by_id(constraint_id).dual_variable

    def get_constraint_value(self, constraint_id: int) -> float:
        """Get the evaluated value of a specific constraint."""
        return self.raw.get_constraint_by_id(constraint_id).evaluated_value

    @property
    def decision_variables(self) -> list[EvaluatedDecisionVariable]:
        """Get evaluated decision variables as a list sorted by ID."""
        return self.raw.decision_variables

    @property
    def constraints(self) -> list[EvaluatedConstraint]:
        """Get evaluated constraints as a list sorted by ID."""
        return self.raw.constraints

    def get_decision_variable_by_id(
        self, variable_id: int
    ) -> EvaluatedDecisionVariable:
        """Get a specific evaluated decision variable by ID."""
        return self.raw.get_decision_variable_by_id(variable_id)

    def get_constraint_by_id(self, constraint_id: int) -> EvaluatedConstraint:
        """Get a specific evaluated constraint by ID."""
        return self.raw.get_constraint_by_id(constraint_id)

    def total_violation_l1(self) -> float:
        """
        Calculate total constraint violation using L1 norm (sum of absolute violations).

        Returns the sum of violations across all constraints (including removed constraints):

        - For equality constraints: ``Σ|f(x)|``
        - For inequality constraints: ``Σmax(0, f(x))``

        Returns
        -------
        float
            The total L1 norm violation value.
        """
        return self.raw.total_violation_l1()

    def total_violation_l2(self) -> float:
        """
        Calculate total constraint violation using L2 norm squared (sum of squared violations).

        Returns the sum of squared violations across all constraints (including removed constraints):

        - For equality constraints: ``Σ(f(x))²``
        - For inequality constraints: ``Σ(max(0, f(x)))²``

        Returns
        -------
        float
            The total L2 norm squared violation value.
        """
        return self.raw.total_violation_l2()


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

    raw: _ommx_rust.DecisionVariable

    # Use the new PyO3 Kind enum
    Kind = _ommx_rust.Kind

    BINARY = _ommx_rust.Kind.Binary
    INTEGER = _ommx_rust.Kind.Integer
    CONTINUOUS = _ommx_rust.Kind.Continuous
    SEMI_INTEGER = _ommx_rust.Kind.SemiInteger
    SEMI_CONTINUOUS = _ommx_rust.Kind.SemiContinuous

    @staticmethod
    def from_bytes(data: bytes) -> DecisionVariable:
        rust_dv = _ommx_rust.DecisionVariable.from_bytes(data)
        return DecisionVariable(rust_dv)

    @staticmethod
    def from_protobuf(pb_dv: _DecisionVariable) -> DecisionVariable:
        """Convert from protobuf DecisionVariable to Rust DecisionVariable via serialization"""
        data = pb_dv.SerializeToString()
        return DecisionVariable.from_bytes(data)

    def to_bytes(self) -> bytes:
        return self.raw.to_bytes()

    def to_protobuf(self) -> _DecisionVariable:
        """Convert to protobuf DecisionVariable via serialization"""
        data = self.to_bytes()
        pb_dv = _DecisionVariable()
        pb_dv.ParseFromString(data)
        return pb_dv

    @staticmethod
    def of_type(
        kind: Kind,
        id: int,
        *,
        lower: float,
        upper: float,
        name: Optional[str] = None,
        subscripts: list[int] = [],
        parameters: dict[str, str] = {},
        description: Optional[str] = None,
    ) -> DecisionVariable:
        # Create Rust bound
        rust_bound = _ommx_rust.Bound(lower, upper)

        # Create Rust DecisionVariable - convert Kind enum to int
        rust_dv = _ommx_rust.DecisionVariable(
            id=id,
            kind=kind.to_pb(),
            bound=rust_bound,
            name=name,
            subscripts=subscripts,
            parameters=parameters,
            description=description,
        )

        return DecisionVariable(rust_dv)

    @staticmethod
    def binary(
        id: int,
        *,
        name: Optional[str] = None,
        subscripts: list[int] = [],
        parameters: dict[str, str] = {},
        description: Optional[str] = None,
    ) -> DecisionVariable:
        rust_dv = _ommx_rust.DecisionVariable.binary(
            id=id,
            name=name,
            subscripts=subscripts,
            parameters=parameters,
            description=description,
        )
        return DecisionVariable(rust_dv)

    @staticmethod
    def integer(
        id: int,
        *,
        lower: float = float("-inf"),
        upper: float = float("inf"),
        name: Optional[str] = None,
        subscripts: list[int] = [],
        parameters: dict[str, str] = {},
        description: Optional[str] = None,
    ) -> DecisionVariable:
        bound = _ommx_rust.Bound(lower, upper)
        rust_dv = _ommx_rust.DecisionVariable.integer(
            id=id,
            bound=bound,
            name=name,
            subscripts=subscripts,
            parameters=parameters,
            description=description,
        )
        return DecisionVariable(rust_dv)

    @staticmethod
    def continuous(
        id: int,
        *,
        lower: float = float("-inf"),
        upper: float = float("inf"),
        name: Optional[str] = None,
        subscripts: list[int] = [],
        parameters: dict[str, str] = {},
        description: Optional[str] = None,
    ) -> DecisionVariable:
        bound = _ommx_rust.Bound(lower, upper)
        rust_dv = _ommx_rust.DecisionVariable.continuous(
            id=id,
            bound=bound,
            name=name,
            subscripts=subscripts,
            parameters=parameters,
            description=description,
        )
        return DecisionVariable(rust_dv)

    @staticmethod
    def semi_integer(
        id: int,
        *,
        lower: float = float("-inf"),
        upper: float = float("inf"),
        name: Optional[str] = None,
        subscripts: list[int] = [],
        parameters: dict[str, str] = {},
        description: Optional[str] = None,
    ) -> DecisionVariable:
        bound = _ommx_rust.Bound(lower, upper)
        rust_dv = _ommx_rust.DecisionVariable.semi_integer(
            id=id,
            bound=bound,
            name=name,
            subscripts=subscripts,
            parameters=parameters,
            description=description,
        )
        return DecisionVariable(rust_dv)

    @staticmethod
    def semi_continuous(
        id: int,
        *,
        lower: float = float("-inf"),
        upper: float = float("inf"),
        name: Optional[str] = None,
        subscripts: list[int] = [],
        parameters: dict[str, str] = {},
        description: Optional[str] = None,
    ) -> DecisionVariable:
        bound = _ommx_rust.Bound(lower, upper)
        rust_dv = _ommx_rust.DecisionVariable.semi_continuous(
            id=id,
            bound=bound,
            name=name,
            subscripts=subscripts,
            parameters=parameters,
            description=description,
        )
        return DecisionVariable(rust_dv)

    @property
    def id(self) -> int:
        return self.raw.id

    @property
    def name(self) -> str:
        return self.raw.name

    @property
    def kind(self) -> Kind:
        # Convert int from Rust to PyO3 Kind enum
        return _ommx_rust.Kind.from_pb(self.raw.kind)

    @property
    def bound(self) -> Bound:
        rust_bound = self.raw.bound
        return Bound(lower=rust_bound.lower, upper=rust_bound.upper)

    @property
    def lower(self) -> float:
        """Lower bound of the decision variable"""
        return self.raw.bound.lower

    @property
    def upper(self) -> float:
        """Upper bound of the decision variable"""
        return self.raw.bound.upper

    @property
    def substituted_value(self) -> float | None:
        """
        The value of the decision variable fixed by `:py:attr:`~Instance.partial_evaluate` or presolvers.
        """
        return self.raw.substituted_value

    @property
    def subscripts(self) -> list[int]:
        return list(self.raw.subscripts)

    @property
    def parameters(self) -> dict[str, str]:
        return self.raw.parameters

    @property
    def description(self) -> str:
        return self.raw.description

    def equals_to(self, other: DecisionVariable) -> bool:
        """
        Alternative to ``==`` operator to compare two decision variables.
        """
        # Compare key properties since we can't directly compare Rust objects
        return (
            self.id == other.id
            and self.kind == other.kind
            and self.name == other.name
            and self.bound.lower == other.bound.lower
            and self.bound.upper == other.bound.upper
            and self.subscripts == other.subscripts
            and self.parameters == other.parameters
            and self.description == other.description
        )

    # The special function __eq__ cannot be inherited from VariableBase
    def __eq__(self, other) -> Constraint:  # type: ignore[reportIncompatibleMethodOverride]
        return Constraint(function=self - other, equality=Constraint.EQUAL_TO_ZERO)

    def _as_pandas_entry(self) -> dict:
        return {
            "id": self.id,
            "kind": str(self.kind),
            "lower": self.bound.lower,
            "upper": self.bound.upper,
            "name": self.name if self.name else NA,
            "subscripts": self.subscripts,
            "description": self.description if self.description else NA,
            "substituted_value": self.substituted_value
            if self.substituted_value is not None
            else NA,
        } | {f"parameters.{key}": value for key, value in self.parameters.items()}


class AsConstraint(ABC):
    def __le__(self, other) -> Constraint:
        return Constraint(
            function=self - other, equality=Constraint.LESS_THAN_OR_EQUAL_TO_ZERO
        )

    def __ge__(self, other) -> Constraint:
        return Constraint(
            function=other - self, equality=Constraint.LESS_THAN_OR_EQUAL_TO_ZERO
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

    raw: _ommx_rust.Linear

    def __init__(self, *, terms: dict[int, float | int], constant: float | int = 0):
        self.raw = _ommx_rust.Linear(terms=terms, constant=constant)

    @classmethod
    def from_object(
        cls, obj: float | int | DecisionVariable | _ommx_rust.Linear | Linear
    ) -> Linear:
        if isinstance(obj, Linear):
            return obj
        elif isinstance(obj, _ommx_rust.Linear):
            new = Linear(terms={})
            new.raw = obj
            return new
        elif isinstance(obj, (float, int)):
            return cls.from_raw(_ommx_rust.Linear.constant(obj))
        elif isinstance(obj, DecisionVariable):
            return cls.from_raw(_ommx_rust.Linear.single_term(obj.raw.id, 1))
        else:
            raise TypeError(f"Cannot create Linear from {type(obj).__name__}. ")

    @classmethod
    def from_raw(cls, obj: _ommx_rust.Linear) -> Linear:
        new = Linear(terms={})
        new.raw = obj
        return new

    @property
    def linear_terms(self) -> dict[int, float]:
        """
        Get the terms of the linear function as a dictionary, except for the constant term.
        """
        return self.raw.linear_terms

    @property
    def terms(self) -> dict[tuple[int, ...], float]:
        """
        Linear terms and constant as a dictionary
        """
        return {(id,): value for id, value in self.linear_terms.items()} | {
            (): self.constant_term
        }

    @property
    def constant_term(self) -> float:
        """
        Get the constant term of the linear function
        """
        return self.raw.constant_term

    @staticmethod
    def from_bytes(data: bytes) -> Linear:
        raw = _ommx_rust.Linear.from_bytes(data)
        return Linear.from_raw(raw)

    def to_bytes(self) -> bytes:
        return self.raw.to_bytes()

    @staticmethod
    def random(rng: _ommx_rust.Rng, num_terms: int = 3, max_id: int = 10) -> Linear:
        """
        Create a random linear function using the given random number generator.

        Args:
            rng: Random number generator
            num_terms: Number of terms in the linear function
            max_id: Maximum variable ID to use

        Returns:
            Random Linear function
        """
        raw = _ommx_rust.Linear.random(rng, num_terms, max_id)
        return Linear.from_raw(raw)

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
        return self.raw.almost_equal(other.raw, atol=atol)

    def evaluate(self, state: ToState, *, atol: float | None = None) -> float:
        """
        Evaluate the linear function with the given state.

        Examples
        =========

        .. doctest::

            Evaluate `2 x1 + 3 x2 + 1` with `x1 = 3, x2 = 4, x3 = 5`

            >>> f = Linear(terms={1: 2, 2: 3}, constant=1)
            >>> value = f.evaluate({1: 3, 2: 4, 3: 5}) # Unused ID `3` can be included

            2*3 + 3*4 + 1 = 19
            >>> value
            19.0

            Missing ID raises an error
            >>> f.evaluate({1: 3})
            Traceback (most recent call last):
            ...
            RuntimeError: Missing entry for id: 2

        """
        return self.raw.evaluate(State(state).to_bytes(), atol=atol)

    def partial_evaluate(self, state: ToState, *, atol: float | None = None) -> Linear:
        """
        Partially evaluate the linear function with the given state.

        Examples
        =========

        .. doctest::

            Evaluate `2 x1 + 3 x2 + 1` with `x1 = 3`, yielding `3 x2 + 7`

            >>> f = Linear(terms={1: 2, 2: 3}, constant=1)
            >>> new_f = f.partial_evaluate({1: 3})
            >>> new_f
            Linear(3*x2 + 7)
            >>> new_f.partial_evaluate({2: 4})
            Linear(19)

        """
        new_raw = self.raw.partial_evaluate(State(state).to_bytes(), atol=atol)
        return Linear.from_raw(new_raw)

    def __repr__(self) -> str:
        return self.raw.__repr__()

    def __add__(
        self, rhs: int | float | DecisionVariable | _ommx_rust.Linear | Linear
    ) -> Linear:
        try:
            rhs = Linear.from_object(rhs)
            return Linear.from_raw(self.raw + rhs.raw)
        except TypeError:
            return NotImplemented

    def __radd__(self, other):
        return self + other

    def __iadd__(
        self, rhs: int | float | DecisionVariable | _ommx_rust.Linear | Linear
    ) -> Linear:
        try:
            rhs = Linear.from_object(rhs)
            self.raw.add_assign(rhs.raw)
            return self
        except TypeError:
            return NotImplemented

    def __sub__(
        self, rhs: int | float | DecisionVariable | _ommx_rust.Linear | Linear
    ) -> Linear:
        try:
            rhs = Linear.from_object(rhs)
            return Linear.from_raw(self.raw - rhs.raw)
        except TypeError:
            return NotImplemented

    def __rsub__(self, other):
        return -self + other

    @overload
    def __mul__(self, other: int | float) -> Linear: ...

    @overload
    def __mul__(self, other: DecisionVariable | Linear) -> Quadratic: ...

    def __mul__(
        self, other: int | float | DecisionVariable | _ommx_rust.Linear | Linear
    ) -> Linear | Quadratic:
        if isinstance(other, (float, int)):
            return Linear.from_raw(self.raw.mul_scalar(other))
        if isinstance(other, (DecisionVariable, Linear)):
            rhs = Linear.from_object(other)
            return Quadratic.from_raw(self.raw * rhs.raw)
        return NotImplemented

    def __rmul__(self, other):
        return self * other

    def __neg__(self) -> Linear:
        return -1 * self

    def __eq__(self, other) -> Constraint:  # type: ignore[reportIncompatibleMethodOverride]
        return Constraint(function=self - other, equality=Constraint.EQUAL_TO_ZERO)


@dataclass
class Quadratic(AsConstraint):
    raw: _ommx_rust.Quadratic

    def __init__(
        self,
        *,
        columns: Iterable[int],
        rows: Iterable[int],
        values: Iterable[float | int],
        linear: Optional[Linear] = None,
    ):
        self.raw = _ommx_rust.Quadratic(
            columns=list(columns),
            rows=list(rows),
            values=[float(v) for v in values],
            linear=linear.raw if linear else None,
        )

    @staticmethod
    def from_raw(raw: _ommx_rust.Quadratic) -> Quadratic:
        new = Quadratic(columns=[], rows=[], values=[])
        new.raw = raw
        return new

    @staticmethod
    def from_bytes(data: bytes) -> Quadratic:
        raw = _ommx_rust.Quadratic.from_bytes(data)
        return Quadratic.from_raw(raw)

    def to_bytes(self) -> bytes:
        return self.raw.to_bytes()

    @staticmethod
    def random(rng: _ommx_rust.Rng, num_terms: int = 5, max_id: int = 10) -> Quadratic:
        """
        Create a random quadratic function using the given random number generator.

        Args:
            rng: Random number generator
            num_terms: Number of terms in the quadratic function
            max_id: Maximum variable ID to use

        Returns:
            Random Quadratic function
        """
        raw = _ommx_rust.Quadratic.random(rng, num_terms, max_id)
        return Quadratic.from_raw(raw)

    def almost_equal(self, other: Quadratic, *, atol: float = 1e-10) -> bool:
        """
        Compare two quadratic functions have almost equal coefficients
        """
        return self.raw.almost_equal(other.raw, atol)

    def evaluate(self, state: ToState, *, atol: float | None = None) -> float:
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
            85.0

            Missing ID raises an error
            >>> f.evaluate({1: 3})
            Traceback (most recent call last):
            ...
            RuntimeError: Missing entry for id: 2

        """
        return self.raw.evaluate(State(state).to_bytes(), atol=atol)

    def partial_evaluate(
        self, state: ToState, *, atol: float | None = None
    ) -> Quadratic:
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
            Quadratic(3*x2*x3 + 6*x2 + 1)

        """
        new_raw = self.raw.partial_evaluate(State(state).to_bytes(), atol=atol)
        return Quadratic.from_raw(new_raw)

    @property
    def linear(self) -> Linear | None:
        linear_terms = self.raw.linear_terms
        constant = self.raw.constant_term
        if linear_terms or constant != 0.0:
            return Linear(terms=linear_terms, constant=constant)
        return None

    @property
    def quadratic_terms(self) -> dict[tuple[int, int], float]:
        """Quadratic terms as a dictionary mapping (row, col) to coefficient"""
        return self.raw.quadratic_terms

    @property
    def linear_terms(self) -> dict[int, float]:
        """Linear terms as a dictionary mapping variable id to coefficient"""
        return self.raw.linear_terms

    @property
    def constant_term(self) -> float:
        """Constant term of the quadratic function"""
        return self.raw.constant_term

    @property
    def terms(self) -> dict[tuple[int, ...], float]:
        """All terms as a dictionary mapping variable id tuples to coefficients

        Returns dictionary with tuple keys (hashable) instead of list keys.

        Examples
        =========

        .. doctest::

            >>> from ommx.v1 import DecisionVariable
            >>> x = DecisionVariable.binary(1, name="x")
            >>> y = DecisionVariable.binary(2, name="y")
            >>> quad = x * y + 2 * x + 3
            >>> quad.terms
            {(1,): 2.0, (1, 2): 1.0, (): 3.0}
        """
        return self.raw.terms()

    def __repr__(self) -> str:
        return self.raw.__repr__()

    def __add__(
        self, other: int | float | DecisionVariable | Linear | Quadratic
    ) -> Quadratic:
        if isinstance(other, float) or isinstance(other, int):
            return Quadratic.from_raw(self.raw.add_scalar(other))
        if isinstance(other, DecisionVariable):
            other_linear = Linear(terms={other.raw.id: 1}, constant=0)
            return Quadratic.from_raw(self.raw.add_linear(other_linear.raw))
        if isinstance(other, Linear):
            return Quadratic.from_raw(self.raw.add_linear(other.raw))
        if isinstance(other, Quadratic):
            return Quadratic.from_raw(self.raw + other.raw)
        return NotImplemented

    def __radd__(self, other):
        return self + other

    def __iadd__(
        self, rhs: int | float | DecisionVariable | Linear | Quadratic
    ) -> Quadratic:
        if isinstance(rhs, Quadratic):
            self.raw.add_assign(rhs.raw)
            return self
        else:
            # For other types, fall back to regular addition and assignment
            result = self + rhs
            self.raw = result.raw
            return self

    def __sub__(
        self, other: int | float | DecisionVariable | Linear | Quadratic
    ) -> Quadratic:
        return self + (-other)

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
            return Quadratic.from_raw(self.raw.mul_scalar(other))
        if isinstance(other, DecisionVariable):
            other_linear = Linear(terms={other.raw.id: 1}, constant=0)
            return Polynomial.from_raw(self.raw.mul_linear(other_linear.raw))
        if isinstance(other, Linear):
            return Polynomial.from_raw(self.raw.mul_linear(other.raw))
        if isinstance(other, Quadratic):
            return Polynomial.from_raw(self.raw * other.raw)
        return NotImplemented

    def __rmul__(self, other):
        return self * other

    def __neg__(self) -> Linear:
        return -1 * self

    def __eq__(self, other) -> Constraint:  # type: ignore[reportIncompatibleMethodOverride]
        return Constraint(function=self - other, equality=Constraint.EQUAL_TO_ZERO)


@dataclass
class Polynomial(AsConstraint):
    raw: _ommx_rust.Polynomial

    def __init__(self, *, terms: dict[Sequence[int], float | int] = {}):
        self.raw = _ommx_rust.Polynomial(terms=terms)

    @staticmethod
    def from_raw(raw: _ommx_rust.Polynomial) -> Polynomial:
        new = Polynomial()
        new.raw = raw
        return new

    @staticmethod
    def from_bytes(data: bytes) -> Polynomial:
        raw = _ommx_rust.Polynomial.from_bytes(data)
        return Polynomial.from_raw(raw)

    @property
    def terms(self) -> dict[tuple[int, ...], float]:
        raw_terms = self.raw.terms()
        return {tuple(ids): coeff for ids, coeff in raw_terms.items()}

    def to_bytes(self) -> bytes:
        return self.raw.to_bytes()

    @staticmethod
    def random(
        rng: _ommx_rust.Rng, num_terms: int = 5, max_degree: int = 3, max_id: int = 10
    ) -> Polynomial:
        """
        Create a random polynomial function using the given random number generator.

        Args:
            rng: Random number generator
            num_terms: Number of terms in the polynomial function
            max_degree: Maximum degree of terms
            max_id: Maximum variable ID to use

        Returns:
            Random Polynomial function
        """
        raw = _ommx_rust.Polynomial.random(rng, num_terms, max_degree, max_id)
        return Polynomial.from_raw(raw)

    def almost_equal(self, other: Polynomial, *, atol: float = 1e-10) -> bool:
        """
        Compare two polynomial have almost equal coefficients
        """
        return self.raw.almost_equal(other.raw, atol)

    def evaluate(self, state: ToState, *, atol: float | None = None) -> float:
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
            181.0

            Missing ID raises an error
            >>> f.evaluate({1: 3})
            Traceback (most recent call last):
            ...
            RuntimeError: Missing entry for id: 2

        """
        return self.raw.evaluate(State(state).to_bytes(), atol=atol)

    def partial_evaluate(
        self, state: ToState, *, atol: float | None = None
    ) -> Polynomial:
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
            Polynomial(9*x2*x3 + 1)

        """
        new_raw = self.raw.partial_evaluate(State(state).to_bytes(), atol=atol)
        return Polynomial.from_raw(new_raw)

    def __repr__(self) -> str:
        return self.raw.__repr__()

    def __add__(
        self, other: int | float | DecisionVariable | Linear | Quadratic | Polynomial
    ) -> Polynomial:
        if isinstance(other, float) or isinstance(other, int):
            return Polynomial.from_raw(self.raw.add_scalar(other))
        if isinstance(other, DecisionVariable):
            other_linear = Linear(terms={other.raw.id: 1}, constant=0)
            return Polynomial.from_raw(self.raw.add_linear(other_linear.raw))
        if isinstance(other, Linear):
            return Polynomial.from_raw(self.raw.add_linear(other.raw))
        if isinstance(other, Quadratic):
            return Polynomial.from_raw(self.raw.add_quadratic(other.raw))
        if isinstance(other, Polynomial):
            return Polynomial.from_raw(self.raw + other.raw)
        return NotImplemented

    def __radd__(self, other):
        return self + other

    def __iadd__(
        self, rhs: int | float | DecisionVariable | Linear | Quadratic | Polynomial
    ) -> Polynomial:
        if isinstance(rhs, Polynomial):
            self.raw.add_assign(rhs.raw)
            return self
        else:
            # For other types, fall back to regular addition and assignment
            result = self + rhs
            self.raw = result.raw
            return self

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
            return Polynomial.from_raw(self.raw.mul_scalar(other))
        if isinstance(other, DecisionVariable):
            other_linear = Linear(terms={other.raw.id: 1}, constant=0)
            return Polynomial.from_raw(self.raw.mul_linear(other_linear.raw))
        if isinstance(other, Linear):
            return Polynomial.from_raw(self.raw.mul_linear(other.raw))
        if isinstance(other, Quadratic):
            return Polynomial.from_raw(self.raw.mul_quadratic(other.raw))
        if isinstance(other, Polynomial):
            return Polynomial.from_raw(self.raw * other.raw)
        return NotImplemented

    def __rmul__(self, other):
        return self * other

    def __neg__(self) -> Linear:
        return -1 * self

    def __eq__(self, other) -> Constraint:  # type: ignore[reportIncompatibleMethodOverride]
        return Constraint(function=self - other, equality=Constraint.EQUAL_TO_ZERO)


@dataclass
class Function(AsConstraint):
    raw: _ommx_rust.Function

    def __init__(
        self,
        inner: int
        | float
        | DecisionVariable
        | Linear
        | Quadratic
        | Polynomial
        | _ommx_rust.Function
        | _Function,
    ):
        if isinstance(inner, (int, float)):
            self.raw = _ommx_rust.Function.from_scalar(inner)
        elif isinstance(inner, DecisionVariable):
            self.raw = _ommx_rust.Function.from_linear(
                _ommx_rust.Linear.single_term(inner.raw.id, 1)
            )
        elif isinstance(inner, Linear):
            self.raw = _ommx_rust.Function.from_linear(inner.raw)
        elif isinstance(inner, Quadratic):
            self.raw = _ommx_rust.Function.from_quadratic(inner.raw)
        elif isinstance(inner, Polynomial):
            self.raw = _ommx_rust.Function.from_polynomial(inner.raw)
        elif isinstance(inner, _ommx_rust.Function):
            self.raw = inner
        elif isinstance(inner, _Function):
            self.raw = _ommx_rust.Function.from_bytes(inner.SerializeToString())
        else:
            raise TypeError(f"Cannot create Function from {type(inner).__name__}")

    @property
    def terms(self) -> dict[tuple[int, ...], float]:
        """All terms as a dictionary mapping variable id tuples to coefficients

        Returns dictionary with tuple keys (hashable) instead of list keys.

        Examples
        =========

        .. doctest::

            >>> from ommx.v1 import Function, Linear, DecisionVariable
            >>> x = DecisionVariable.binary(1, name="x")
            >>> linear = Linear(terms={1: 2.5}, constant=1.0)
            >>> func = Function(linear)
            >>> func.terms
            {(1,): 2.5, (): 1.0}
        """
        return self.raw.terms()

    @staticmethod
    def from_raw(raw: _ommx_rust.Function) -> Function:
        new = Function(0)
        new.raw = raw
        return new

    @staticmethod
    def from_bytes(data: bytes) -> Function:
        new = Function(0)
        new.raw = _ommx_rust.Function.from_bytes(data)
        return new

    def to_bytes(self) -> bytes:
        return self.raw.to_bytes()

    @staticmethod
    def random(
        rng: _ommx_rust.Rng, num_terms: int = 5, max_degree: int = 3, max_id: int = 10
    ) -> Function:
        """
        Create a random function using the given random number generator.

        Args:
            rng: Random number generator
            num_terms: Number of terms in the function
            max_degree: Maximum degree of terms
            max_id: Maximum variable ID to use

        Returns:
            Random Function
        """
        raw = _ommx_rust.Function.random(rng, num_terms, max_degree, max_id)
        return Function.from_raw(raw)

    def almost_equal(self, other: Function, *, atol: float = 1e-10) -> bool:
        """
        Compare two functions have almost equal coefficients as a polynomial
        """
        return self.raw.almost_equal(other.raw, atol)

    def evaluate(self, state: ToState, *, atol: float | None = None) -> float:
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
            85.0

            Missing ID raises an error
            >>> f.evaluate({1: 3})
            Traceback (most recent call last):
            ...
            RuntimeError: Missing entry for id: 2

        """
        return self.raw.evaluate(State(state).to_bytes(), atol=atol)

    def partial_evaluate(
        self, state: ToState, *, atol: float | None = None
    ) -> Function:
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
            Function(3*x2*x3 + 6*x2 + 1)

        """
        new_raw = self.raw.partial_evaluate(State(state).to_bytes(), atol=atol)
        return Function.from_raw(new_raw)

    def used_decision_variable_ids(self) -> set[int]:
        """
        Get the IDs of decision variables used in the function.
        """
        return self.raw.required_ids()

    def content_factor(self) -> float:
        r"""
        For given polynomial :math:`f(x)`, get the minimal positive factor :math:`a` which makes all coefficient of :math:`a f(x)` integer.
        See also https://en.wikipedia.org/wiki/Primitive_part_and_content

        Examples
        =========

        :math:`\frac{1}{3} x_0 + \frac{3}{2} x_1` can be multiplied by 6 to make all coefficients integer.

        >>> x = [DecisionVariable.integer(i) for i in range(2)]
        >>> f = Function((1.0/3.0)*x[0] + (3.0/2.0)*x[1])
        >>> a = f.content_factor()
        >>> (a, a*f)
        (6.0, Function(2*x0 + 9*x1))

        This works even for non-rational numbers like :math:`\pi` because 64-bit float is actually rational.

        >>> import math
        >>> f = Function(math.pi*x[0] + 3*math.pi*x[1])
        >>> a = f.content_factor()
        >>> (a, a*f)
        (0.3183098861837907, Function(x0 + 3*x1))

        But this returns very large number if there is no multiplier:

        >>> f = Function(math.pi*x[0] + math.e*x[1])
        >>> a = f.content_factor()
        >>> (a, a*f)
        (3122347504612692.0, Function(9809143982445656*x0 + 8487420483923125*x1))

        In practice, you must check if the multiplier is enough small.

        """
        return self.raw.content_factor()

    def degree(self) -> int:
        """
        Get the degree of the polynomial function.

        Returns:
            The degree of the polynomial (0 for constants, 1 for linear, 2 for quadratic, etc.)
        """
        return self.raw.degree()

    def num_terms(self) -> int:
        """
        Get the number of terms in the polynomial function.

        Returns:
            The number of terms in the polynomial
        """
        return self.raw.num_terms()

    def as_linear(self) -> Linear | None:
        """
        Try to convert the function to a Linear object if it's linear.

        Returns:
            A Linear object if the function is linear, None otherwise
        """
        linear_raw = self.raw.as_linear()
        if linear_raw is None:
            return None
        return Linear.from_raw(linear_raw)

    def as_quadratic(self) -> Quadratic | None:
        """
        Try to convert the function to a Quadratic object if it's quadratic.

        Returns:
            A Quadratic object if the function is quadratic, None otherwise
        """
        quadratic_raw = self.raw.as_quadratic()
        if quadratic_raw is None:
            return None
        return Quadratic.from_raw(quadratic_raw)

    @property
    def linear_terms(self) -> dict[int, float]:
        """
        Get linear terms as a dictionary mapping variable id to coefficient.

        Returns:
            Dictionary mapping variable IDs to their linear coefficients.
            Returns empty dict if function has no linear terms.
            Works for all polynomial functions by filtering only degree-1 terms.

        Examples:
            >>> x1 = DecisionVariable.integer(1)
            >>> x2 = DecisionVariable.integer(2)
            >>> f = Function(2*x1 + 3*x2 + 5)
            >>> f.linear_terms
            {1: 2.0, 2: 3.0}

            >>> f_const = Function(5)
            >>> f_const.linear_terms
            {}

            >>> # Works for higher degree polynomials too
            >>> f_cubic = Function(x1*x1*x1 + 2*x1 + 5)
            >>> f_cubic.linear_terms
            {1: 2.0}
        """
        return self.raw.linear_terms

    @property
    def quadratic_terms(self) -> dict[tuple[int, int], float]:
        """
        Get quadratic terms as a dictionary mapping (row, col) to coefficient.

        Returns:
            Dictionary mapping variable ID pairs to their quadratic coefficients.
            Returns empty dict if function has no quadratic terms.
            Works for all polynomial functions by filtering only degree-2 terms.

        Examples:
            >>> x1 = DecisionVariable.integer(1)
            >>> x2 = DecisionVariable.integer(2)
            >>> f = Function(2*x1*x2 + 3*x1 + 5)
            >>> f.quadratic_terms
            {(1, 2): 2.0}

            >>> f_linear = Function(3*x1 + 5)
            >>> f_linear.quadratic_terms
            {}

            >>> # Works for higher degree polynomials too
            >>> f_cubic = Function(x1*x1*x1 + 2*x1*x2 + 5)
            >>> f_cubic.quadratic_terms
            {(1, 2): 2.0}
        """
        return self.raw.quadratic_terms

    @property
    def constant_term(self) -> float:
        """
        Get the constant term of the function.

        Returns:
            The constant term. Returns 0.0 if function has no constant term.
            Works for all polynomial functions by filtering the degree-0 term.

        Examples:
            >>> x1 = DecisionVariable.integer(1)
            >>> f = Function(2*x1 + 5)
            >>> f.constant_term
            5.0

            >>> f_no_const = Function(2*x1)
            >>> f_no_const.constant_term
            0.0

            >>> f_const_only = Function(7)
            >>> f_const_only.constant_term
            7.0

            >>> # Works for higher degree polynomials too
            >>> f_cubic = Function(x1*x1*x1 + 2*x1 + 5)
            >>> f_cubic.constant_term
            5.0
        """
        return self.raw.constant_term

    def reduce_binary_power(self, binary_ids: set[int]) -> bool:
        """
        Reduce binary powers in the function.

        For binary variables, x^n = x for any n >= 1, so we can reduce higher powers to linear terms.

        Args:
            binary_ids: Set of binary variable IDs to reduce powers for

        Returns:
            True if any reduction was performed, False otherwise

        Examples
        =========

        Consider a function with binary variables and quadratic terms:

        >>> from ommx.v1 import DecisionVariable, Function
        >>> x0 = DecisionVariable.binary(0)
        >>> x1 = DecisionVariable.binary(1)
        >>> f = Function(x0 * x0 + x0 * x1)  # x0^2 + x0*x1
        >>> f
        Function(x0*x0 + x0*x1)

        After reducing binary powers for variable 0, x0^2 becomes x0:

        >>> changed = f.reduce_binary_power({0})
        >>> changed
        True
        >>> f
        Function(x0*x1 + x0)

        Running it again should not change anything:

        >>> changed = f.reduce_binary_power({0})
        >>> changed
        False

        """
        return self.raw.reduce_binary_power(binary_ids)

    def __repr__(self) -> str:
        return self.raw.__repr__()

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
            rhs = _ommx_rust.Function.from_linear(other.raw)
        elif isinstance(other, Quadratic):
            rhs = _ommx_rust.Function.from_quadratic(other.raw)
        elif isinstance(other, Polynomial):
            rhs = _ommx_rust.Function.from_polynomial(other.raw)
        elif isinstance(other, Function):
            rhs = other.raw
        else:
            return NotImplemented
        return Function.from_raw(self.raw + rhs)

    def __radd__(self, other):
        return self + other

    def __iadd__(
        self,
        rhs: int
        | float
        | DecisionVariable
        | Linear
        | Quadratic
        | Polynomial
        | Function,
    ) -> Function:
        if isinstance(rhs, Function):
            self.raw.add_assign(rhs.raw)
            return self
        else:
            # For other types, fall back to regular addition and assignment
            result = self + rhs
            self.raw = result.raw
            return self

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
        other: (
            int | float | DecisionVariable | Linear | Quadratic | Polynomial | Function
        ),
    ) -> Function:
        if isinstance(other, float) or isinstance(other, int):
            rhs = _ommx_rust.Function.from_scalar(other)
        elif isinstance(other, DecisionVariable):
            rhs = _ommx_rust.Function.from_linear(
                _ommx_rust.Linear.single_term(other.raw.id, 1)
            )
        elif isinstance(other, Linear):
            rhs = _ommx_rust.Function.from_linear(other.raw)
        elif isinstance(other, Quadratic):
            rhs = _ommx_rust.Function.from_quadratic(other.raw)
        elif isinstance(other, Polynomial):
            rhs = _ommx_rust.Function.from_polynomial(other.raw)
        elif isinstance(other, Function):
            rhs = other.raw
        else:
            return NotImplemented
        return Function.from_raw(self.raw * rhs)

    def __rmul__(self, other):
        return self * other

    def __neg__(self) -> Function:
        return -1 * self

    def __eq__(self, other) -> Constraint:  # type: ignore[reportIncompatibleMethodOverride]
        return Constraint(function=self - other, equality=Constraint.EQUAL_TO_ZERO)


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
        Constraint(x1 + x2 - 1 == 0)

        To set the name or other attributes, use methods like :py:meth:`add_name`.

        >>> (x + y <= 5).add_name("constraint 1")
        Constraint(x1 + x2 - 5 <= 0)

    """

    raw: _ommx_rust.Constraint
    _counter: int = 0

    EQUAL_TO_ZERO = _ommx_rust.Equality.EqualToZero
    LESS_THAN_OR_EQUAL_TO_ZERO = _ommx_rust.Equality.LessThanOrEqualToZero

    def __init__(
        self,
        *,
        function: int
        | float
        | DecisionVariable
        | Linear
        | Quadratic
        | Polynomial
        | Function
        | _ommx_rust.Function,
        equality: _ommx_rust.Equality,
        id: Optional[int] = None,
        name: Optional[str] = None,
        description: Optional[str] = None,
        subscripts: list[int] = [],
        parameters: dict[str, str] = {},
    ):
        if id is None:
            id = Constraint._counter
            Constraint._counter += 1
        if id > Constraint._counter:
            Constraint._counter = id + 1

        if not isinstance(function, Function):
            function = Function(function)

        # Convert equality to Rust Equality enum
        if isinstance(equality, _ommx_rust.Equality):
            rust_equality = equality
        else:
            # Handle Protocol Buffer integer values
            rust_equality = _ommx_rust.Equality.from_pb(equality)

        self.raw = _ommx_rust.Constraint(
            id=id,
            function=function.raw,
            equality=rust_equality,
            name=name,
            subscripts=subscripts or [],
            description=description,
            parameters=parameters or {},
        )

    @staticmethod
    def from_raw(raw: _ommx_rust.Constraint) -> Constraint:
        new = Constraint(function=0, equality=Constraint.EQUAL_TO_ZERO)
        new.raw = raw
        Constraint._counter = max(Constraint._counter, raw.id + 1)
        return new

    @staticmethod
    def from_bytes(data: bytes) -> Constraint:
        rust_constraint = _ommx_rust.Constraint.from_bytes(data)
        return Constraint.from_raw(rust_constraint)

    @staticmethod
    def from_protobuf(pb_constraint: _Constraint) -> Constraint:
        """Convert from protobuf Constraint to Rust Constraint via serialization"""
        data = pb_constraint.SerializeToString()
        return Constraint.from_bytes(data)

    def to_bytes(self) -> bytes:
        return self.raw.to_bytes()

    def to_protobuf(self) -> _Constraint:
        """Convert to protobuf Constraint"""
        pb_constraint = _Constraint()
        pb_constraint.ParseFromString(self.to_bytes())
        return pb_constraint

    def set_id(self, id: int) -> Constraint:
        """
        Overwrite the constraint ID.
        """
        self.raw.set_id(id)
        return self

    def add_name(self, name: str) -> Constraint:
        """
        Add or update the name of the constraint.
        """
        self.raw.set_name(name)
        return self

    def add_description(self, description: str) -> Constraint:
        """
        Add or update the description of the constraint.
        """
        self.raw.set_description(description)
        return self

    def add_subscripts(self, subscripts: list[int]) -> Constraint:
        """
        Add subscripts to the constraint.
        """
        self.raw.add_subscripts(subscripts)
        return self

    def add_parameters(self, parameters: dict[str, str]) -> Constraint:
        """
        Add or update parameters of the constraint.
        """
        for key, value in parameters.items():
            self.raw.add_parameter(key, value)
        return self

    @property
    def function(self) -> Function:
        return Function(self.raw.function)

    @property
    def id(self) -> int:
        return self.raw.id

    @property
    def equality(self) -> _ommx_rust.Equality:
        # Return the PyO3 Equality enum directly from Rust
        return self.raw.equality

    @property
    def name(self) -> str | None:
        name = self.raw.name
        return name if name else None

    @property
    def description(self) -> str | None:
        desc = self.raw.description
        return desc if desc else None

    @property
    def subscripts(self) -> list[int]:
        return list(self.raw.subscripts)

    @property
    def parameters(self) -> dict[str, str]:
        return self.raw.parameters

    def __repr__(self) -> str:
        return self.raw.__repr__()

    def _as_pandas_entry(self) -> dict:
        c = self.raw
        return {
            "id": c.id,
            "equality": str(c.equality),
            "type": c.function.type_name,
            "used_ids": c.function.required_ids(),
            "name": c.name if c.name else NA,
            "subscripts": c.subscripts,
            "description": NA,  # Description not supported in Rust implementation
        }  # Parameters not supported in Rust implementation


@dataclass
class RemovedConstraint:
    """
    Constraints removed while preprocessing
    """

    raw: _ommx_rust.RemovedConstraint

    def __init__(self, raw: _ommx_rust.RemovedConstraint):
        self.raw = raw

    @staticmethod
    def from_raw(raw: _ommx_rust.RemovedConstraint) -> RemovedConstraint:
        return RemovedConstraint(raw)

    @staticmethod
    def from_protobuf(pb_removed_constraint: _RemovedConstraint) -> RemovedConstraint:
        """Convert from protobuf RemovedConstraint to Rust RemovedConstraint"""
        # Use Rust decode method to convert Protocol Buffer to Rust implementation
        rust_removed_constraint = _ommx_rust.RemovedConstraint.from_bytes(
            pb_removed_constraint.SerializeToString()
        )
        return RemovedConstraint(rust_removed_constraint)

    def __repr__(self) -> str:
        return self.raw.__repr__()

    @property
    def equality(self) -> _ommx_rust.Equality:
        # Return the PyO3 Equality enum directly from Rust
        return self.raw.constraint.equality

    @property
    def id(self) -> int:
        return self.raw.id

    @property
    def function(self) -> Function:
        return Function(self.raw.constraint.function)

    @property
    def name(self) -> str | None:
        name = self.raw.name
        return name if name else None

    @property
    def description(self) -> str | None:
        desc = self.raw.constraint.description
        return desc if desc else None

    @property
    def subscripts(self) -> list[int]:
        return list(self.raw.constraint.subscripts)

    @property
    def parameters(self) -> dict[str, str]:
        return self.raw.constraint.parameters

    @property
    def removed_reason(self) -> str:
        return self.raw.removed_reason

    @property
    def removed_reason_parameters(self) -> dict[str, str]:
        return self.raw.removed_reason_parameters

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
        >>> solution.decision_variables_df  # doctest: +NORMALIZE_WHITESPACE
              kind  lower  upper  name subscripts description substituted_value  value
        id
        0   Binary    0.0    1.0  <NA>         []        <NA>              <NA>    1.0
        1   Binary    0.0    1.0  <NA>         []        <NA>              <NA>    0.0
        2   Binary    0.0    1.0  <NA>         []        <NA>              <NA>    0.0

    :meth:`best_feasible` returns the best feasible sample, i.e. the largest objective value among feasible samples:

    .. doctest::

        >>> solution = sample_set.best_feasible
        >>> solution.objective
        3.0
        >>> solution.decision_variables_df  # doctest: +NORMALIZE_WHITESPACE
              kind  lower  upper  name subscripts description substituted_value  value
        id                                                                            
        0   Binary    0.0    1.0  <NA>         []        <NA>              <NA>    0.0
        1   Binary    0.0    1.0  <NA>         []        <NA>              <NA>    0.0
        2   Binary    0.0    1.0  <NA>         []        <NA>              <NA>    1.0

    Of course, the sample of smallest objective value is returned for minimization problems.

    """

    raw: _ommx_rust.SampleSet

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
        raw = _ommx_rust.SampleSet.from_bytes(data)
        return SampleSet(raw)

    def to_bytes(self) -> bytes:
        return self.raw.to_bytes()

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
            ascending=[False, self.raw.sense == Sense.Minimize],
        ).set_index("sample_id")

    @property
    def summary_with_constraints(self) -> DataFrame:
        def _constraint_label(c: _ommx_rust.SampledConstraint) -> str:
            name = ""
            if c.name:
                name += c.name
            else:
                return f"{c.id}"
            if c.subscripts:
                name += f"{c.subscripts}"
            # Parameters are not directly available in Rust SampledConstraint
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
            ascending=[False, self.raw.sense == Sense.Minimize],
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
        return self.raw.objectives

    @property
    def sample_ids(self) -> list[int]:
        return self.summary.index.tolist()  # type: ignore[attr-defined]

    @property
    def decision_variables_df(self) -> DataFrame:
        df = DataFrame(
            {
                "id": v.id,
                "kind": str(v.kind),
                "lower": v.bound.lower,
                "upper": v.bound.upper,
                "name": v.name,
                "subscripts": v.subscripts,
                "description": v.description,
            }
            | {str(id): value for id, value in v.samples.items()}
            for v in self.raw.decision_variables
        )
        if not df.empty:
            return df.set_index("id")
        return df

    @property
    def constraints_df(self) -> DataFrame:
        df = DataFrame(
            {
                "id": c.id,
                "equality": str(c.equality),
                "used_ids": set(c.used_decision_variable_ids),
                "name": c.name,
                "subscripts": c.subscripts,
                "description": c.description,
                "removed_reason": c.removed_reason,
            }
            | {
                f"removed_reason.{key}": value
                for key, value in c.removed_reason_parameters.items()
            }
            | {f"value.{id}": value for id, value in c.evaluated_values.items()}
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
        return self.raw.extract_decision_variables(name, sample_id)

    @property
    def decision_variable_names(self) -> set[str]:
        """
        Get all unique decision variable names in this sample set.

        Returns a set of all unique variable names. Variables without names are not included.

        Examples
        =========

        .. doctest::

            >>> from ommx.v1 import Instance, DecisionVariable
            >>> x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
            >>> y = [DecisionVariable.binary(i+3, name="y", subscripts=[i]) for i in range(2)]
            >>> instance = Instance.from_components(
            ...     decision_variables=x + y,
            ...     objective=sum(x) + sum(y),
            ...     constraints=[],
            ...     sense=Instance.MAXIMIZE,
            ... )
            >>> sample_set = instance.evaluate_samples({0: {i: 1 for i in range(5)}})
            >>> sorted(sample_set.decision_variable_names)
            ['x', 'y']

        """
        return self.raw.decision_variable_names

    def extract_all_decision_variables(
        self, sample_id: int
    ) -> dict[str, dict[tuple[int, ...], float]]:
        """
        Extract all decision variables grouped by name for a given sample ID.

        Returns a mapping from variable name to a mapping from subscripts to values.
        This is useful for extracting all variables at once in a structured format.
        Variables without names are not included in the result.

        :raises ValueError: If a decision variable with parameters is found, or if the same name and subscript combination is found multiple times, or if the sample ID is invalid.

        Examples
        =========

        .. doctest::

            >>> from ommx.v1 import Instance, DecisionVariable
            >>> x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
            >>> y = [DecisionVariable.binary(i+3, name="y", subscripts=[i]) for i in range(2)]
            >>> instance = Instance.from_components(
            ...     decision_variables=x + y,
            ...     objective=sum(x) + sum(y),
            ...     constraints=[],
            ...     sense=Instance.MAXIMIZE,
            ... )
            >>> sample_set = instance.evaluate_samples({0: {i: 1 for i in range(5)}})
            >>> all_vars = sample_set.extract_all_decision_variables(0)
            >>> all_vars["x"]
            {(0,): 1.0, (1,): 1.0, (2,): 1.0}
            >>> all_vars["y"]
            {(0,): 1.0, (1,): 1.0}

        """
        return self.raw.extract_all_decision_variables(sample_id)

    def extract_constraints(
        self, name: str, sample_id: int
    ) -> dict[tuple[int, ...], float]:
        """
        Extract evaluated constraint violations for a given constraint name and sample ID.
        """
        return self.raw.extract_constraints(name, sample_id)

    def get(self, sample_id: int) -> Solution:
        """
        Get a sample for a given ID as a solution format
        """
        solution = self.raw.get(sample_id)
        return Solution(solution)

    def get_sample_by_id(self, sample_id: int) -> Solution:
        """
        Get sample by ID (alias for get method)
        """
        return self.get(sample_id)

    def get_decision_variable_by_id(self, variable_id: int) -> SampledDecisionVariable:
        """Get a specific sampled decision variable by ID."""
        return self.raw.get_decision_variable_by_id(variable_id)

    def get_constraint_by_id(self, constraint_id: int) -> SampledConstraint:
        """Get a specific sampled constraint by ID."""
        return self.raw.get_constraint_by_id(constraint_id)

    @property
    def decision_variables(self) -> list[SampledDecisionVariable]:
        """Get sampled decision variables as a list sorted by ID."""
        return self.raw.decision_variables

    @property
    def constraints(self) -> list[SampledConstraint]:
        """Get sampled constraints as a list sorted by ID."""
        return self.raw.constraints

    @property
    def best_feasible_id(self) -> int:
        """
        Get the sample ID of the best feasible solution.

        Raises
        ------
        RuntimeError
            If no feasible solution exists.
        """
        return self.raw.best_feasible_id

    @property
    def best_feasible_relaxed_id(self) -> int:
        """
        Get the sample ID of the best feasible solution without relaxation.

        Raises
        ------
        RuntimeError
            If no feasible solution exists.
        """
        return self.raw.best_feasible_relaxed_id

    @property
    def best_feasible_unrelaxed_id(self) -> int:
        """
        Get the sample ID of the best feasible solution without relaxation.

        Raises
        ------
        RuntimeError
            If no feasible solution exists.
        """
        return self.best_feasible_unrelaxed_id

    @property
    def best_feasible(self) -> Solution:
        """
        Get the best feasible solution.

        Raises
        ------
        RuntimeError
            If no feasible solution exists.
        """
        solution = self.raw.best_feasible
        return Solution(solution)

    @property
    def best_feasible_relaxed(self) -> Solution:
        """
        Get the best feasible solution without relaxation.

        Raises
        ------
        RuntimeError
            If no feasible solution exists.
        """
        solution = self.raw.best_feasible_relaxed
        return Solution(solution)

    @property
    def best_feasible_unrelaxed(self) -> Solution:
        """
        Get the best feasible solution without relaxation.

        Raises
        ------
        RuntimeError
            If no feasible solution exists.
        """
        solution = self.raw.best_feasible_unrelaxed
        return Solution(solution)

    @property
    def sense(self) -> _ommx_rust.Sense:
        return self.raw.sense


@dataclass
class DecisionVariableAnalysis:
    """
    Analysis of decision variables in an optimization problem instance.

    This class provides comprehensive information about decision variables including
    their types, usage patterns, and bounds.
    """

    raw: _ommx_rust.DecisionVariableAnalysis

    def __init__(self, raw: _ommx_rust.DecisionVariableAnalysis):
        """Initialize DecisionVariableAnalysis from raw Rust object."""
        self.raw = raw

    def used_binary(self) -> dict[int, "Bound"]:
        """
        Get binary variables that are actually used in the problem.

        Returns
        -------
        dict[int, Bound]
            Mapping from variable ID to Bound object for binary variables
        """
        return {
            var_id: Bound.__new__(Bound).__init_from_raw__(bound)
            for var_id, bound in self.raw.used_binary().items()
        }

    def used_integer(self) -> dict[int, "Bound"]:
        """
        Get integer variables that are actually used in the problem.

        Returns
        -------
        dict[int, Bound]
            Mapping from variable ID to Bound object for integer variables
        """
        return {
            var_id: Bound.__new__(Bound).__init_from_raw__(bound)
            for var_id, bound in self.raw.used_integer().items()
        }

    def used_continuous(self) -> dict[int, "Bound"]:
        """
        Get continuous variables that are actually used in the problem.

        Returns
        -------
        dict[int, Bound]
            Mapping from variable ID to Bound object for continuous variables
        """
        return {
            var_id: Bound.__new__(Bound).__init_from_raw__(bound)
            for var_id, bound in self.raw.used_continuous().items()
        }

    def used_semi_integer(self) -> dict[int, "Bound"]:
        """
        Get semi-integer variables that are actually used in the problem.

        Returns
        -------
        dict[int, Bound]
            Mapping from variable ID to Bound object for semi-integer variables
        """
        return {
            var_id: Bound.__new__(Bound).__init_from_raw__(bound)
            for var_id, bound in self.raw.used_semi_integer().items()
        }

    def used_semi_continuous(self) -> dict[int, "Bound"]:
        """
        Get semi-continuous variables that are actually used in the problem.

        Returns
        -------
        dict[int, Bound]
            Mapping from variable ID to Bound object for semi-continuous variables
        """
        return {
            var_id: Bound.__new__(Bound).__init_from_raw__(bound)
            for var_id, bound in self.raw.used_semi_continuous().items()
        }

    def used_decision_variable_ids(self) -> set[int]:
        """
        Get the set of decision variable IDs that are actually used in the problem.

        Returns
        -------
        set[int]
            Set of variable IDs used in either objective function or constraints
        """
        return self.raw.used_decision_variable_ids()

    def all_decision_variable_ids(self) -> set[int]:
        """
        Get the set of all decision variable IDs defined in the problem.

        Returns
        -------
        set[int]
            Set of all variable IDs defined in the problem
        """
        return self.raw.all_decision_variable_ids()

    def used_in_objective(self) -> set[int]:
        """
        Get decision variables used in the objective function.

        Returns
        -------
        set[int]
            Set of variable IDs used in the objective function
        """
        return self.raw.used_in_objective()

    def used_in_constraints(self) -> dict[int, set[int]]:
        """
        Get decision variables used in each constraint.

        Returns
        -------
        dict[int, set[int]]
            Mapping from constraint ID to set of variable IDs used in that constraint
        """
        return self.raw.used_in_constraints()

    def fixed(self) -> dict[int, float]:
        """
        Get variables with fixed/substituted values.

        Returns
        -------
        dict[int, float]
            Mapping from variable ID to fixed value
        """
        return self.raw.fixed()

    def irrelevant(self) -> set[int]:
        """
        Get variables that are not used anywhere in the problem.

        Returns
        -------
        set[int]
            Set of variable IDs that are irrelevant (not used in objective or constraints)
        """
        return self.raw.irrelevant()

    def dependent(self) -> set[int]:
        """
        Get variables that depend on other variables.

        Returns
        -------
        set[int]
            Set of variable IDs that are dependent on other variables
        """
        return self.raw.dependent()

    def to_dict(self) -> dict:
        """
        Convert the analysis to a dictionary representation.

        Returns
        -------
        dict
            Dictionary containing all analysis information including variable bounds,
            usage patterns, and categorizations
        """
        return self.raw.to_dict()

    def __repr__(self) -> str:
        """Return a detailed string representation."""
        return repr(self.raw)


@dataclass
class Bound:
    """
    Variable bound representing the valid range for a decision variable.

    This class provides a clean interface for working with variable bounds,
    including lower bounds, upper bounds, and various utility methods.
    """

    raw: _ommx_rust.Bound

    def __init__(self, lower: float, upper: float):
        """
        Create a new bound with specified lower and upper limits.

        Parameters
        ----------
        lower : float
            Lower bound (can be -inf)
        upper : float
            Upper bound (can be +inf)
        """
        self.raw = _ommx_rust.Bound(lower, upper)

    @classmethod
    def unbounded(cls) -> "Bound":
        """Create an unbounded range (-inf, +inf)."""
        return cls.__new__(cls).__init_from_raw__(_ommx_rust.Bound.unbounded())

    @classmethod
    def positive(cls) -> "Bound":
        """Create a positive bound [0, +inf)."""
        return cls.__new__(cls).__init_from_raw__(_ommx_rust.Bound.positive())

    @classmethod
    def negative(cls) -> "Bound":
        """Create a negative bound (-inf, 0]."""
        return cls.__new__(cls).__init_from_raw__(_ommx_rust.Bound.negative())

    @classmethod
    def of_binary(cls) -> "Bound":
        """Create a binary variable bound [0, 1]."""
        return cls.__new__(cls).__init_from_raw__(_ommx_rust.Bound.of_binary())

    def __init_from_raw__(self, raw: _ommx_rust.Bound) -> "Bound":
        """Internal method to initialize from raw Rust object."""
        self.raw = raw
        return self

    @property
    def lower(self) -> float:
        """Get the lower bound."""
        return self.raw.lower

    @property
    def upper(self) -> float:
        """Get the upper bound."""
        return self.raw.upper

    def width(self) -> float:
        """Get the width (upper - lower) of the bound."""
        return self.raw.width()

    def is_finite(self) -> bool:
        """Check if both bounds are finite (not infinite)."""
        return self.raw.is_finite()

    def contains(self, value: float, atol: float = 1e-6) -> bool:
        """Check if a value is within the bound with tolerance."""
        return self.raw.contains(value, atol)

    def nearest_to_zero(self) -> float:
        """Get the value within the bound that is nearest to zero."""
        return self.raw.nearest_to_zero()

    def intersection(self, other: "Bound") -> Optional["Bound"]:
        """Get the intersection of two bounds, or None if no intersection."""
        result = self.raw.intersection(other.raw)
        if result is None:
            return None
        return Bound.__new__(Bound).__init_from_raw__(result)

    def __repr__(self) -> str:
        return f"Bound(lower={self.lower}, upper={self.upper})"

    def __str__(self) -> str:
        return self.raw.__str__()
