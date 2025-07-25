# This file is automatically generated by pyo3_stub_gen
# ruff: noqa: E501, F401

import builtins
import os
import pathlib
import typing
from enum import Enum

class ArtifactArchive:
    image_name: typing.Optional[builtins.str]
    annotations: builtins.dict[builtins.str, builtins.str]
    layers: builtins.list[Descriptor]
    @staticmethod
    def from_oci_archive(
        path: builtins.str | os.PathLike | pathlib.Path,
    ) -> ArtifactArchive: ...
    def get_blob(self, digest: builtins.str) -> bytes: ...
    def push(self) -> None: ...

class ArtifactArchiveBuilder:
    @staticmethod
    def new_unnamed(
        path: builtins.str | os.PathLike | pathlib.Path,
    ) -> ArtifactArchiveBuilder: ...
    @staticmethod
    def new(
        path: builtins.str | os.PathLike | pathlib.Path, image_name: builtins.str
    ) -> ArtifactArchiveBuilder: ...
    @staticmethod
    def temp() -> ArtifactArchiveBuilder: ...
    def add_layer(
        self,
        media_type: builtins.str,
        blob: bytes,
        annotations: typing.Mapping[builtins.str, builtins.str],
    ) -> Descriptor: ...
    def add_annotation(self, key: builtins.str, value: builtins.str) -> None: ...
    def build(self) -> ArtifactArchive: ...

class ArtifactDir:
    image_name: typing.Optional[builtins.str]
    annotations: builtins.dict[builtins.str, builtins.str]
    layers: builtins.list[Descriptor]
    @staticmethod
    def from_image_name(image_name: builtins.str) -> ArtifactDir: ...
    @staticmethod
    def from_oci_dir(
        path: builtins.str | os.PathLike | pathlib.Path,
    ) -> ArtifactDir: ...
    def get_blob(self, digest: builtins.str) -> bytes: ...
    def push(self) -> None: ...

class ArtifactDirBuilder:
    @staticmethod
    def new(image_name: builtins.str) -> ArtifactDirBuilder: ...
    @staticmethod
    def for_github(
        org: builtins.str, repo: builtins.str, name: builtins.str, tag: builtins.str
    ) -> ArtifactDirBuilder: ...
    def add_layer(
        self,
        media_type: builtins.str,
        blob: bytes,
        annotations: typing.Mapping[builtins.str, builtins.str],
    ) -> Descriptor: ...
    def add_annotation(self, key: builtins.str, value: builtins.str) -> None: ...
    def build(self) -> ArtifactDir: ...

class Bound:
    r"""
    Variable bound wrapper for Python

    Note: This struct is named `VariableBound` in Rust code to avoid conflicts with PyO3's `Bound` type,
    but is exposed as `Bound` in Python through the `#[pyclass(name = "Bound")]` attribute.
    """

    lower: builtins.float
    upper: builtins.float
    def __new__(cls, lower: builtins.float, upper: builtins.float) -> Bound: ...
    @staticmethod
    def unbounded() -> Bound: ...
    @staticmethod
    def positive() -> Bound: ...
    @staticmethod
    def negative() -> Bound: ...
    @staticmethod
    def of_binary() -> Bound: ...
    def width(self) -> builtins.float: ...
    def is_finite(self) -> builtins.bool: ...
    def contains(
        self, value: builtins.float, atol: builtins.float
    ) -> builtins.bool: ...
    def nearest_to_zero(self) -> builtins.float: ...
    def intersection(self, other: Bound) -> typing.Optional[Bound]: ...
    def __repr__(self) -> builtins.str: ...
    def __copy__(self) -> Bound: ...
    def __deepcopy__(self, _memo: typing.Any) -> Bound: ...

class Constraint:
    r"""
    Constraint wrapper for Python
    """

    id: builtins.int
    function: Function
    equality: Equality
    name: builtins.str
    subscripts: builtins.list[builtins.int]
    description: builtins.str
    parameters: builtins.dict[builtins.str, builtins.str]
    def __new__(
        cls,
        id: builtins.int,
        function: Function,
        equality: Equality,
        name: typing.Optional[builtins.str] = None,
        subscripts: typing.Sequence[builtins.int] = [],
        description: typing.Optional[builtins.str] = None,
        parameters: typing.Mapping[builtins.str, builtins.str] = {},
    ) -> Constraint: ...
    @staticmethod
    def from_bytes(bytes: bytes) -> Constraint: ...
    def to_bytes(self) -> bytes: ...
    def evaluate(self, state: bytes) -> bytes: ...
    def partial_evaluate(self, state: bytes) -> bytes: ...
    def set_name(self, name: builtins.str) -> None:
        r"""
        Set the name of the constraint
        """
    def set_subscripts(self, subscripts: typing.Sequence[builtins.int]) -> None:
        r"""
        Set the subscripts of the constraint
        """
    def add_subscripts(self, subscripts: typing.Sequence[builtins.int]) -> None:
        r"""
        Add subscripts to the constraint
        """
    def set_id(self, id: builtins.int) -> None:
        r"""
        Set the ID of the constraint
        """
    def set_description(self, description: builtins.str) -> None:
        r"""
        Set the description of the constraint
        """
    def set_parameters(
        self, parameters: typing.Mapping[builtins.str, builtins.str]
    ) -> None:
        r"""
        Set the parameters of the constraint
        """
    def add_parameter(self, key: builtins.str, value: builtins.str) -> None:
        r"""
        Add a parameter to the constraint
        """
    def __repr__(self) -> builtins.str: ...
    def __copy__(self) -> Constraint: ...
    def __deepcopy__(self, _memo: typing.Any) -> Constraint: ...

class ConstraintHints:
    r"""
    ConstraintHints wrapper for Python
    """

    one_hot_constraints: builtins.list[OneHot]
    sos1_constraints: builtins.list[Sos1]
    def __new__(
        cls,
        one_hot_constraints: typing.Sequence[OneHot] = [],
        sos1_constraints: typing.Sequence[Sos1] = [],
    ) -> ConstraintHints: ...
    def __repr__(self) -> builtins.str: ...
    def __copy__(self) -> ConstraintHints: ...
    def __deepcopy__(self, _memo: typing.Any) -> ConstraintHints: ...

class DecisionVariable:
    r"""
    DecisionVariable wrapper for Python
    """

    id: builtins.int
    kind: builtins.int
    bound: Bound
    name: builtins.str
    subscripts: builtins.list[builtins.int]
    parameters: builtins.dict[builtins.str, builtins.str]
    description: builtins.str
    substituted_value: typing.Optional[builtins.float]
    def __new__(
        cls,
        id: builtins.int,
        kind: builtins.int,
        bound: Bound,
        name: typing.Optional[builtins.str] = None,
        subscripts: typing.Sequence[builtins.int] = [],
        parameters: typing.Mapping[builtins.str, builtins.str] = {},
        description: typing.Optional[builtins.str] = None,
    ) -> DecisionVariable: ...
    @staticmethod
    def binary(
        id: builtins.int,
        name: typing.Optional[builtins.str] = None,
        subscripts: typing.Sequence[builtins.int] = [],
        parameters: typing.Mapping[builtins.str, builtins.str] = {},
        description: typing.Optional[builtins.str] = None,
    ) -> DecisionVariable: ...
    @staticmethod
    def integer(
        id: builtins.int,
        bound: Bound,
        name: typing.Optional[builtins.str] = None,
        subscripts: typing.Sequence[builtins.int] = [],
        parameters: typing.Mapping[builtins.str, builtins.str] = {},
        description: typing.Optional[builtins.str] = None,
    ) -> DecisionVariable: ...
    @staticmethod
    def continuous(
        id: builtins.int,
        bound: Bound,
        name: typing.Optional[builtins.str] = None,
        subscripts: typing.Sequence[builtins.int] = [],
        parameters: typing.Mapping[builtins.str, builtins.str] = {},
        description: typing.Optional[builtins.str] = None,
    ) -> DecisionVariable: ...
    @staticmethod
    def semi_integer(
        id: builtins.int,
        bound: Bound,
        name: typing.Optional[builtins.str] = None,
        subscripts: typing.Sequence[builtins.int] = [],
        parameters: typing.Mapping[builtins.str, builtins.str] = {},
        description: typing.Optional[builtins.str] = None,
    ) -> DecisionVariable: ...
    @staticmethod
    def semi_continuous(
        id: builtins.int,
        bound: Bound,
        name: typing.Optional[builtins.str] = None,
        subscripts: typing.Sequence[builtins.int] = [],
        parameters: typing.Mapping[builtins.str, builtins.str] = {},
        description: typing.Optional[builtins.str] = None,
    ) -> DecisionVariable: ...
    @staticmethod
    def from_bytes(bytes: bytes) -> DecisionVariable: ...
    def to_bytes(self) -> bytes: ...
    def __repr__(self) -> builtins.str: ...
    def __copy__(self) -> DecisionVariable: ...
    def __deepcopy__(self, _memo: typing.Any) -> DecisionVariable: ...

class DecisionVariableAnalysis:
    def used_binary(self) -> builtins.dict[builtins.int, Bound]: ...
    def used_integer(self) -> builtins.dict[builtins.int, Bound]: ...
    def used_continuous(self) -> builtins.dict[builtins.int, Bound]: ...
    def used_semi_integer(self) -> builtins.dict[builtins.int, Bound]: ...
    def used_semi_continuous(self) -> builtins.dict[builtins.int, Bound]: ...
    def used_decision_variable_ids(self) -> builtins.set[builtins.int]: ...
    def all_decision_variable_ids(self) -> builtins.set[builtins.int]: ...
    def used_in_objective(self) -> builtins.set[builtins.int]: ...
    def used_in_constraints(
        self,
    ) -> builtins.dict[builtins.int, builtins.set[builtins.int]]: ...
    def fixed(self) -> builtins.dict[builtins.int, builtins.float]: ...
    def irrelevant(self) -> builtins.set[builtins.int]: ...
    def dependent(self) -> builtins.set[builtins.int]: ...

class Descriptor:
    r"""
    Descriptor of blob in artifact
    """

    digest: builtins.str
    size: builtins.int
    media_type: builtins.str
    annotations: builtins.dict[builtins.str, builtins.str]
    user_annotations: builtins.dict[builtins.str, builtins.str]
    r"""
    Return annotations with key prefix "org.ommx.user."
    """
    def to_dict(self) -> dict: ...
    @staticmethod
    def from_dict(dict: dict) -> Descriptor: ...
    def to_json(self) -> builtins.str: ...
    @staticmethod
    def from_json(json: builtins.str) -> Descriptor: ...
    def __str__(self) -> builtins.str: ...
    def __eq__(self, rhs: typing.Any) -> builtins.bool: ...

class EvaluatedConstraint:
    id: builtins.int
    r"""
    Get the constraint ID
    """
    equality: Equality
    r"""
    Get the constraint equality type
    """
    evaluated_value: builtins.float
    r"""
    Get the evaluated constraint value
    """
    dual_variable: typing.Optional[builtins.float]
    r"""
    Get the dual variable value
    """
    feasible: builtins.bool
    r"""
    Get the feasibility status
    """
    removed_reason: typing.Optional[builtins.str]
    r"""
    Get the removal reason
    """
    name: typing.Optional[builtins.str]
    r"""
    Get the constraint name
    """
    subscripts: builtins.list[builtins.int]
    r"""
    Get the subscripts
    """
    parameters: builtins.dict[builtins.str, builtins.str]
    r"""
    Get the parameters
    """
    description: typing.Optional[builtins.str]
    r"""
    Get the description
    """
    used_decision_variable_ids: builtins.set[builtins.int]
    r"""
    Get the used decision variable IDs
    """
    @staticmethod
    def from_bytes(bytes: bytes) -> EvaluatedConstraint: ...
    def to_bytes(self) -> bytes: ...
    def set_dual_variable(self, value: typing.Optional[builtins.float]) -> None:
        r"""
        Set the dual variable value
        """

class EvaluatedDecisionVariable:
    id: builtins.int
    r"""
    Get the variable ID
    """
    kind: Kind
    r"""
    Get the variable kind
    """
    value: builtins.float
    r"""
    Get the evaluated value
    """
    lower_bound: builtins.float
    r"""
    Get the lower bound
    """
    upper_bound: builtins.float
    r"""
    Get the upper bound
    """
    name: typing.Optional[builtins.str]
    r"""
    Get the variable name
    """
    subscripts: builtins.list[builtins.int]
    r"""
    Get the subscripts
    """
    parameters: builtins.dict[builtins.str, builtins.str]
    r"""
    Get the parameters
    """
    description: typing.Optional[builtins.str]
    r"""
    Get the description
    """
    @staticmethod
    def from_bytes(bytes: bytes) -> EvaluatedDecisionVariable: ...
    def to_bytes(self) -> bytes: ...

class Function:
    linear_terms: builtins.dict[builtins.int, builtins.float]
    r"""
    Get linear terms as a dictionary mapping variable id to coefficient.
    
    Returns dictionary mapping variable IDs to their linear coefficients.
    Returns empty dict if function has no linear terms.
    Works for all polynomial functions by filtering only degree-1 terms.
    """
    quadratic_terms: builtins.dict[tuple[builtins.int, builtins.int], builtins.float]
    r"""
    Get quadratic terms as a dictionary mapping (row, col) to coefficient.
    
    Returns dictionary mapping variable ID pairs to their quadratic coefficients.
    Returns empty dict if function has no quadratic terms.
    Works for all polynomial functions by filtering only degree-2 terms.
    """
    constant_term: builtins.float
    r"""
    Get the constant term of the function.
    
    Returns the constant term. Returns 0.0 if function has no constant term.
    Works for all polynomial functions by filtering the degree-0 term.
    """
    type_name: builtins.str
    @staticmethod
    def from_scalar(scalar: builtins.float) -> Function: ...
    @staticmethod
    def from_linear(linear: Linear) -> Function: ...
    @staticmethod
    def from_quadratic(quadratic: Quadratic) -> Function: ...
    @staticmethod
    def from_polynomial(polynomial: Polynomial) -> Function: ...
    @staticmethod
    def from_bytes(bytes: bytes) -> Function: ...
    def to_bytes(self) -> bytes: ...
    def as_linear(self) -> typing.Optional[Linear]:
        r"""
        Try to convert this function to a linear function.

        Returns Some(Linear) if the function can be represented as linear,
        None otherwise. This is useful for checking if a function is suitable
        for linear programming solvers.
        """
    def as_quadratic(self) -> typing.Optional[Quadratic]:
        r"""
        Try to convert this function to a quadratic function.

        Returns Some(Quadratic) if the function can be represented as quadratic,
        None otherwise.
        """
    def degree(self) -> builtins.int:
        r"""
        Get the degree of this function.

        Returns the highest degree of any term in the function.
        Zero function has degree 0, constant function has degree 0,
        linear function has degree 1, quadratic function has degree 2, etc.
        """
    def num_terms(self) -> builtins.int:
        r"""
        Get the number of terms in this function.

        Zero function has 0 terms, constant function has 1 term,
        and polynomial functions have the number of non-zero coefficient terms.
        """
    def almost_equal(
        self, other: Function, atol: builtins.float = 1e-06
    ) -> builtins.bool: ...
    def __repr__(self) -> builtins.str: ...
    def __add__(self, rhs: Function) -> Function: ...
    def __sub__(self, rhs: Function) -> Function: ...
    def add_assign(self, rhs: Function) -> None: ...
    def __mul__(self, rhs: Function) -> Function: ...
    def add_scalar(self, scalar: builtins.float) -> Function: ...
    def add_linear(self, linear: Linear) -> Function: ...
    def add_quadratic(self, quadratic: Quadratic) -> Function: ...
    def add_polynomial(self, polynomial: Polynomial) -> Function: ...
    def mul_scalar(self, scalar: builtins.float) -> Function: ...
    def mul_linear(self, linear: Linear) -> Function: ...
    def mul_quadratic(self, quadratic: Quadratic) -> Function: ...
    def mul_polynomial(self, polynomial: Polynomial) -> Function: ...
    def content_factor(self) -> builtins.float: ...
    def required_ids(self) -> builtins.set[builtins.int]: ...
    def terms(self) -> dict: ...
    @staticmethod
    def random(
        rng: Rng,
        num_terms: builtins.int = 5,
        max_degree: builtins.int = 3,
        max_id: builtins.int = 10,
    ) -> Function: ...
    def evaluate(self, state: bytes) -> builtins.float: ...
    def partial_evaluate(self, state: bytes) -> Function: ...
    def __copy__(self) -> Function: ...
    def __deepcopy__(self, _memo: typing.Any) -> Function: ...
    def reduce_binary_power(
        self, binary_ids: builtins.set[builtins.int]
    ) -> builtins.bool:
        r"""
        Reduce binary powers in the function.

        For binary variables, x^n = x for any n >= 1, so we can reduce higher powers to linear terms.

        Args:
            binary_ids: Set of binary variable IDs to reduce powers for

        Returns:
            True if any reduction was performed, False otherwise
        """

class Instance:
    sense: Sense
    objective: Function
    decision_variables: builtins.list[DecisionVariable]
    r"""
    List of all decision variables in the instance sorted by their IDs.
    """
    constraints: builtins.list[Constraint]
    r"""
    List of all decision variables in the instance sorted by their IDs.
    """
    removed_constraints: builtins.list[RemovedConstraint]
    r"""
    List of all removed constraints in the instance sorted by their IDs.
    """
    description: typing.Optional[InstanceDescription]
    constraint_hints: ConstraintHints
    used_decision_variables: builtins.list[DecisionVariable]
    @staticmethod
    def from_bytes(bytes: bytes) -> Instance: ...
    @staticmethod
    def from_components(
        sense: Sense,
        objective: Function,
        decision_variables: typing.Mapping[builtins.int, DecisionVariable],
        constraints: typing.Mapping[builtins.int, Constraint],
        description: typing.Optional[InstanceDescription] = None,
        constraint_hints: typing.Optional[ConstraintHints] = None,
    ) -> Instance: ...
    def set_objective(self, objective: Function) -> None: ...
    def to_bytes(self) -> bytes: ...
    def required_ids(self) -> builtins.set[builtins.int]: ...
    def as_qubo_format(self) -> tuple[dict, builtins.float]: ...
    def as_hubo_format(self) -> tuple[dict, builtins.float]: ...
    def as_parametric_instance(self) -> ParametricInstance: ...
    def penalty_method(self) -> ParametricInstance: ...
    def uniform_penalty_method(self) -> ParametricInstance: ...
    def evaluate(self, state: bytes) -> Solution: ...
    def partial_evaluate(self, state: bytes) -> bytes: ...
    def evaluate_samples(self, samples: Samples) -> SampleSet: ...
    def random_state(self, rng: Rng) -> State: ...
    def random_samples(
        self,
        rng: Rng,
        *,
        num_different_samples: builtins.int = 5,
        num_samples: builtins.int = 10,
        max_sample_id: typing.Optional[builtins.int] = None,
    ) -> Samples: ...
    def relax_constraint(
        self,
        constraint_id: builtins.int,
        removed_reason: builtins.str,
        removed_reason_parameters: typing.Mapping[builtins.str, builtins.str],
    ) -> None: ...
    def restore_constraint(self, constraint_id: builtins.int) -> None: ...
    def log_encode(self, integer_variable_ids: builtins.set[builtins.int]) -> None: ...
    def convert_inequality_to_equality_with_integer_slack(
        self, constraint_id: builtins.int, max_integer_range: builtins.int
    ) -> None: ...
    def add_integer_slack_to_inequality(
        self, constraint_id: builtins.int, slack_upper_bound: builtins.int
    ) -> typing.Optional[builtins.float]: ...
    def decision_variable_analysis(self) -> DecisionVariableAnalysis: ...
    def __copy__(self) -> Instance: ...
    def __deepcopy__(self, _memo: typing.Any) -> Instance: ...
    def as_minimization_problem(self) -> builtins.bool: ...
    def as_maximization_problem(self) -> builtins.bool: ...
    def get_decision_variable_by_id(
        self, variable_id: builtins.int
    ) -> DecisionVariable:
        r"""
        Get a specific decision variable by ID
        """
    def get_constraint_by_id(self, constraint_id: builtins.int) -> Constraint:
        r"""
        Get a specific constraint by ID
        """
    def get_removed_constraint_by_id(
        self, constraint_id: builtins.int
    ) -> RemovedConstraint:
        r"""
        Get a specific removed constraint by ID
        """
    def reduce_binary_power(self) -> builtins.bool:
        r"""
        Reduce binary powers in the instance.

        This method replaces binary powers in the instance with their equivalent linear expressions.
        For binary variables, x^n = x for any n >= 1, so we can reduce higher powers to linear terms.

        Returns `True` if any reduction was performed, `False` otherwise.
        """
    @staticmethod
    def load_mps(path: builtins.str) -> Instance: ...
    def save_mps(self, path: builtins.str, compress: builtins.bool = True) -> None: ...
    @staticmethod
    def load_qplib(path: builtins.str) -> Instance: ...

class InstanceDescription:
    name: typing.Optional[builtins.str]
    description: typing.Optional[builtins.str]
    authors: builtins.list[builtins.str]
    created_by: typing.Optional[builtins.str]
    def __new__(
        cls,
        name: typing.Optional[builtins.str] = None,
        description: typing.Optional[builtins.str] = None,
        authors: typing.Optional[typing.Sequence[builtins.str]] = None,
        created_by: typing.Optional[builtins.str] = None,
    ) -> InstanceDescription: ...
    def __repr__(self) -> builtins.str: ...
    def __copy__(self) -> InstanceDescription: ...
    def __deepcopy__(self, _memo: typing.Any) -> InstanceDescription: ...

class Linear:
    linear_terms: builtins.dict[builtins.int, builtins.float]
    constant_term: builtins.float
    def __new__(
        cls,
        terms: typing.Mapping[builtins.int, builtins.float],
        constant: builtins.float = 0.0,
    ) -> Linear: ...
    @staticmethod
    def single_term(id: builtins.int, coefficient: builtins.float) -> Linear: ...
    @staticmethod
    def constant(constant: builtins.float) -> Linear: ...
    @staticmethod
    def random(
        rng: Rng, num_terms: builtins.int = 3, max_id: builtins.int = 10
    ) -> Linear: ...
    @staticmethod
    def from_bytes(bytes: bytes) -> Linear: ...
    def to_bytes(self) -> bytes: ...
    def almost_equal(
        self, other: Linear, atol: builtins.float = 1e-06
    ) -> builtins.bool: ...
    def __repr__(self) -> builtins.str: ...
    def __add__(self, rhs: Linear) -> Linear: ...
    def __sub__(self, rhs: Linear) -> Linear: ...
    def __mul__(self, rhs: Linear) -> Quadratic: ...
    def add_assign(self, rhs: Linear) -> None: ...
    def add_scalar(self, scalar: builtins.float) -> Linear: ...
    def mul_scalar(self, scalar: builtins.float) -> Linear: ...
    def terms(self) -> dict: ...
    def evaluate(self, state: bytes) -> builtins.float: ...
    def partial_evaluate(self, state: bytes) -> Linear: ...
    def __copy__(self) -> Linear: ...
    def __deepcopy__(self, _memo: typing.Any) -> Linear: ...

class OneHot:
    r"""
    OneHot constraint hint wrapper for Python
    """

    id: builtins.int
    variables: builtins.list[builtins.int]
    def __new__(
        cls, id: builtins.int, variables: typing.Sequence[builtins.int]
    ) -> OneHot: ...
    def __repr__(self) -> builtins.str: ...
    def __copy__(self) -> OneHot: ...
    def __deepcopy__(self, _memo: typing.Any) -> OneHot: ...

class Parameters:
    @staticmethod
    def from_bytes(bytes: bytes) -> Parameters: ...
    def to_bytes(self) -> bytes: ...

class ParametricInstance:
    @staticmethod
    def from_bytes(bytes: bytes) -> ParametricInstance: ...
    def to_bytes(self) -> bytes: ...
    def with_parameters(self, parameters: Parameters) -> Instance: ...

class Polynomial:
    def __new__(
        cls, terms: typing.Mapping[typing.Sequence[builtins.int], builtins.float]
    ) -> Polynomial: ...
    @staticmethod
    def from_bytes(bytes: bytes) -> Polynomial: ...
    def to_bytes(self) -> bytes: ...
    def almost_equal(
        self, other: Polynomial, atol: builtins.float = 1e-06
    ) -> builtins.bool: ...
    def __repr__(self) -> builtins.str: ...
    def __add__(self, rhs: Polynomial) -> Polynomial: ...
    def __sub__(self, rhs: Polynomial) -> Polynomial: ...
    def add_assign(self, rhs: Polynomial) -> None: ...
    def __mul__(self, rhs: Polynomial) -> Polynomial: ...
    def add_scalar(self, scalar: builtins.float) -> Polynomial: ...
    def add_linear(self, linear: Linear) -> Polynomial: ...
    def add_quadratic(self, quadratic: Quadratic) -> Polynomial: ...
    def mul_scalar(self, scalar: builtins.float) -> Polynomial: ...
    def mul_linear(self, linear: Linear) -> Polynomial: ...
    def mul_quadratic(self, quadratic: Quadratic) -> Polynomial: ...
    def terms(self) -> dict: ...
    @staticmethod
    def random(
        rng: Rng,
        num_terms: builtins.int = 5,
        max_degree: builtins.int = 3,
        max_id: builtins.int = 10,
    ) -> Polynomial: ...
    def evaluate(self, state: bytes) -> builtins.float: ...
    def partial_evaluate(self, state: bytes) -> Polynomial: ...
    def __copy__(self) -> Polynomial: ...
    def __deepcopy__(self, _memo: typing.Any) -> Polynomial: ...

class Quadratic:
    linear_terms: builtins.dict[builtins.int, builtins.float]
    constant_term: builtins.float
    quadratic_terms: builtins.dict[tuple[builtins.int, builtins.int], builtins.float]
    def __new__(
        cls,
        columns: typing.Sequence[builtins.int],
        rows: typing.Sequence[builtins.int],
        values: typing.Sequence[builtins.float],
        linear: typing.Optional[Linear] = None,
    ) -> Quadratic: ...
    @staticmethod
    def from_bytes(bytes: bytes) -> Quadratic: ...
    def to_bytes(self) -> bytes: ...
    def almost_equal(
        self, other: Quadratic, atol: builtins.float = 1e-06
    ) -> builtins.bool: ...
    def __repr__(self) -> builtins.str: ...
    def __add__(self, rhs: Quadratic) -> Quadratic: ...
    def __sub__(self, rhs: Quadratic) -> Quadratic: ...
    def add_assign(self, rhs: Quadratic) -> None: ...
    def __mul__(self, rhs: Quadratic) -> Polynomial: ...
    def add_scalar(self, scalar: builtins.float) -> Quadratic: ...
    def add_linear(self, linear: Linear) -> Quadratic: ...
    def mul_scalar(self, scalar: builtins.float) -> Quadratic: ...
    def mul_linear(self, linear: Linear) -> Polynomial: ...
    def terms(self) -> dict: ...
    @staticmethod
    def random(
        rng: Rng, num_terms: builtins.int = 5, max_id: builtins.int = 10
    ) -> Quadratic: ...
    def evaluate(self, state: bytes) -> builtins.float: ...
    def partial_evaluate(self, state: bytes) -> Quadratic: ...
    def __copy__(self) -> Quadratic: ...
    def __deepcopy__(self, _memo: typing.Any) -> Quadratic: ...

class RemovedConstraint:
    r"""
    RemovedConstraint wrapper for Python
    """

    constraint: Constraint
    removed_reason: builtins.str
    removed_reason_parameters: builtins.dict[builtins.str, builtins.str]
    id: builtins.int
    name: builtins.str
    def __new__(
        cls,
        constraint: Constraint,
        removed_reason: builtins.str,
        removed_reason_parameters: typing.Optional[
            typing.Mapping[builtins.str, builtins.str]
        ] = None,
    ) -> RemovedConstraint: ...
    @staticmethod
    def from_bytes(bytes: bytes) -> RemovedConstraint: ...
    def to_bytes(self) -> bytes: ...
    def __repr__(self) -> builtins.str: ...
    def __copy__(self) -> RemovedConstraint: ...
    def __deepcopy__(self, _memo: typing.Any) -> RemovedConstraint: ...

class Rng:
    def __new__(cls) -> Rng:
        r"""
        Create a new random number generator with a deterministic seed.
        """

class SampleSet:
    best_feasible_id: builtins.int
    best_feasible_relaxed_id: builtins.int
    best_feasible: Solution
    best_feasible_relaxed: Solution
    best_feasible_unrelaxed: Solution
    objectives: builtins.dict[builtins.int, builtins.float]
    r"""
    Get objectives for all samples
    """
    feasible: builtins.dict[builtins.int, builtins.bool]
    r"""
    Get feasibility status for all samples
    """
    feasible_relaxed: builtins.dict[builtins.int, builtins.bool]
    r"""
    Get relaxed feasibility status for all samples
    """
    feasible_unrelaxed: builtins.dict[builtins.int, builtins.bool]
    r"""
    Get unrelaxed feasibility status for all samples
    """
    sense: Sense
    r"""
    Get the optimization sense (minimize or maximize)
    """
    constraints: builtins.list[SampledConstraint]
    r"""
    Get constraints for compatibility with existing Python code
    """
    decision_variables: builtins.list[SampledDecisionVariable]
    r"""
    Get decision variables for compatibility with existing Python code
    """
    sample_ids_list: builtins.list[builtins.int]
    r"""
    Get sample IDs as a list (property version)
    """
    @staticmethod
    def from_bytes(bytes: bytes) -> SampleSet: ...
    def to_bytes(self) -> bytes: ...
    def get(self, sample_id: builtins.int) -> Solution: ...
    def get_sample_by_id(self, sample_id: builtins.int) -> Solution:
        r"""
        Get sample by ID (alias for get method)
        """
    def num_samples(self) -> builtins.int: ...
    def sample_ids(self) -> builtins.set[builtins.int]: ...
    def feasible_ids(self) -> builtins.set[builtins.int]: ...
    def feasible_relaxed_ids(self) -> builtins.set[builtins.int]: ...
    def feasible_unrelaxed_ids(self) -> builtins.set[builtins.int]: ...
    def extract_decision_variables(
        self, name: builtins.str, sample_id: builtins.int
    ) -> dict:
        r"""
        Extract decision variable values for a given name and sample ID
        """
    def extract_constraints(self, name: builtins.str, sample_id: builtins.int) -> dict:
        r"""
        Extract constraint values for a given name and sample ID
        """
    def get_decision_variable_by_id(
        self, variable_id: builtins.int
    ) -> SampledDecisionVariable:
        r"""
        Get a specific sampled decision variable by ID
        """
    def get_constraint_by_id(self, constraint_id: builtins.int) -> SampledConstraint:
        r"""
        Get a specific sampled constraint by ID
        """

class SampledConstraint:
    id: builtins.int
    r"""
    Get the constraint ID
    """
    equality: Equality
    r"""
    Get the constraint equality type
    """
    name: typing.Optional[builtins.str]
    r"""
    Get the constraint name
    """
    subscripts: builtins.list[builtins.int]
    r"""
    Get the subscripts
    """
    description: typing.Optional[builtins.str]
    r"""
    Get the description
    """
    removed_reason: typing.Optional[builtins.str]
    r"""
    Get the removal reason
    """
    removed_reason_parameters: builtins.dict[builtins.str, builtins.str]
    r"""
    Get the removal reason parameters
    """
    used_decision_variable_ids: builtins.set[builtins.int]
    r"""
    Get the used decision variable IDs
    """
    evaluated_values: builtins.dict[builtins.int, builtins.float]
    r"""
    Get the evaluated values for all samples
    """
    feasible: builtins.dict[builtins.int, builtins.bool]
    r"""
    Get the feasibility status for all samples
    """
    @staticmethod
    def from_bytes(bytes: bytes) -> SampledConstraint: ...
    def to_bytes(self) -> bytes: ...

class SampledDecisionVariable:
    id: builtins.int
    r"""
    Get the decision variable ID
    """
    kind: Kind
    r"""
    Get the decision variable kind
    """
    bound: Bound
    r"""
    Get the decision variable bound
    """
    name: typing.Optional[builtins.str]
    r"""
    Get the decision variable name
    """
    subscripts: builtins.list[builtins.int]
    r"""
    Get the subscripts
    """
    description: typing.Optional[builtins.str]
    r"""
    Get the description
    """
    parameters: builtins.dict[builtins.str, builtins.str]
    r"""
    Get the parameters
    """
    samples: builtins.dict[builtins.int, builtins.float]
    r"""
    Get the sampled values for all samples
    """
    @staticmethod
    def from_bytes(bytes: bytes) -> SampledDecisionVariable: ...
    def to_bytes(self) -> bytes: ...

class Samples:
    def __new__(cls, entries: typing.Any) -> Samples: ...
    @staticmethod
    def from_bytes(bytes: bytes) -> Samples: ...
    def to_bytes(self) -> bytes: ...
    def num_samples(self) -> builtins.int:
        r"""
        Get the number of samples
        """
    def sample_ids(self) -> builtins.set[builtins.int]:
        r"""
        Get all sample IDs
        """
    def get_state(self, sample_id: builtins.int) -> State:
        r"""
        Get the state for a specific sample ID
        """
    def append(self, sample_ids: typing.Sequence[builtins.int], state: State) -> None:
        r"""
        Append a sample with the given sample IDs and state
        """

class Solution:
    objective: builtins.float
    r"""
    Get the objective function value
    """
    state: State
    r"""
    Get the solution state containing variable values
    """
    feasible: builtins.bool
    r"""
    Check if the solution is feasible
    """
    feasible_relaxed: builtins.bool
    r"""
    Check if the solution is feasible in the relaxed problem
    """
    feasible_unrelaxed: builtins.bool
    r"""
    Check if the solution is feasible in the unrelaxed problem
    """
    sense: Sense
    optimality: Optimality
    r"""
    Get the optimality status
    """
    relaxation: Relaxation
    r"""
    Get the relaxation status
    """
    decision_variables: builtins.list[EvaluatedDecisionVariable]
    r"""
    Get evaluated decision variables as a list sorted by ID
    """
    constraints: builtins.list[EvaluatedConstraint]
    r"""
    Get evaluated constraints as a list sorted by ID
    """
    decision_variable_ids: builtins.set[builtins.int]
    constraint_ids: builtins.set[builtins.int]
    @staticmethod
    def from_bytes(bytes: bytes) -> Solution: ...
    def to_bytes(self) -> bytes: ...
    def set_optimality(self, optimality: Optimality) -> None:
        r"""
        Set the optimality status
        """
    def set_relaxation(self, relaxation: Relaxation) -> None:
        r"""
        Set the relaxation status
        """
    def extract_decision_variables(self, name: builtins.str) -> dict:
        r"""
        Extract decision variables by name with subscripts as key (returns a Python dict)
        """
    def extract_constraints(self, name: builtins.str) -> dict:
        r"""
        Extract constraints by name with subscripts as key (returns a Python dict)
        """
    def set_dual_variable(
        self, constraint_id: builtins.int, value: typing.Optional[builtins.float]
    ) -> None:
        r"""
        Set the dual variable value for a specific constraint by ID
        """
    def get_decision_variable_by_id(
        self, variable_id: builtins.int
    ) -> EvaluatedDecisionVariable:
        r"""
        Get a specific evaluated decision variable by ID
        """
    def get_constraint_by_id(self, constraint_id: builtins.int) -> EvaluatedConstraint:
        r"""
        Get a specific evaluated constraint by ID
        """

class Sos1:
    r"""
    SOS1 constraint hint wrapper for Python
    """

    binary_constraint_id: builtins.int
    big_m_constraint_ids: builtins.list[builtins.int]
    variables: builtins.list[builtins.int]
    def __new__(
        cls,
        binary_constraint_id: builtins.int,
        big_m_constraint_ids: typing.Sequence[builtins.int],
        variables: typing.Sequence[builtins.int],
    ) -> Sos1: ...
    def __repr__(self) -> builtins.str: ...
    def __copy__(self) -> Sos1: ...
    def __deepcopy__(self, _memo: typing.Any) -> Sos1: ...

class State:
    r"""
    State wrapper for Python
    """

    entries: builtins.dict[builtins.int, builtins.float]
    def __new__(cls, entries: typing.Any) -> State: ...
    @staticmethod
    def from_bytes(bytes: bytes) -> State: ...
    def to_bytes(self) -> bytes: ...
    def set_entries(
        self, entries: typing.Mapping[builtins.int, builtins.float]
    ) -> None: ...
    def get(self, key: builtins.int) -> typing.Optional[builtins.float]: ...
    def set(self, key: builtins.int, value: builtins.float) -> None: ...
    def __len__(self) -> builtins.int: ...
    def __contains__(self, key: builtins.int) -> builtins.bool: ...
    def keys(self) -> builtins.list[builtins.int]: ...
    def values(self) -> builtins.list[builtins.float]: ...
    def items(self) -> builtins.list[tuple[builtins.int, builtins.float]]: ...
    def __repr__(self) -> builtins.str: ...
    def __copy__(self) -> State: ...
    def __deepcopy__(self, _memo: typing.Any) -> State: ...

class Equality(Enum):
    r"""
    Equality type for constraints
    """

    EqualToZero = ...
    LessThanOrEqualToZero = ...

    @staticmethod
    def from_pb(value: builtins.int) -> Equality:
        r"""
        Convert from Protocol Buffer equality value
        """

    def to_pb(self) -> builtins.int:
        r"""
        Convert to Protocol Buffer equality value
        """

    def __repr__(self) -> builtins.str: ...
    def __str__(self) -> builtins.str: ...

class Kind(Enum):
    r"""
    Kind of decision variable
    """

    Binary = ...
    Integer = ...
    Continuous = ...
    SemiInteger = ...
    SemiContinuous = ...

    @staticmethod
    def from_pb(value: builtins.int) -> Kind:
        r"""
        Convert from Protocol Buffer kind value
        """

    def to_pb(self) -> builtins.int:
        r"""
        Convert to Protocol Buffer kind value
        """

    def __repr__(self) -> builtins.str: ...
    def __str__(self) -> builtins.str: ...

class Optimality(Enum):
    r"""
    Optimality status of a solution
    """

    Unspecified = ...
    Optimal = ...
    NotOptimal = ...

    @staticmethod
    def from_pb(value: builtins.int) -> Optimality:
        r"""
        Convert from Protocol Buffer optimality value
        """

    def to_pb(self) -> builtins.int:
        r"""
        Convert to Protocol Buffer optimality value
        """

    def __repr__(self) -> builtins.str: ...
    def __str__(self) -> builtins.str: ...

class Relaxation(Enum):
    r"""
    Relaxation status of a solution
    """

    Unspecified = ...
    LpRelaxed = ...

    @staticmethod
    def from_pb(value: builtins.int) -> Relaxation:
        r"""
        Convert from Protocol Buffer relaxation value
        """

    def to_pb(self) -> builtins.int:
        r"""
        Convert to Protocol Buffer relaxation value
        """

    def __repr__(self) -> builtins.str: ...
    def __str__(self) -> builtins.str: ...

class Sense(Enum):
    r"""
    Sense of optimization (minimize or maximize)
    """

    Minimize = ...
    Maximize = ...

    @staticmethod
    def from_pb(value: builtins.int) -> Sense:
        r"""
        Convert from Protocol Buffer sense value
        """

    def to_pb(self) -> builtins.int:
        r"""
        Convert to Protocol Buffer sense value
        """

    def __repr__(self) -> builtins.str: ...
    def __str__(self) -> builtins.str: ...

def miplib2017_instance_annotations() -> builtins.dict[
    builtins.str, builtins.dict[builtins.str, builtins.str]
]: ...
