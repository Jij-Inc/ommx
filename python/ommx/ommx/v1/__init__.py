from __future__ import annotations
from typing import Optional, Mapping
from typing_extensions import TypeAlias, Union, Sequence
from dataclasses import dataclass

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
EvaluatedNamedFunction = _ommx_rust.EvaluatedNamedFunction
SampledDecisionVariable = _ommx_rust.SampledDecisionVariable
SampledConstraint = _ommx_rust.SampledConstraint
SampledNamedFunction = _ommx_rust.SampledNamedFunction

# Import function types from Rust
Linear = _ommx_rust.Linear
Quadratic = _ommx_rust.Quadratic
Polynomial = _ommx_rust.Polynomial
Function = _ommx_rust.Function

# Import DecisionVariable and Parameter from Rust
DecisionVariable = _ommx_rust.DecisionVariable
Parameter = _ommx_rust.Parameter

# Import Constraint, RemovedConstraint, and NamedFunction from Rust
Constraint = _ommx_rust.Constraint
RemovedConstraint = _ommx_rust.RemovedConstraint
NamedFunction = _ommx_rust.NamedFunction

# Type alias from Rust
ToState = _ommx_rust.ToState

# Core types - full Rust re-exports
Instance = _ommx_rust.Instance
ParametricInstance = _ommx_rust.ParametricInstance
Solution = _ommx_rust.Solution
SampleSet = _ommx_rust.SampleSet


__all__ = [
    "Instance",
    "ParametricInstance",
    "Solution",
    "Constraint",
    "RemovedConstraint",
    "SampleSet",
    # Function and its bases
    "DecisionVariable",
    "Parameter",
    "Linear",
    "Quadratic",
    "Polynomial",
    "Function",
    "NamedFunction",
    # Constraint hints
    "OneHot",
    "Sos1",
    "ConstraintHints",
    # Core types
    "State",
    "Samples",
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
    "EvaluatedNamedFunction",
    "SampledDecisionVariable",
    "SampledConstraint",
    "SampledNamedFunction",
    # Type Alias
    "ToState",
    "ToSamples",
]

ToSamples: TypeAlias = Union[Samples, Mapping[int, ToState], Sequence[ToState]]
"""
Type alias for convertible types to :class:`Samples`.
"""


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
