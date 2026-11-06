from itertools import product
import sys

import pytest

from ommx import (
    ConstraintID,
    DecisionVariable,
    Function,
    Instance,
    NamedFunction,
    OneHotConstraint,
    Parameter,
    ParametricInstance,
    Polynomial,
    Sense,
)


@pytest.mark.parametrize("path", ["fixed_only", "regular", "dependent", "one_hot"])
def test_partial_evaluate_preserves_reporting_context(path):
    x = DecisionVariable.binary(0)
    y = DecisionVariable.binary(1)
    z = DecisionVariable.continuous(2)
    metric = x + y + z
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=0 if path == "fixed_only" else x + y,
        decision_variables=[x, y, z],
        constraints={7: x + y == 1},
        named_functions=[
            NamedFunction(id=9, function=metric, name="metric", subscripts=[4])
        ],
        one_hot_constraints={8: OneHotConstraint(variables=[x, y])}
        if path == "one_hot"
        else {},
    )
    instance.relax_constraint(7, "retained source")
    if path == "dependent":
        instance.substitute({2: 2 * x})
    before = instance.to_v2_bytes()
    removed = instance.removed_constraints[ConstraintID(7)].function
    fixed = instance.partial_evaluate({0: 1.0000005}, atol=1e-6)
    assert instance.to_v2_bytes() == before
    for result in (fixed, Instance.from_v2_bytes(fixed.to_v2_bytes())):
        nf = result.get_named_function_by_id(9)
        assert nf.function == metric
        assert nf.name == "metric"
        assert nf.subscripts == [4]
        assert result.removed_constraints[ConstraintID(7)].function == removed
        assert result.fixed_decision_variables()[0] == 1
        if path == "one_hot":
            assert result.fixed_decision_variables()[1] == 0
        if path == "dependent":
            assert result.fixed_decision_variables()[2] == 2
        solution = result.evaluate({1: 0, 2: 2})
        source = instance.evaluate({0: 1, 1: 0, 2: 2})
        assert solution.objective == source.objective
        assert solution.feasible == source.feasible
        evaluated = solution.get_named_function_by_id(9)
        assert evaluated.evaluated_value == 3
        assert evaluated.used_decision_variable_ids == {0, 1, 2}
        samples = result.evaluate_samples({5: {1: 0, 2: 2}})
        assert samples.extract_named_functions("metric", 5) == {(4,): 3}


@pytest.mark.parametrize("active", [False, True])
def test_partial_evaluate_does_not_expand_reporting_coefficients(active):
    x = DecisionVariable.continuous(0)
    y = DecisionVariable.continuous(1)
    metric = Function(Polynomial({(0, 1): sys.float_info.max}))
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=x if active else 0,
        decision_variables=[x, y],
        constraints={},
        named_functions=[NamedFunction(id=9, function=metric)],
    )
    fixed = instance.partial_evaluate({1: 2})
    assert fixed.get_named_function_by_id(9).function == metric
    assert fixed.evaluate({0: 0}).get_named_function_by_id(9).evaluated_value == 0


@pytest.mark.parametrize(
    "operation", ["log_encode", "unary_encode", "substitute", "parametric"]
)
@pytest.mark.parametrize("existing_output", [False, True])
def test_substitution_preserves_named_function_for_reconstructed_values(
    operation, existing_output
):
    x = DecisionVariable.integer(0, lower=-1, upper=1)
    bits = [DecisionVariable.binary(1), DecisionVariable.binary(2)]
    metric = Function(Polynomial({(): sys.float_info.max, (0, 0): -sys.float_info.max}))
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[x, *bits],
        constraints={7: x <= 1},
        named_functions=[NamedFunction(id=9, function=metric, name="source_metric")],
    )
    if existing_output:
        assert instance.convert_active_objective(Sense.Maximize)
    source = Instance.from_v2_bytes(instance.to_v2_bytes())

    if operation == "parametric":
        parametric = instance.as_parametric_instance()
        parametric.substitute({0: bits[0] + bits[1] - 1})
        assert parametric.named_functions[0].function == metric
        instance = parametric.with_parameters({})
    elif operation == "substitute":
        instance.substitute({0: bits[0] + bits[1] - 1})
    else:
        getattr(instance, operation)({0})

    assert instance.named_functions[0].function == metric
    assert instance.named_functions[0].name == "source_metric"
    assert instance.output_objective is not None
    expected_output = (
        source.output_objective.function if existing_output else source.objective
    )
    assert instance.output_objective.function == expected_output
    decoded = Instance.from_v2_bytes(instance.to_v2_bytes())
    used = sorted(instance.required_ids())
    assert len(used) == 2
    seen = set()
    samples = {}
    expected_metrics = {}
    for values in product([0, 1], repeat=len(used)):
        state = dict(zip(used, values))
        for transformed in (instance, decoded):
            result = transformed.evaluate(state)
            original_x = result.state.entries[0]
            seen.add(original_x)
            expected = source.evaluate({0: original_x})
            assert result.feasible == expected.feasible
            assert result.objective == expected.objective
            assert (
                result.get_named_function_by_id(9).evaluated_value
                == expected.get_named_function_by_id(9).evaluated_value
            )
        sample_id = len(samples)
        samples[sample_id] = state
        expected_metrics[sample_id] = expected.get_named_function_by_id(
            9
        ).evaluated_value
    assert seen == {-1, 0, 1}
    sampled = instance.evaluate_samples(samples)
    for sample_id, expected_metric in expected_metrics.items():
        assert sampled.extract_named_functions("source_metric", sample_id) == {
            (): expected_metric
        }


@pytest.mark.parametrize(
    "operation", ["log_encode", "unary_encode", "substitute", "parametric"]
)
def test_substitution_active_overflow_is_atomic(operation):
    x = DecisionVariable.integer(0, lower=0, upper=3)
    bits = [DecisionVariable.binary(1), DecisionVariable.binary(2)]
    overflowing = Function(Polynomial({(0, 0): sys.float_info.max}))
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[x, *bits],
        constraints={7: overflowing <= 0},
        named_functions=[NamedFunction(id=9, function=x * x)],
    )
    target = (
        instance.as_parametric_instance() if operation == "parametric" else instance
    )
    before = target.to_v2_bytes()
    with pytest.raises(ValueError, match="Coefficient must be finite"):
        if operation in ("substitute", "parametric"):
            target.substitute({0: bits[0] + 2 * bits[1]})
        else:
            getattr(target, operation)({0})
    assert target.to_v2_bytes() == before


@pytest.mark.parametrize(
    "operation", ["log_encode", "unary_encode", "substitute", "parametric"]
)
def test_substitution_only_in_context_does_not_capture_objective(operation):
    x = DecisionVariable.integer(0, lower=-1, upper=1)
    bits = [DecisionVariable.binary(1), DecisionVariable.binary(2)]
    source = x * x
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=0,
        decision_variables=[x, *bits],
        constraints={},
        named_functions=[NamedFunction(id=9, function=source)],
    )
    if operation == "parametric":
        instance = instance.as_parametric_instance()
    if operation in ("substitute", "parametric"):
        instance.substitute({0: bits[0] + bits[1] - 1})
    else:
        getattr(instance, operation)({0})
    if operation == "parametric":
        instance = instance.with_parameters({})
    assert instance.output_objective is None
    assert instance.required_ids() == set()
    assert instance.named_functions[0].function == source
    for bit in (0, 1):
        state = {
            variable.id: bit
            for variable in instance.decision_variables
            if variable.id != 0
        }
        result = instance.evaluate(state)
        assert result.state.entries[0] == 2 * bit - 1
        assert result.get_named_function_by_id(9).evaluated_value == 1


def test_parametric_substitution_keeps_source_until_parameter_specialization():
    x = DecisionVariable.continuous(0)
    b = DecisionVariable.binary(1)
    p = Parameter(2)
    instance = ParametricInstance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[x, b],
        parameters=[p],
        constraints={},
        named_functions=[NamedFunction(id=9, function=x * x + p)],
    )
    source = instance.named_functions[0].function
    instance.substitute({0: b + p})
    assert instance.named_functions[0].function == source
    concrete = instance.with_parameters({2: 3})
    assert concrete.named_functions[0].function.required_ids() == {0}
    solution = concrete.evaluate({1: 1})
    assert solution.state.entries[0] == 4
    assert solution.get_named_function_by_id(9).evaluated_value == 19
    assert solution.objective == 4


@pytest.mark.parametrize("parametric", [False, True])
def test_substitution_allows_intentionally_restricted_domain(parametric):
    x = DecisionVariable.integer(0, lower=0, upper=2)
    b = DecisionVariable.binary(1)
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[x, b],
        constraints={},
        named_functions=[NamedFunction(id=9, function=x * x)],
    )
    if parametric:
        instance = instance.as_parametric_instance()
    instance.substitute({0: 2 * b})  # Intentionally excludes original x = 1.
    if parametric:
        instance = instance.with_parameters({})
    assert instance.named_functions[0].function.required_ids() == {0}
    for value in (0, 1):
        result = instance.evaluate({1: value})
        assert result.feasible
        assert result.state.entries[0] == 2 * value
        assert result.get_named_function_by_id(9).evaluated_value == 4 * value
