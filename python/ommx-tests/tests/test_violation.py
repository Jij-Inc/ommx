"""Tests for constraint violation calculation methods."""

from typing import Literal

import pytest
from ommx import (
    DecisionVariable,
    Instance,
    OneHotConstraint,
    Sense,
    SampleSet,
    Solution,
    Sos1Constraint,
)


def test_explicit_feasibility_queries_preserve_evaluation_and_saved_conditions():
    x = DecisionVariable.continuous(0, lower=-1, upper=1)
    constraint = x == 0
    evaluated = constraint.evaluate({0: 0.0625}, atol=0.5)
    assert not evaluated.is_feasible(atol=0.03125)
    assert evaluated.is_feasible(atol=0.125)
    assert evaluated.evaluated_value == evaluated.violation() == 0.0625

    instance = Instance.from_components(
        decision_variables=[x],
        objective=0,
        constraints={1: constraint},
        sense=Sense.Minimize,
    )
    solution = instance.evaluate({0: 0.0625}, atol=0.125)
    wire = solution.to_v2_bytes()
    for result in [solution, Solution.from_v2_bytes(wire)]:
        assert result.feasibility_atol == 0.125
        assert not result.constraints[1].is_feasible(atol=0.03125)
        assert result.constraints[1].is_feasible(atol=result.feasibility_atol)
        assert result.feasible
        assert result.constraints_df().loc[1, "feasible"]
        assert result.to_v2_bytes() == wire

    samples = instance.evaluate_samples({7: {0: 0.0625}, 8: {0: 0.25}}, atol=0.125)
    wire = samples.to_v2_bytes()
    for result in [samples, SampleSet.from_v2_bytes(wire)]:
        assert result.feasibility_atol == 0.125
        sampled_constraint = result.constraints[0]
        assert sampled_constraint.feasible(atol=0.03125) == {7: False, 8: False}
        assert sampled_constraint.feasible(atol=result.feasibility_atol) == {
            7: True,
            8: False,
        }
        assert result.feasible == {7: True, 8: False}
        assert result.get(7).feasibility_atol == result.feasibility_atol
        assert sampled_constraint.evaluated_values == {7: 0.0625, 8: 0.25}
        assert result.to_v2_bytes() == wire


def test_evaluated_constraint_violation_equality():
    """Test violation calculation for equality constraints."""
    # Create instance with equality constraint: x = 2.5 evaluated at x=0
    # This gives f(x) = x - 2.5 = 0 - 2.5 = -2.5, violation = |-2.5| = 2.5
    x = DecisionVariable.continuous(id=1, lower=0, upper=10)
    constraint = x == 2.5

    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={1: constraint},
        sense=Sense.Minimize,
    )

    # Evaluate at x=0, so constraint becomes 0 = 2.5, f(x) = -2.5
    solution = instance.evaluate({1: 0.0})
    evaluated_constraint = solution.constraints[1]

    # For equality constraint f(x) = 0, violation = |f(x)| = |-2.5| = 2.5
    assert evaluated_constraint.violation() == pytest.approx(2.5)


def test_evaluated_constraint_violation_inequality_violated():
    """Test violation calculation for violated inequality constraints."""
    # Create instance with inequality constraint: x <= 1.5 evaluated at x=3
    # This gives f(x) = x - 1.5 = 3 - 1.5 = 1.5, violation = max(0, 1.5) = 1.5
    x = DecisionVariable.continuous(id=1, lower=0, upper=10)
    constraint = x <= 1.5

    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={1: constraint},
        sense=Sense.Minimize,
    )

    # Evaluate at x=3, so constraint becomes 3 <= 1.5, f(x) = 1.5
    solution = instance.evaluate({1: 3.0})
    evaluated_constraint = solution.constraints[1]

    # For inequality constraint f(x) ≤ 0, violation = max(0, f(x)) = max(0, 1.5) = 1.5
    assert evaluated_constraint.violation() == pytest.approx(1.5)


def test_evaluated_constraint_violation_inequality_satisfied():
    """Test violation calculation for satisfied inequality constraints."""
    # Create instance with inequality constraint: x <= 5 evaluated at x=2
    # This gives f(x) = x - 5 = 2 - 5 = -3, violation = max(0, -3) = 0
    x = DecisionVariable.continuous(id=1, lower=0, upper=10)
    constraint = x <= 5.0

    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={1: constraint},
        sense=Sense.Minimize,
    )

    # Evaluate at x=2, so constraint becomes 2 <= 5, f(x) = -3
    solution = instance.evaluate({1: 2.0})
    evaluated_constraint = solution.constraints[1]

    # For inequality constraint f(x) ≤ 0, violation = max(0, f(x)) = max(0, -3) = 0.0
    assert evaluated_constraint.violation() == pytest.approx(0.0)


def test_solution_total_violation():
    """Test total violation calculation."""
    x = DecisionVariable.continuous(id=1, lower=0, upper=10)

    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={
            0: x == 2.5,  # Equality: x = 2.5, evaluated at x=0 gives f(x) = -2.5
            1: x <= 1.5,  # Inequality: x <= 1.5, evaluated at x=3 gives f(x) = 1.5
        },
        sense=Sense.Minimize,
    )

    # Use x=0 for equality (violation=2.5) but that would make inequality satisfied
    # So let's use x=5: equality violation = |5-2.5| = 2.5, inequality violation = max(0, 5-1.5) = 3.5
    solution = instance.evaluate({1: 5.0})

    # L1 = |5-2.5| + max(0, 5-1.5) = 2.5 + 3.5 = 6.0
    expected_l1 = 2.5 + 3.5
    assert solution.total_violation() == pytest.approx(expected_l1)


def test_solution_total_violation_with_satisfied_constraints():
    """Test total violation when some constraints are satisfied."""
    x = DecisionVariable.continuous(id=1, lower=0, upper=10)

    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={
            0: x == 2.0,  # Violated equality: x = 2.0, evaluated at x=5 gives |5-2| = 3
            1: x
            <= 10.0,  # Satisfied inequality: x <= 10, evaluated at x=5 gives max(0, 5-10) = 0
        },
        sense=Sense.Minimize,
    )

    solution = instance.evaluate({1: 5.0})

    # L1 = |5-2.0| + max(0, 5-10) = 3.0 + 0.0 = 3.0
    assert solution.total_violation() == pytest.approx(3.0)


def test_solution_total_violation_empty():
    """Test total violation with no constraints."""
    x = DecisionVariable.integer(id=1, lower=0, upper=10)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Sense.Minimize,
    )

    solution = instance.evaluate({1: 5.0})

    # No constraints means no violations
    assert solution.total_violation() == pytest.approx(0.0)


@pytest.mark.parametrize("active", [False, True])
@pytest.mark.parametrize("equality", [False, True])
@pytest.mark.parametrize("removed", [False, True])
@pytest.mark.parametrize("value", [-2.5, 0.0, 1.5])
def test_total_violation_indicator(active, equality, removed, value):
    z = DecisionVariable.binary(0)
    x = DecisionVariable.continuous(1, lower=-10, upper=10)
    constraint = x == 0 if equality else x <= 0
    instance = Instance.from_components(
        decision_variables=[z, x],
        objective=0,
        constraints={},
        indicator_constraints={0: constraint.with_indicator(z)},
        sense=Sense.Minimize,
    )
    if removed:
        instance.relax_indicator_constraint(0, "test relaxation")
    solution = instance.evaluate({0: float(active), 1: value})
    violation = (abs(value) if equality else max(0, value)) if active else 0
    assert solution.constraint_violation(0, kind="indicator") == pytest.approx(
        violation
    )
    assert solution.total_violation() == pytest.approx(violation)
    assert solution.constraints_df(kind="indicator").loc[
        0, "violation"
    ] == pytest.approx(violation)


@pytest.mark.parametrize(
    "values, violation",
    [
        ([0, 0, 0], 1),
        ([0, 1, 0], 0),
        ([1, 1, 1], 2),
        ([0.5, 0.5], 1),
        ([-1, 2], 2),
        ([0.5, 0.5, 0.5], 1.5),
        ([1, 1e-8], 1e-8),
    ],
)
def test_one_hot_violation(values, violation):
    xs = [DecisionVariable.binary(i) for i in range(len(values))]
    instance = Instance.from_components(
        decision_variables=xs,
        objective=0,
        constraints={},
        one_hot_constraints={0: OneHotConstraint(variables=xs)},
        sense=Sense.Minimize,
    )
    state = dict(enumerate(values))
    # A small tolerance retains the tiny member rather than canonicalizing it.
    solution = instance.evaluate(state, atol=1e-12)
    samples = instance.evaluate_samples({7: state}, atol=1e-12)
    for result in [
        solution,
        samples.get(7),
        Solution.from_v2_bytes(solution.to_v2_bytes()),
        SampleSet.from_v2_bytes(samples.to_v2_bytes()).get(7),
    ]:
        assert result.constraint_violation(0, kind="one_hot") == pytest.approx(
            violation
        )
        assert result.total_violation() == pytest.approx(violation)
        frame = result.constraints_df(kind="one_hot")
        assert frame.loc[0, "violation"] == pytest.approx(violation)
        if violation == 0:
            assert frame.loc[0, "feasible"]


@pytest.mark.parametrize(
    "values, violation",
    [
        ([0, 0, 0], 0),
        ([3, 0, 0], 0),
        ([3, -2, 0], 2),
        ([3, -5, 1, 4, -6], 13),
        ([4, -7, 1, 6, -9], 18),
        ([0.5, -0.5, 0, 0.5, -0.5], 1.5),
        ([1e20, 1, -2], 3),
        ([1, 1e-8], 1e-8),
    ],
)
def test_sos1_violation(values, violation):
    xs = [DecisionVariable.continuous(i) for i in range(len(values))]
    instance = Instance.from_components(
        decision_variables=xs,
        objective=0,
        constraints={},
        sos1_constraints={0: Sos1Constraint(variables=xs)},
        sense=Sense.Minimize,
    )
    state = dict(enumerate(values))
    solution = instance.evaluate(state)
    samples = instance.evaluate_samples({7: state})
    for result in [
        solution,
        samples.get(7),
        Solution.from_v2_bytes(solution.to_v2_bytes()),
        SampleSet.from_v2_bytes(samples.to_v2_bytes()).get(7),
    ]:
        assert result.constraint_violation(0, kind="sos1") == pytest.approx(violation)
        assert result.total_violation() == pytest.approx(violation)
        frame = result.constraints_df(kind="sos1")
        assert frame.loc[0, "violation"] == pytest.approx(violation)
        if violation == 0:
            assert frame.loc[0, "feasible"]


def test_violation_uses_evaluated_discrete_values():
    xs = [DecisionVariable.binary(i) for i in range(2)]
    instance = Instance.from_components(
        decision_variables=xs,
        objective=0,
        constraints={},
        one_hot_constraints={0: OneHotConstraint(variables=xs)},
        sense=Sense.Minimize,
    )
    solution = instance.evaluate({0: 0.125, 1: 0.875}, atol=0.125)
    assert solution.total_violation() == 0
    assert solution.feasible


@pytest.mark.parametrize("atol", [1e-12, 0.5, 1.0, 2.0])
def test_zero_violation_implies_one_hot_feasibility_with_large_tolerance(atol):
    xs = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=xs,
        objective=0,
        constraints={},
        one_hot_constraints={0: OneHotConstraint(variables=xs)},
        sense=Sense.Minimize,
    )
    state = {0: 0, 1: 1, 2: 0}
    solution = instance.evaluate(state, atol=atol)
    samples = instance.evaluate_samples({7: state}, atol=atol)
    for result in [
        solution,
        samples.get(7),
        Solution.from_v2_bytes(solution.to_v2_bytes()),
        SampleSet.from_v2_bytes(samples.to_v2_bytes()).get(7),
    ]:
        assert result.total_violation() == 0
        assert result.feasible


def test_total_is_sum_of_individual_and_dataframe_violations():
    xs = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=xs,
        objective=0,
        constraints={0: xs[0] == 3},
        indicator_constraints={0: (xs[1] <= 0).with_indicator(xs[0])},
        one_hot_constraints={0: OneHotConstraint(variables=xs)},
        sos1_constraints={0: Sos1Constraint(variables=xs)},
        sense=Sense.Minimize,
    )
    instance.relax_constraint(0, "test relaxation")
    one_hot_row = instance.convert_one_hot_to_constraint(0)
    sos1_rows = instance.convert_sos1_to_constraints(0)
    solution = instance.evaluate({0: 1, 1: 1, 2: 1})
    kinds: list[Literal["regular", "indicator", "one_hot", "sos1"]] = [
        "regular",
        "indicator",
        "one_hot",
        "sos1",
    ]
    assert [solution.constraint_violation(0, kind=kind) for kind in kinds] == [
        2,
        1,
        2,
        2,
    ]
    assert solution.constraint_violation(one_hot_row) == 2
    assert sum(solution.constraint_violation(row) for row in sos1_rows) == 2
    # Generated rows and retained removed originals both contribute.
    assert solution.total_violation() == 11
    assert (
        sum(solution.constraints_df(kind=kind)["violation"].sum() for kind in kinds)
        == 11
    )
    assert solution.constraints[0].violation() == solution.constraint_violation(0)
    for kind in kinds:
        with pytest.raises(KeyError, match="42"):
            solution.constraint_violation(42, kind=kind)
    assert not hasattr(solution, "total_violation_l1")
    assert not hasattr(solution, "total_violation_l2")


def test_lowering_may_change_total_violation():
    xs = [DecisionVariable.binary(i) for i in range(2)]
    instance = Instance.from_components(
        decision_variables=xs,
        objective=0,
        constraints={},
        one_hot_constraints={0: OneHotConstraint(variables=xs)},
        sense=Sense.Minimize,
    )
    state = {0: 0.5, 1: 0.5}
    assert instance.evaluate(state).total_violation() == 1
    row = instance.convert_one_hot_to_constraint(0)
    lowered = instance.evaluate(state)
    assert lowered.constraint_violation(row) == 0
    assert lowered.constraint_violation(0, kind="one_hot") == 1
    # Removed native constraints and generated regular constraints both contribute.
    assert instance.evaluate({0: 1, 1: 1}).total_violation() == 2


def test_zero_constraint_violation_does_not_validate_variable_bounds():
    x = DecisionVariable.continuous(0, lower=0, upper=1)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=0,
        constraints={},
        sos1_constraints={0: Sos1Constraint(variables=[x])},
        sense=Sense.Minimize,
    )
    solution = instance.evaluate({0: 10})
    assert solution.total_violation() == 0
    assert solution.constraints_df(kind="sos1").loc[0, "feasible"]
    assert not solution.feasible


def test_feasible_sos1_can_have_positive_violation_within_tolerance():
    xs = [DecisionVariable.continuous(i) for i in range(2)]
    instance = Instance.from_components(
        decision_variables=xs,
        objective=0,
        constraints={},
        sos1_constraints={0: Sos1Constraint(variables=xs)},
        sense=Sense.Minimize,
    )
    solution = instance.evaluate({0: 3, 1: 0.125}, atol=0.125)
    assert solution.total_violation() == 0.125
    assert solution.feasible


@pytest.mark.parametrize("atol", [1e-4, 1e-8])
@pytest.mark.parametrize("lowered", [False, True])
def test_sos1_feasibility_uses_total_member_error_across_evaluation_paths(
    atol, lowered
):
    xs = [DecisionVariable.continuous(i, lower=-2, upper=2) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=xs,
        objective=0,
        constraints={},
        sos1_constraints={0: Sos1Constraint(variables=xs)},
        sense=Sense.Minimize,
    )
    if lowered:
        instance.convert_sos1_to_constraints(0)
    state = {0: 1, 1: 0.75 * atol, 2: 0.75 * atol}
    if lowered:
        # The generated selectors correspond to x0, x1, and x2 in order.
        state.update({3: 1, 4: 0, 5: 0})
    solution = instance.evaluate(state, atol=atol)
    samples = instance.evaluate_samples({7: state}, atol=atol)
    for result in [
        solution,
        samples.get(7),
        Solution.from_v2_bytes(solution.to_v2_bytes()),
        SampleSet.from_v2_bytes(samples.to_v2_bytes()).get(7),
    ]:
        violation = result.constraint_violation(0, kind="sos1")
        assert violation == pytest.approx(1.5 * atol)
        regular_total = sum(c.violation() for c in result.constraints.values())
        assert result.total_violation() == pytest.approx(violation + regular_total)
        row = result.constraints_df(kind="sos1").loc[0]
        assert row["violation"] == violation
        assert not row["feasible"]
        assert not result.feasible
        assert result.feasible_relaxed == lowered


@pytest.mark.parametrize("atol", [1e-4, 1e-8])
def test_partial_evaluation_cannot_discard_small_sos1_member_contributions(atol):
    xs = [DecisionVariable.continuous(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=xs,
        objective=0,
        constraints={},
        sos1_constraints={0: Sos1Constraint(variables=xs)},
        sense=Sense.Minimize,
    )
    fixed = {1: 0.75 * atol, 2: 0.75 * atol}
    original = instance.evaluate({0: 1, **fixed}, atol=atol)
    assert not original.feasible
    with pytest.raises(RuntimeError, match="without changing constraint feasibility"):
        instance.partial_evaluate(fixed, atol=atol)
    assert (
        instance.evaluate({0: 1, **fixed}, atol=atol).total_violation()
        == original.total_violation()
    )

    # Exact zero elimination preserves both the scalar metric and a valid Instance.
    original = instance.evaluate({0: 1, 1: 0.75 * atol, 2: 0}, atol=atol)
    partial = instance.partial_evaluate({2: 0}, atol=atol)
    restored = Instance.from_v2_bytes(partial.to_v2_bytes())
    for problem in [partial, restored]:
        result = problem.evaluate({0: 1, 1: 0.75 * atol}, atol=atol)
        assert result.total_violation() == original.total_violation()
        assert result.feasible


def test_constraint_thresholds_do_not_apply_to_total_violation():
    x = DecisionVariable.continuous(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=0,
        constraints={0: x == 0, 1: x <= 0},
        sense=Sense.Minimize,
    )
    solution = instance.evaluate({0: 0.000075}, atol=0.0001)
    assert solution.total_violation() == pytest.approx(0.00015)
    assert solution.feasible


@pytest.mark.parametrize(
    "variable,invalid,valid",
    [
        (DecisionVariable.binary(0), 0.5, 1.0),
        (DecisionVariable.integer(0), 0.5, 1.0),
        (DecisionVariable.continuous(0, lower=0, upper=1), -(2**-9), -(2**-10)),
    ],
)
def test_sampleset_feasibility_and_best_sample_include_variable_domains(
    variable, invalid, valid
):
    atol = 2**-10
    instance = Instance.from_components(
        decision_variables=[variable],
        objective=variable,
        constraints={},
        sos1_constraints={0: Sos1Constraint(variables=[variable])},
        sense=Sense.Minimize,
    )
    samples = instance.evaluate_samples({0: {0: invalid}, 1: {0: valid}}, atol=atol)
    for result in [samples, SampleSet.from_v2_bytes(samples.to_v2_bytes())]:
        assert result.feasible == result.feasible_relaxed == {0: False, 1: True}
        for sample_id in (0, 1):
            solution = result.get(sample_id)
            assert result.feasible[sample_id] == solution.feasible
            # Variable-domain validity does not change the constraint metric.
            assert solution.total_violation() == 0
        assert result.best_feasible.objective == valid
        assert result.best_feasible_relaxed.objective == valid
        assert result.summary.index.tolist() == [1, 0]
        assert result.summary["feasible"].to_dict() == result.feasible


def test_binary_canonicalization_precedes_constraint_violation():
    xs = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=xs,
        objective=0,
        constraints={},
        one_hot_constraints={0: OneHotConstraint(variables=xs)},
        sense=Sense.Minimize,
    )
    solution = instance.evaluate({0: 1, 1: 0.000075, 2: 0.000075}, atol=0.0001)
    assert solution.state.entries == {0: 1, 1: 0, 2: 0}
    assert solution.constraint_violation(0, kind="one_hot") == 0
    assert solution.feasible
