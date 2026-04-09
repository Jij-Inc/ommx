from .. import _ommx_rust

# Enums
Sense = _ommx_rust.Sense
Equality = _ommx_rust.Equality
Kind = _ommx_rust.Kind
Optimality = _ommx_rust.Optimality
Relaxation = _ommx_rust.Relaxation

# Core types
State = _ommx_rust.State
Samples = _ommx_rust.Samples
Bound = _ommx_rust.Bound

# Function types
Linear = _ommx_rust.Linear
Quadratic = _ommx_rust.Quadratic
Polynomial = _ommx_rust.Polynomial
Function = _ommx_rust.Function

# Decision variable and parameter
DecisionVariable = _ommx_rust.DecisionVariable
Parameter = _ommx_rust.Parameter

# Constraint and named function
Constraint = _ommx_rust.Constraint
RemovedConstraint = _ommx_rust.RemovedConstraint
NamedFunction = _ommx_rust.NamedFunction

# Constraint hints
OneHot = _ommx_rust.OneHot
Sos1 = _ommx_rust.Sos1
ConstraintHints = _ommx_rust.ConstraintHints

# Evaluated types
EvaluatedDecisionVariable = _ommx_rust.EvaluatedDecisionVariable
EvaluatedConstraint = _ommx_rust.EvaluatedConstraint
EvaluatedNamedFunction = _ommx_rust.EvaluatedNamedFunction
SampledDecisionVariable = _ommx_rust.SampledDecisionVariable
SampledConstraint = _ommx_rust.SampledConstraint
SampledNamedFunction = _ommx_rust.SampledNamedFunction

# Analysis
DecisionVariableAnalysis = _ommx_rust.DecisionVariableAnalysis

# Top-level types
Instance = _ommx_rust.Instance
ParametricInstance = _ommx_rust.ParametricInstance
Solution = _ommx_rust.Solution
SampleSet = _ommx_rust.SampleSet

# Utility
Rng = _ommx_rust.Rng

# Type aliases
ToState = _ommx_rust.ToState
ToSamples = _ommx_rust.ToSamples

__all__ = [
    # Enums
    "Sense",
    "Equality",
    "Kind",
    "Optimality",
    "Relaxation",
    # Core types
    "State",
    "Samples",
    "Bound",
    # Function types
    "Linear",
    "Quadratic",
    "Polynomial",
    "Function",
    # Decision variable and parameter
    "DecisionVariable",
    "Parameter",
    # Constraint and named function
    "Constraint",
    "RemovedConstraint",
    "NamedFunction",
    # Constraint hints
    "OneHot",
    "Sos1",
    "ConstraintHints",
    # Evaluated types
    "EvaluatedDecisionVariable",
    "EvaluatedConstraint",
    "EvaluatedNamedFunction",
    "SampledDecisionVariable",
    "SampledConstraint",
    "SampledNamedFunction",
    # Analysis
    "DecisionVariableAnalysis",
    # Top-level types
    "Instance",
    "ParametricInstance",
    "Solution",
    "SampleSet",
    # Utility
    "Rng",
    # Type aliases
    "ToState",
    "ToSamples",
]
