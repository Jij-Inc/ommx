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
def test_missing_state_entry_raises_value_error_across_entrypoints(operation) -> None:
    with pytest.raises(ValueError):
        operation()


def test_instance_state_shape_errors_raise_value_error() -> None:
    instance = _instance_with_required_variable()

    with pytest.raises(ValueError, match="missing required variable IDs"):
        instance.populate_state({})
    with pytest.raises(ValueError, match="unknown variable IDs"):
        instance.populate_state({1: 0.0, 99: 0.0})


@pytest.mark.parametrize("value", [float("nan"), 0.5])
def test_partial_evaluate_validates_state_before_special_constraint_propagation(
    value: float,
) -> None:
    instance = _instance_with_one_hot_constraint()

    with pytest.raises(ValueError):
        instance.partial_evaluate({1: value})


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
