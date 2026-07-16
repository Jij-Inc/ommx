import pytest

import ommx


def _evaluated_results() -> tuple[ommx.Solution, ommx.SampleSet]:
    x = ommx.DecisionVariable.binary(0, name="x", subscripts=[0])
    constraint = (x == 1).set_name("c").add_subscripts([0])
    named_function = ommx.NamedFunction(id=0, function=x, name="metric", subscripts=[0])
    instance = ommx.Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={0: constraint},
        sense=ommx.Instance.MINIMIZE,
        named_functions=[named_function],
    )
    return instance.evaluate({0: 1}), instance.evaluate_samples({7: {0: 1}})


@pytest.mark.parametrize(
    "operation",
    [
        lambda solution, _sample_set: solution.extract_decision_variables("missing"),
        lambda solution, _sample_set: solution.extract_constraints("missing"),
        lambda solution, _sample_set: solution.extract_named_functions("missing"),
        lambda solution, _sample_set: solution.get_constraint_value(999),
        lambda solution, _sample_set: solution.get_dual_variable(999),
        lambda solution, _sample_set: solution.set_dual_variable(999, None),
        lambda _solution, sample_set: sample_set.get(999),
        lambda _solution, sample_set: sample_set.get_sample_by_id(999),
        lambda _solution, sample_set: sample_set.extract_decision_variables(
            "missing", 7
        ),
        lambda _solution, sample_set: sample_set.extract_constraints("missing", 7),
        lambda _solution, sample_set: sample_set.extract_named_functions("missing", 7),
        lambda _solution, sample_set: sample_set.extract_decision_variables("x", 999),
        lambda _solution, sample_set: sample_set.extract_all_decision_variables(999),
        lambda _solution, sample_set: sample_set.extract_all_named_functions(999),
    ],
)
def test_lookup_failures_raise_key_error(operation) -> None:
    solution, sample_set = _evaluated_results()

    with pytest.raises(KeyError):
        operation(solution, sample_set)


@pytest.mark.parametrize(
    "operation",
    [
        lambda: ommx.DecisionVariable(0, 1, ommx.Bound(2, 3)),
        lambda: ommx.DecisionVariable.integer(0, lower=0.1, upper=0.2),
    ],
)
def test_decision_variable_kind_bound_mismatch_raises_value_error(operation) -> None:
    with pytest.raises(ValueError, match="Bound is inconsistent to kind"):
        operation()


@pytest.mark.parametrize("kind", [0, 99])
def test_unknown_decision_variable_kind_raises_value_error(kind: int) -> None:
    with pytest.raises(ValueError, match=f"Unknown decision variable kind: {kind}"):
        ommx.DecisionVariable(0, kind, ommx.Bound(0, 1))


def test_duplicate_variable_subscripts_raise_value_error() -> None:
    variables = [
        ommx.DecisionVariable.binary(
            0, name="x", subscripts=[0], parameters={"scenario": "a"}
        ),
        ommx.DecisionVariable.binary(
            1, name="x", subscripts=[0], parameters={"scenario": "b"}
        ),
    ]
    instance = ommx.Instance.from_components(
        decision_variables=variables,
        objective=0,
        constraints={},
        sense=ommx.Instance.MINIMIZE,
    )
    solution = instance.evaluate({0: 0, 1: 1})
    sample_set = instance.evaluate_samples({7: {0: 0, 1: 1}})

    operations = [
        lambda: solution.extract_decision_variables("x"),
        solution.extract_all_decision_variables,
        lambda: sample_set.extract_decision_variables("x", 7),
        lambda: sample_set.extract_all_decision_variables(7),
    ]
    for operation in operations:
        with pytest.raises(ValueError, match="Duplicate subscript"):
            operation()


def test_duplicate_named_function_subscripts_raise_value_error() -> None:
    x = ommx.DecisionVariable.binary(0)
    named_functions = [
        ommx.NamedFunction(
            id=0,
            function=x,
            name="metric",
            subscripts=[0],
            parameters={"scenario": "a"},
        ),
        ommx.NamedFunction(
            id=1,
            function=x + 1,
            name="metric",
            subscripts=[0],
            parameters={"scenario": "b"},
        ),
    ]
    instance = ommx.Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=ommx.Instance.MINIMIZE,
        named_functions=named_functions,
    )
    solution = instance.evaluate({0: 1})
    sample_set = instance.evaluate_samples({7: {0: 1}})

    operations = [
        lambda: solution.extract_named_functions("metric"),
        solution.extract_all_named_functions,
        lambda: sample_set.extract_named_functions("metric", 7),
        lambda: sample_set.extract_all_named_functions(7),
    ]
    for operation in operations:
        with pytest.raises(ValueError, match="Duplicate subscript"):
            operation()


def test_parameterized_constraints_raise_value_error() -> None:
    x = ommx.DecisionVariable.binary(0)
    constraint = (x == 1).set_name("c").add_parameters({"group": "a"})
    instance = ommx.Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={0: constraint},
        sense=ommx.Instance.MINIMIZE,
    )
    solution = instance.evaluate({0: 1})
    sample_set = instance.evaluate_samples({7: {0: 1}})

    with pytest.raises(ValueError, match="Constraint with parameters"):
        solution.extract_constraints("c")
    with pytest.raises(ValueError, match="Constraint with parameters"):
        sample_set.extract_constraints("c", 7)


def test_missing_feasible_sample_raises_value_error_for_all_aliases() -> None:
    x = ommx.DecisionVariable.binary(0)
    instance = ommx.Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={0: x == 1},
        sense=ommx.Instance.MINIMIZE,
    )
    sample_set = instance.evaluate_samples({7: {0: 0}})

    for attribute in (
        "best_feasible_id",
        "best_feasible_relaxed_id",
        "best_feasible",
        "best_feasible_relaxed",
        "best_feasible_unrelaxed",
    ):
        with pytest.raises(ValueError, match="No feasible solution"):
            getattr(sample_set, attribute)


def test_all_extractors_validate_sample_id_before_skipping_unnamed_entries() -> None:
    x = ommx.DecisionVariable.binary(0)
    instance = ommx.Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=ommx.Instance.MINIMIZE,
        named_functions=[ommx.NamedFunction(id=0, function=x)],
    )
    sample_set = instance.evaluate_samples({7: {0: 1}})

    assert sample_set.extract_all_decision_variables(7) == {}
    assert sample_set.extract_all_named_functions(7) == {}

    with pytest.raises(KeyError):
        sample_set.extract_all_decision_variables(999)
    with pytest.raises(KeyError):
        sample_set.extract_all_named_functions(999)
