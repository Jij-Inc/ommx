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
    Optimality,
    Parameter,
    ParametricInstance,
    Polynomial,
    Sense,
)


@pytest.mark.parametrize("parametric", [False, True])
@pytest.mark.parametrize("assumption", [None, False, True])
@pytest.mark.parametrize("previous", [None, False, True])
@pytest.mark.parametrize("invalid", ["target", "rhs", "intermediate", "irrelevant"])
def test_substitution_rejects_undefined_ids_atomically(
    parametric, assumption, previous, invalid
):
    x, y, z = [DecisionVariable.binary(i) for i in range(3)]
    unknown = DecisionVariable.binary(99)
    source = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[x, y, z],
        constraints={7: x <= 1},
        named_functions=[NamedFunction(id=9, function=x + y)],
    )
    if previous is not None:
        source.convert_active_objective(Sense.Maximize)
    if previous is False:
        source.substitute({2: y})
    target = source.as_parametric_instance() if parametric else source
    assignments = {
        "target": {99: y},
        "rhs": {0: unknown},
        "intermediate": {0: unknown, 99: y},
        "irrelevant": {1: unknown},
    }[invalid]
    before = target.to_v2_bytes()
    with pytest.raises(ValueError, match="Undefined variable ID.*99"):
        if assumption is None:
            target.substitute(assignments)
        else:
            target.substitute(assignments, assume_preserves_optimality=assumption)
    assert target.to_v2_bytes() == before
    if isinstance(target, ParametricInstance):
        concrete = ParametricInstance.from_v2_bytes(before).with_parameters({})
    else:
        concrete = Instance.from_v2_bytes(before)
    output = concrete.output_objective
    if previous is None:
        assert output is None
    else:
        assert output is not None and output.preserves_optimality == previous
    assert concrete.evaluate({0: 1, 1: 0}).objective == 1


@pytest.mark.parametrize("parametric", [False, True])
@pytest.mark.parametrize("existing_output", [False, True])
def test_substitution_rounding_cannot_transport_optimality(parametric, existing_output):
    x, b = [DecisionVariable.binary(i) for i in range(2)]
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=2**22 * x - b,
        decision_variables=[x, b],
        constraints={},
    )
    if existing_output:
        instance.convert_active_objective(Sense.Maximize)
    target = instance.as_parametric_instance() if parametric else instance
    target.substitute({0: 2**-21 * b})
    if isinstance(target, ParametricInstance):
        target = ParametricInstance.from_v2_bytes(target.to_v2_bytes()).with_parameters(
            {}
        )
    for concrete in (target, Instance.from_v2_bytes(target.to_v2_bytes())):
        output = concrete.output_objective
        assert output is not None and not output.preserves_optimality
        for status in (Optimality.Optimal, Optimality.NotOptimal):
            assert concrete.map_active_optimality(status) == Optimality.Unspecified
        samples = concrete.evaluate_samples({0: {1: 0}, 1: {1: 1}}, atol=1e-6)
        for value in (0, 1):
            assert concrete.objective.evaluate({1: value}) == (
                -value if existing_output else value
            )
            solution = concrete.evaluate({1: value}, atol=1e-6)
            assert solution.feasible
            assert solution.state.entries[0] == 0
            assert solution.objective == -value
            assert samples.get(value).objective == -value


@pytest.mark.parametrize("parametric", [False, True])
@pytest.mark.parametrize("existing_guarantee", [None, False, True])
@pytest.mark.parametrize("assume_preserves_optimality", [False, True])
def test_substitution_composes_caller_optimality_assumption(
    parametric, existing_guarantee, assume_preserves_optimality
):
    x, y, z = [DecisionVariable.binary(i) for i in range(3)]
    source = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[x, y, z],
        constraints={},
    )
    source_objective = source.objective
    if existing_guarantee is not None:
        source.convert_active_objective(Sense.Maximize)
    assigned_id = 0
    if existing_guarantee is False:
        source.substitute({0: y})
        assigned_id = 1
    target = source.as_parametric_instance() if parametric else source
    target.substitute(
        {assigned_id: z}, assume_preserves_optimality=assume_preserves_optimality
    )
    expected = existing_guarantee is not False and assume_preserves_optimality
    output = target.output_objective
    assert output is not None
    assert output.preserves_optimality == expected
    assert output.function == source_objective
    assert output.sense == Sense.Minimize

    # Empty calls and rejected assignments cannot alter even a true guarantee.
    before = target.to_v2_bytes()
    for assumption in (False, True):
        target.substitute({}, assume_preserves_optimality=assumption)
        assert target.to_v2_bytes() == before
        with pytest.raises(ValueError, match="already fixed or dependent"):
            target.substitute({assigned_id: 0}, assume_preserves_optimality=assumption)
        assert target.to_v2_bytes() == before

    if isinstance(target, ParametricInstance):
        target = ParametricInstance.from_v2_bytes(target.to_v2_bytes()).with_parameters(
            {}
        )
    for concrete in (target, Instance.from_v2_bytes(target.to_v2_bytes())):
        output = concrete.output_objective
        assert output is not None and output.preserves_optimality == expected
        for status in (Optimality.Optimal, Optimality.NotOptimal):
            assert concrete.map_active_optimality(status) == (
                status if expected else Optimality.Unspecified
            )
        for value in (0, 1):
            solution = concrete.evaluate({2: value})
            assert solution.objective == value
            assert solution.sense == Sense.Minimize
            assert solution.feasible


@pytest.mark.parametrize("parametric", [False, True])
@pytest.mark.parametrize("assume_preserves_optimality", [False, True])
def test_substitution_composes_guarantee_without_active_objective_change(
    parametric, assume_preserves_optimality
):
    x, y, z = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[x, y, z],
        constraints={},
    )
    instance.convert_active_objective(Sense.Maximize)
    target = instance.as_parametric_instance() if parametric else instance
    target.substitute({})
    active, output = target.objective, target.output_objective
    assert output is not None and output.preserves_optimality
    target.substitute({1: z}, assume_preserves_optimality=assume_preserves_optimality)
    replaced_output = target.output_objective
    assert replaced_output is not None
    assert target.objective == active
    assert replaced_output.function == output.function
    assert replaced_output.sense == output.sense
    assert replaced_output.preserves_optimality == assume_preserves_optimality


@pytest.mark.parametrize("method", ["log_encode", "unary_encode"])
@pytest.mark.parametrize("invalidate_first", [False, True])
def test_exact_encoding_does_not_restore_lost_substitution_guarantee(
    method, invalidate_first
):
    x = DecisionVariable.integer(0, lower=0, upper=3)
    y, z = DecisionVariable.binary(1), DecisionVariable.binary(2)
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=x + y,
        decision_variables=[x, y, z],
        constraints={},
    )
    instance.convert_active_objective(Sense.Maximize)
    if invalidate_first:
        instance.substitute({1: z})
    getattr(instance, method)({0})
    output = instance.output_objective
    assert output is not None
    assert output.preserves_optimality == (not invalidate_first)
    expected = Optimality.Unspecified if invalidate_first else Optimality.Optimal
    assert instance.map_active_optimality(Optimality.Optimal) == expected


@pytest.mark.parametrize("parametric", [False, True])
def test_substitution_without_distinct_output_keeps_identical_objective_ordering(
    parametric,
):
    x, y, b = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=b,
        decision_variables=[x, y, b],
        constraints={},
    )
    target = instance.as_parametric_instance() if parametric else instance
    target.substitute({0: y})
    target.substitute({1: 2**-21 * b})
    concrete = (
        target.with_parameters({}) if isinstance(target, ParametricInstance) else target
    )
    assert concrete.output_objective is None
    # No output pair means the active objective itself is evaluated for output.
    assert concrete.map_active_optimality(Optimality.Optimal) == Optimality.Optimal
    for value in (0, 1):
        solution = concrete.evaluate({2: value}, atol=1e-6)
        assert solution.objective == concrete.objective.evaluate({2: value})
        assert solution.state.entries[0] == solution.state.entries[1] == 0


@pytest.mark.parametrize("parametric", [False, True])
def test_reassigning_dependency_is_rejected_without_changing_output(parametric):
    x, y, z = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[x, y, z],
        constraints={},
        named_functions=[NamedFunction(id=9, function=x)],
    )
    target = instance.as_parametric_instance() if parametric else instance
    target.substitute({0: y})
    output = target.output_objective
    assert output is not None and not output.preserves_optimality
    # Extending a chain remains supported without claiming proven ordering.
    target.substitute({1: z})
    output = target.output_objective
    assert output is not None and not output.preserves_optimality
    # Overwriting its source interpretation would reverse that ordering.
    before = target.to_v2_bytes()
    with pytest.raises(ValueError, match="already fixed or dependent"):
        target.substitute({0: 1 - z})
    assert target.to_v2_bytes() == before
    result = (
        target.with_parameters({}) if isinstance(target, ParametricInstance) else target
    )
    for concrete in (result, Instance.from_v2_bytes(result.to_v2_bytes())):
        output = concrete.output_objective
        assert output is not None and not output.preserves_optimality
        assert concrete.objective.evaluate({2: 0}) == 0
        solution = concrete.evaluate({2: 0})
        assert solution.objective == 0
        assert solution.get_named_function_by_id(9).evaluated_value == 0
        assert (
            concrete.map_active_optimality(Optimality.Optimal) == Optimality.Unspecified
        )
        assert (
            concrete.map_active_optimality(Optimality.NotOptimal)
            == Optimality.Unspecified
        )


@pytest.mark.parametrize("parametric", [False, True])
@pytest.mark.parametrize("normalize_dependency", [False, True])
def test_substitution_rejects_fixed_targets_atomically(
    parametric, normalize_dependency
):
    x, y, z = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=x + z,
        decision_variables=[x, y, z],
        constraints={},
    )
    if normalize_dependency:
        instance.substitute({0: y})
        instance = instance.partial_evaluate({1: 1})
    else:
        instance = instance.partial_evaluate({0: 1})
    target = instance.as_parametric_instance() if parametric else instance
    before = target.to_v2_bytes()
    with pytest.raises(ValueError, match="already fixed or dependent"):
        target.substitute({2: 0, 0: y})
    assert target.to_v2_bytes() == before


@pytest.mark.parametrize("first", ["log_encode", "unary_encode"])
@pytest.mark.parametrize("second", ["log_encode", "unary_encode", "substitute"])
def test_integer_encoding_cannot_replace_existing_representation(first, second):
    x = DecisionVariable.integer(0, lower=0, upper=3)
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[x],
        constraints={},
    )
    getattr(instance, first)({0})
    before = instance.to_v2_bytes()
    with pytest.raises(ValueError, match="already fixed or dependent"):
        if second == "substitute":
            instance.substitute({0: 0})
        else:
            getattr(instance, second)({0})
    assert instance.to_v2_bytes() == before


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
    source_output = source.output_objective
    expected_output = (
        source_output.function if source_output is not None else source.objective
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
@pytest.mark.parametrize("assume_preserves_optimality", [False, True])
def test_substitution_active_overflow_is_atomic(operation, assume_preserves_optimality):
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
    instance.convert_active_objective(Sense.Maximize)
    target = (
        instance.as_parametric_instance() if operation == "parametric" else instance
    )
    before = target.to_v2_bytes()
    with pytest.raises(ValueError, match="Coefficient must be finite"):
        if operation in ("substitute", "parametric"):
            target.substitute(
                {0: bits[0] + 2 * bits[1]},
                assume_preserves_optimality=assume_preserves_optimality,
            )
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
        parametric = instance.as_parametric_instance()
        parametric.substitute({0: bits[0] + bits[1] - 1})
        instance = parametric.with_parameters({})
    elif operation == "substitute":
        instance.substitute({0: bits[0] + bits[1] - 1})
    else:
        getattr(instance, operation)({0})
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


@pytest.mark.parametrize("assume_preserves_optimality", [False, True])
def test_parametric_substitution_keeps_source_until_parameter_specialization(
    assume_preserves_optimality,
):
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
    instance.substitute(
        {0: b + p}, assume_preserves_optimality=assume_preserves_optimality
    )
    assert instance.named_functions[0].function == source
    instance = ParametricInstance.from_v2_bytes(instance.to_v2_bytes())
    concrete = instance.with_parameters({2: 3})
    output = concrete.output_objective
    assert output is not None
    assert output.preserves_optimality == assume_preserves_optimality
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
        parameterized = instance.as_parametric_instance()
        parameterized.substitute({0: 2 * b})
        instance = parameterized.with_parameters({})
    else:
        instance.substitute({0: 2 * b})  # Intentionally excludes original x = 1.
    assert instance.named_functions[0].function.required_ids() == {0}
    for value in (0, 1):
        result = instance.evaluate({1: value})
        assert result.feasible
        assert result.state.entries[0] == 2 * value
        assert result.get_named_function_by_id(9).evaluated_value == 4 * value
