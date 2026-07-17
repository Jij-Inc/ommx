import sys

import pytest

import ommx


def _attached_constraint_with_missing_state():
    variable = ommx.DecisionVariable.continuous(1)
    instance = ommx.Instance.from_components(
        decision_variables=[variable],
        objective=0,
        constraints={0: variable == 0},
        sense=ommx.Instance.MINIMIZE,
    )
    return instance.constraints[0].evaluate({})


def _instance_with_required_variable() -> ommx.Instance:
    variable = ommx.DecisionVariable.continuous(1)
    return ommx.Instance.from_components(
        decision_variables=[variable],
        objective=variable,
        constraints={},
        sense=ommx.Instance.MINIMIZE,
    )


def _instance_with_one_hot_constraint() -> ommx.Instance:
    variables = [ommx.DecisionVariable.binary(1), ommx.DecisionVariable.binary(2)]
    return ommx.Instance.from_components(
        decision_variables=variables,
        objective=0,
        constraints={},
        one_hot_constraints={0: ommx.OneHotConstraint(variables=variables)},
        sense=ommx.Instance.MINIMIZE,
    )


def _dependent_instance_y_eq_scaled_x(coefficient: float) -> ommx.Instance:
    x = ommx.DecisionVariable.continuous(1)
    y = ommx.DecisionVariable.continuous(10)
    instance = ommx.Instance.from_components(
        decision_variables=[x, y],
        objective=y,
        constraints={},
        sense=ommx.Instance.MINIMIZE,
    )
    instance.substitute({10: coefficient * x})
    return instance


def _instance_with_sos1_derived_bound_conflict() -> ommx.Instance:
    variables = [
        ommx.DecisionVariable.continuous(1, lower=1.0, upper=2.0),
        ommx.DecisionVariable.continuous(2),
    ]
    return ommx.Instance.from_components(
        decision_variables=variables,
        objective=0,
        constraints={},
        sos1_constraints={0: ommx.Sos1Constraint(variables=variables)},
        sense=ommx.Instance.MINIMIZE,
    )


@pytest.mark.parametrize(
    "operation",
    [
        lambda: ommx.Linear({1: 1.0}).evaluate({}),
        lambda: ommx.Quadratic([1], [1], [1.0]).evaluate({}),
        lambda: ommx.Polynomial({(1,): 1.0}).evaluate({}),
        lambda: ommx.Function.from_linear(ommx.Linear({1: 1.0})).evaluate({}),
        lambda: (ommx.DecisionVariable.continuous(1) == 0).evaluate({}),
        lambda: ommx.NamedFunction(
            id=0, function=ommx.DecisionVariable.continuous(1)
        ).evaluate({}),
        _attached_constraint_with_missing_state,
        lambda: _instance_with_required_variable().evaluate({}),
        lambda: _instance_with_required_variable().evaluate_samples({7: {}}),
    ],
)
def test_missing_state_entries_raise_value_error_across_entrypoints(operation) -> None:
    with pytest.raises(ValueError):
        operation()


def test_missing_state_entries_report_all_required_ids() -> None:
    with pytest.raises(ValueError) as error:
        ommx.Linear({1: 1.0, 2: 1.0}).evaluate({})

    assert "1" in str(error.value)
    assert "2" in str(error.value)


def test_instance_state_shape_errors_raise_value_error() -> None:
    instance = _instance_with_required_variable()

    with pytest.raises(ValueError, match="missing required variable IDs"):
        instance.populate_state({})
    with pytest.raises(ValueError, match="unknown variable IDs"):
        instance.populate_state({1: 0.0, 99: 0.0})


def test_fixed_state_conflict_reuses_decision_variable_value_error() -> None:
    instance = _instance_with_required_variable().partial_evaluate({1: 0.0})

    with pytest.raises(ValueError, match="cannot be overwritten"):
        instance.populate_state({1: 1.0})


def test_non_finite_dependent_evaluation_falls_back_to_runtime_error() -> None:
    instance = _dependent_instance_y_eq_scaled_x(sys.float_info.max)

    with pytest.raises(RuntimeError, match="evaluated to non-finite"):
        instance.populate_state({1: sys.float_info.max})


def test_dependency_partial_evaluation_overflow_falls_back_to_runtime_error() -> None:
    instance = _dependent_instance_y_eq_scaled_x(sys.float_info.max)

    with pytest.raises(RuntimeError, match="failed to normalize dependent variable"):
        instance.partial_evaluate({1: sys.float_info.max})


def test_constraint_restore_overflow_falls_back_to_runtime_error() -> None:
    variable = ommx.DecisionVariable.continuous(1)
    instance = ommx.Instance.from_components(
        decision_variables=[variable],
        objective=0,
        constraints={0: sys.float_info.max * variable <= 0},
        sense=ommx.Instance.MINIMIZE,
    )
    instance.relax_constraint(0, "test")
    instance = instance.partial_evaluate({1: sys.float_info.max})

    with pytest.raises(RuntimeError, match="failed to normalize removed constraint"):
        instance.restore_constraint(0)

    assert instance.constraints == {}
    assert set(instance.removed_constraints) == {0}


def test_indicator_constraint_restore_overflow_falls_back_to_runtime_error() -> None:
    variable = ommx.DecisionVariable.continuous(1)
    indicator = ommx.DecisionVariable.binary(10)
    instance = ommx.Instance.from_components(
        decision_variables=[variable, indicator],
        objective=0,
        constraints={},
        indicator_constraints={
            0: ommx.IndicatorConstraint(
                indicator_variable=indicator,
                function=sys.float_info.max * variable,
                equality=ommx.Equality.LessThanOrEqualToZero,
            )
        },
        sense=ommx.Instance.MINIMIZE,
    )
    instance.relax_indicator_constraint(0, "test")
    instance = instance.partial_evaluate({1: sys.float_info.max})

    with pytest.raises(
        RuntimeError, match="failed to normalize removed indicator constraint"
    ):
        instance.restore_indicator_constraint(0)

    assert instance.indicator_constraints == {}
    assert set(instance.removed_indicator_constraints) == {0}


@pytest.mark.parametrize("value", [float("nan"), 0.5])
def test_partial_evaluate_validates_state_before_special_constraint_propagation(
    value: float,
) -> None:
    instance = _instance_with_one_hot_constraint()

    with pytest.raises(ValueError):
        instance.partial_evaluate({1: value})


def test_partial_evaluate_reports_all_unknown_state_entries() -> None:
    instance = _instance_with_one_hot_constraint()

    with pytest.raises(ValueError, match="unknown variable IDs") as error:
        instance.partial_evaluate({99: 0.0, 100: 0.0})

    assert "99" in str(error.value)
    assert "100" in str(error.value)


def test_special_constraint_propagation_failure_falls_back_to_runtime_error() -> None:
    instance = _instance_with_one_hot_constraint()

    with pytest.raises(RuntimeError, match="fixed to 0"):
        instance.partial_evaluate({1: 0.0, 2: 0.0})


def test_derived_state_validation_failure_falls_back_to_runtime_error() -> None:
    instance = _instance_with_sos1_derived_bound_conflict()

    with pytest.raises(RuntimeError, match="produced an invalid value"):
        instance.partial_evaluate({2: 1.0})


@pytest.mark.parametrize(
    "operation",
    [
        lambda: ommx.Linear({1: sys.float_info.max}).partial_evaluate(
            {1: sys.float_info.max}
        ),
        lambda: ommx.Quadratic([1], [1], [sys.float_info.max]).partial_evaluate(
            {1: sys.float_info.max}
        ),
        lambda: ommx.Polynomial({(1,): sys.float_info.max}).partial_evaluate(
            {1: sys.float_info.max}
        ),
        lambda: ommx.Function.from_linear(
            ommx.Linear({1: sys.float_info.max})
        ).partial_evaluate({1: sys.float_info.max}),
        lambda: (
            ommx.DecisionVariable.continuous(1) * sys.float_info.max == 0
        ).partial_evaluate({1: sys.float_info.max}),
        lambda: ommx.NamedFunction(
            id=0,
            function=ommx.DecisionVariable.continuous(1) * sys.float_info.max,
        ).partial_evaluate({1: sys.float_info.max}),
    ],
)
def test_partial_evaluate_preserves_coefficient_value_error(operation) -> None:
    with pytest.raises(ValueError, match="Coefficient must be finite"):
        operation()
