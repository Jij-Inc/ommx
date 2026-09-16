"""Static return-type assertions for the downstream bridge consumer."""

import ommx
from ommx_pyo3_bridge_fixture import (
    constraint,
    decision_variable,
    function,
    instance,
    parametric_instance,
    sample_set,
    solution,
)
from typing_extensions import assert_type
import ommx_pyo3_bridge_fixture as fixture


assert_type(function(), ommx.Function)
assert_type(constraint(), ommx.Constraint)
assert_type(decision_variable(), ommx.DecisionVariable)
assert_type(instance(), ommx.Instance)
assert_type(parametric_instance(), ommx.ParametricInstance)
assert_type(solution(), ommx.Solution)
assert_type(sample_set(), ommx.SampleSet)

# Route parameters and different Rust source types never enter Python stubs.
assert_type(fixture.negotiated_instance(), ommx.Instance)
assert_type(fixture.completed_instance(lambda: None), ommx.Instance)
assert_type(fixture.compile_for_target(), ommx.Instance)
assert_type(fixture.v1_first_instance(), ommx.Instance)
assert_type(fixture.invalid_instance(), ommx.Instance)
assert_type(fixture.v1_function(), ommx.Function)
assert_type(fixture.v1_constraint(), ommx.Constraint)
assert_type(fixture.v1_decision_variable(), ommx.DecisionVariable)
assert_type(fixture.negotiated_parametric_instance(), ommx.ParametricInstance)
assert_type(fixture.negotiated_solution(), ommx.Solution)
assert_type(fixture.negotiated_sample_set(), ommx.SampleSet)
assert_type(
    fixture.v2_components(),
    tuple[ommx.Function, ommx.Constraint, ommx.DecisionVariable],
)
