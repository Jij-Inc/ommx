from types import SimpleNamespace
from typing import cast

import openjij as oj
import pytest
from ommx import DecisionVariable, FixedPenaltyPreparation, Instance, Sos1Constraint
from ommx_openjij_adapter import OMMXOpenJijSAAdapter


def binary_no_constraint_minimize():
    x0 = DecisionVariable.binary(0, name="x", subscripts=[0])
    x1 = DecisionVariable.binary(1, name="x", subscripts=[1])
    instance = Instance.from_components(
        decision_variables=[x0, x1],
        objective=x0 + x1,
        constraints={},
        sense=Instance.MINIMIZE,
    )
    ans = {(0,): 0.0, (1,): 0.0}
    return pytest.param(instance, ans, id="binary_no_constraint_minimize")


def binary_no_constraint_maximize():
    x0 = DecisionVariable.binary(0, name="x", subscripts=[0])
    x1 = DecisionVariable.binary(1, name="x", subscripts=[1])
    instance = Instance.from_components(
        decision_variables=[x0, x1],
        objective=x0 + x1,
        constraints={},
        sense=Instance.MAXIMIZE,
    )
    ans = {(0,): 1.0, (1,): 1.0}
    return pytest.param(instance, ans, id="binary_no_constraint_maximize")


def binary_equality():
    x0 = DecisionVariable.binary(0, name="x", subscripts=[0])
    x1 = DecisionVariable.binary(1, name="x", subscripts=[1])
    x2 = DecisionVariable.binary(2, name="x", subscripts=[2])

    instance = Instance.from_components(
        decision_variables=[x0, x1, x2],
        objective=x0 + 2 * x1 + 3 * x2,
        constraints={0: x1 * x2 == 0},
        sense=Instance.MAXIMIZE,
    )

    # x0 = x2 = 1, x1 = 0 is maximum
    ans = {(0,): 1.0, (1,): 0.0, (2,): 1.0}
    return pytest.param(instance, ans, id="binary_equality")


def binary_inequality():
    x0 = DecisionVariable.binary(0, name="x", subscripts=[0])
    x1 = DecisionVariable.binary(1, name="x", subscripts=[1])
    x2 = DecisionVariable.binary(2, name="x", subscripts=[2])

    instance = Instance.from_components(
        decision_variables=[x0, x1, x2],
        objective=x0 + 2 * x1 + 3 * x2,
        constraints={0: x0 + x1 + x2 <= 2},
        sense=Instance.MAXIMIZE,
    )

    # x1 = x2 = 1, x0 = 0 is maximum
    ans = {(0,): 0.0, (1,): 1.0, (2,): 1.0}
    return pytest.param(instance, ans, id="binary_inequality")


def integer_equality():
    x0 = DecisionVariable.integer(0, name="x", lower=-1, upper=1, subscripts=[0])
    x1 = DecisionVariable.integer(1, name="x", lower=-1, upper=1, subscripts=[1])

    instance = Instance.from_components(
        decision_variables=[x0, x1],
        objective=x0 + 2 * x1,
        constraints={0: x0 + x1 == 0},
        sense=Instance.MAXIMIZE,
    )

    # x1 = -x0 = 1 is maximum
    ans = {(0,): -1.0, (1,): 1.0}
    return pytest.param(instance, ans, id="integer_equality")


def integer_inequality():
    x0 = DecisionVariable.integer(0, name="x", lower=-1, upper=1, subscripts=[0])
    x1 = DecisionVariable.integer(1, name="x", lower=-1, upper=1, subscripts=[1])

    instance = Instance.from_components(
        decision_variables=[x0, x1],
        objective=x0 + 2 * x1,
        constraints={0: x0 + x1 <= 0},
        sense=Instance.MAXIMIZE,
    )

    # x1 = -x0 = 1 is maximum
    ans = {(0,): -1.0, (1,): 1.0}
    return pytest.param(instance, ans, id="integer_inequality")


def hubo_binary_no_constraint_minimize():
    x0 = DecisionVariable.binary(0, name="x", subscripts=[0])
    x1 = DecisionVariable.binary(1, name="x", subscripts=[1])
    x2 = DecisionVariable.binary(2, name="x", subscripts=[2])
    instance = Instance.from_components(
        decision_variables=[x0, x1, x2],
        objective=x0 + x1 + x2 + x0 * x1 * x2,
        constraints={},
        sense=Instance.MINIMIZE,
    )
    ans = {(0,): 0.0, (1,): 0.0, (2,): 0.0}
    return pytest.param(instance, ans, id="hubo_binary_no_constraint_minimize")


def hubo_binary_no_constraint_maximize():
    x0 = DecisionVariable.binary(0, name="x", subscripts=[0])
    x1 = DecisionVariable.binary(1, name="x", subscripts=[1])
    x2 = DecisionVariable.binary(2, name="x", subscripts=[2])
    instance = Instance.from_components(
        decision_variables=[x0, x1, x2],
        objective=x0 + x0 * x1 * x2,
        constraints={},
        sense=Instance.MAXIMIZE,
    )
    ans = {(0,): 1.0, (1,): 1.0, (2,): 1.0}
    return pytest.param(instance, ans, id="hubo_binary_no_constraint_maximize")


def hubo_binary_equality():
    x0 = DecisionVariable.binary(0, name="x", subscripts=[0])
    x1 = DecisionVariable.binary(1, name="x", subscripts=[1])
    x2 = DecisionVariable.binary(2, name="x", subscripts=[2])

    instance = Instance.from_components(
        decision_variables=[x0, x1, x2],
        objective=x0 + 2 * x1 + 3 * x2 + x0 * x1 * x2,
        constraints={0: x1 * x2 == 0},
        sense=Instance.MAXIMIZE,
    )

    # x0 = x2 = 1, x1 = 0 is maximum
    ans = {(0,): 1.0, (1,): 0.0, (2,): 1.0}
    return pytest.param(instance, ans, id="hubo_binary_equality")


def hubo_binary_inequality():
    x0 = DecisionVariable.binary(0, name="x", subscripts=[0])
    x1 = DecisionVariable.binary(1, name="x", subscripts=[1])
    x2 = DecisionVariable.binary(2, name="x", subscripts=[2])

    instance = Instance.from_components(
        decision_variables=[x0, x1, x2],
        objective=x0 + 2 * x1 + 3 * x2 + x0 * x1 * x2,
        constraints={0: x0 + x1 + x2 <= 2},
        sense=Instance.MAXIMIZE,
    )

    # x1 = x2 = 1, x0 = 0 is maximum
    ans = {(0,): 0.0, (1,): 1.0, (2,): 1.0}
    return pytest.param(instance, ans, id="hubo_binary_inequality")


def _openjij_input(instance):
    if not OMMXOpenJijSAAdapter.check_applicability(instance).is_member:
        input_class = OMMXOpenJijSAAdapter.INPUT_CLASS
        policy = OMMXOpenJijSAAdapter.recommended_preparation_policy()
        policy.fixed_penalty = (
            FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight(weight=3.1)
        )
        instance.prepare(input_class, policy)
    return instance


def _expected_solution(instance, ans):
    state = {
        variable.id: ans[tuple(variable.subscripts)]
        for variable in instance.decision_variables
    }
    return instance.evaluate(state)


def _assert_sample_set_uses_instance_model(sample_set, instance):
    for sample_id in sample_set.sample_ids():
        actual = sample_set.get(sample_id)
        expected = instance.evaluate(actual.state)
        assert actual.objective == pytest.approx(expected.objective)
        assert actual.sense == expected.sense
        assert actual.feasible == expected.feasible
        assert len(actual.constraints) == len(expected.constraints)


def _assert_solution_uses_source_model(solution, instance, ans):
    expected = _expected_solution(instance, ans)
    assert solution.sense == expected.sense
    assert solution.objective == pytest.approx(expected.objective)
    assert solution.feasible == expected.feasible
    assert len(solution.constraints) == len(expected.constraints)


@pytest.mark.parametrize(
    "instance, ans",
    [
        binary_no_constraint_minimize(),
        binary_no_constraint_maximize(),
        binary_equality(),
        binary_inequality(),
        integer_equality(),
        integer_inequality(),
        hubo_binary_no_constraint_minimize(),
        hubo_binary_no_constraint_maximize(),
        hubo_binary_equality(),
        hubo_binary_inequality(),
    ],
)
def test_sample(instance, ans):
    # The uniform_penalty_weight of 3.1 was chosen to resolve multiple optimal solutions
    # effectively. This value was determined based on prior experimentation and ensures
    # that constraints are sufficiently penalized without overwhelming the objective.
    adapter_input = _openjij_input(instance)
    sample_set = OMMXOpenJijSAAdapter.sample(adapter_input, num_reads=1, seed=999)
    assert sample_set.extract_decision_variables("x", 0) == ans
    _assert_sample_set_uses_instance_model(sample_set, adapter_input)


@pytest.mark.parametrize(
    "instance, ans",
    [
        binary_no_constraint_minimize(),
        hubo_binary_no_constraint_minimize(),
    ],
)
def test_solve(instance, ans):
    before = instance.to_v2_bytes()
    solution = OMMXOpenJijSAAdapter.solve(instance, num_reads=1, seed=999)
    assert solution.extract_decision_variables("x") == ans
    _assert_solution_uses_source_model(solution, instance, ans)
    assert instance.to_v2_bytes() == before


@pytest.mark.parametrize(
    "instance, ans",
    [
        binary_no_constraint_minimize(),
        binary_no_constraint_maximize(),
        binary_equality(),
        binary_inequality(),
        integer_equality(),
        integer_inequality(),
        hubo_binary_no_constraint_minimize(),
        hubo_binary_no_constraint_maximize(),
        hubo_binary_equality(),
        hubo_binary_inequality(),
    ],
)
def test_sample_twice(instance, ans):
    adapter_input = _openjij_input(instance)
    for _ in range(2):
        sample_set = OMMXOpenJijSAAdapter.sample(adapter_input, num_reads=1, seed=999)
        assert sample_set.extract_decision_variables("x", 0) == ans
        _assert_sample_set_uses_instance_model(sample_set, adapter_input)


@pytest.mark.parametrize(
    "objective_factory",
    [
        pytest.param(lambda x: x - x * x, id="binary-power-cancellation"),
        pytest.param(lambda x: 1e-20 * x, id="sub-epsilon-coefficient"),
    ],
)
def test_sample_fills_evaluation_variables_omitted_from_sampler_input(
    objective_factory,
):
    x = DecisionVariable.binary(0, name="x", subscripts=[0])
    instance = Instance.from_components(
        decision_variables=[x],
        objective=objective_factory(x),
        constraints={},
        sense=Instance.MINIMIZE,
    )
    adapter = OMMXOpenJijSAAdapter(instance)
    assert adapter.sampler_input == {}

    sample_set = OMMXOpenJijSAAdapter.sample(instance, num_reads=1, seed=999)
    assert sample_set.extract_decision_variables("x", 0) == {(0,): 0.0}
    assert sample_set.objectives[0] == pytest.approx(0.0)


def test_decode_to_sampleset_uses_instance_variables():
    x0 = DecisionVariable.binary(0)
    x2 = DecisionVariable.binary(2)
    instance = Instance.from_components(
        decision_variables=[x0, x2],
        objective=x0 + x2,
        constraints={},
        sense=Instance.MINIMIZE,
    )
    response = SimpleNamespace(
        variables=[0, 1],  # 1 is not part of the Adapter input.
        record=SimpleNamespace(sample=[[1.0, 1.0]], num_occurrences=[1]),
    )

    adapter = OMMXOpenJijSAAdapter(instance)
    sample_set = adapter.decode_to_sampleset(cast(oj.Response, response))
    solution = sample_set.get(0)
    assert solution.state.get(0) == 1.0
    assert solution.state.get(1) is None
    assert solution.state.get(2) == 0.0
    assert solution.objective == pytest.approx(1.0)


@pytest.mark.parametrize(
    ("variable", "constraint", "expected_value"),
    [
        pytest.param(
            DecisionVariable.binary(0),
            lambda x: x - 2 <= 0,
            0.0,
            id="binary",
        ),
        pytest.param(
            DecisionVariable.integer(0, lower=2, upper=7),
            lambda x: x - 8 <= 0,
            2.0,
            id="integer",
        ),
    ],
)
def test_prepared_instance_evaluation_populates_variable_removed_with_trivial_inequality(
    variable,
    constraint,
    expected_value,
):
    instance = Instance.from_components(
        decision_variables=[variable],
        objective=0,
        constraints={7: constraint(variable)},
        sense=Instance.MINIMIZE,
    )
    input_class = OMMXOpenJijSAAdapter.INPUT_CLASS
    instance.prepare(
        input_class,
        OMMXOpenJijSAAdapter.recommended_preparation_policy(),
    )
    assert instance.used_decision_variables == []

    sample_set = OMMXOpenJijSAAdapter.sample(
        instance,
        num_reads=1,
        seed=999,
    )
    solution = sample_set.get(0)
    assert solution.state.get(0) == expected_value
    assert solution.feasible


def test_prepared_instance_evaluation_restores_integer_sos1():
    integer = DecisionVariable.integer(0, lower=-1, upper=1)
    binary = DecisionVariable.binary(1)
    instance = Instance.from_components(
        decision_variables=[integer, binary],
        objective=integer + binary,
        constraints={},
        sos1_constraints={30: Sos1Constraint(variables=[integer, binary])},
        sense=Instance.MINIMIZE,
    )

    input_class = OMMXOpenJijSAAdapter.INPUT_CLASS
    policy = OMMXOpenJijSAAdapter.recommended_preparation_policy()
    policy.fixed_penalty = (
        FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight(weight=4.0)
    )
    instance.prepare(input_class, policy)

    sample_set = OMMXOpenJijSAAdapter.sample(
        instance,
        num_reads=4,
        seed=999,
    )
    assert len(sample_set.constraints_df(kind="sos1")) == 1
