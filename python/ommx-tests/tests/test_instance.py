from ommx.v1 import Instance, DecisionVariable, Function, ConstraintHints, OneHot
import math
import pytest


def test_set_objective():
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints=[],
        sense=Instance.MAXIMIZE,
    )
    assert instance.objective.almost_equal(Function(sum(x)))

    instance.objective = x[1]
    assert instance.objective.almost_equal(Function(x[1]))


def test_convert_inequality_to_equality_with_integer_slack_limit():
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints=[(math.pi * x[0] + math.e * x[1] >= 1).set_id(0)],
        sense=Instance.MAXIMIZE,
    )
    with pytest.raises(RuntimeError) as e:
        instance.convert_inequality_to_equality_with_integer_slack(0, 32)
    assert (
        str(e.value)
        == "The range of the slack variable exceeds the limit: evaluated(15174216961756088) > limit(32)"
    )


def test_convert_inequality_to_equality_with_integer_slack_continuous():
    x = [DecisionVariable.continuous(i, lower=-1.23, upper=4.56) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints=[(x[0] + x[1] >= 7.89).set_id(0)],
        sense=Instance.MAXIMIZE,
    )
    with pytest.raises(RuntimeError) as e:
        instance.convert_inequality_to_equality_with_integer_slack(0, 32)
    assert (
        str(e.value)
        == "The constraint contains continuous decision variables: ID=VariableID(0)"
    )


def test_convert_inequality_to_equality_with_integer_slack_infeasible():
    x = [
        DecisionVariable.integer(i, lower=0, upper=3, name="x", subscripts=[i])
        for i in range(3)
    ]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints=[
            (x[0] + 2 * x[1] <= -1).set_id(
                0
            )  # Never satisfied since both x0 and x1 are non-negative
        ],
        sense=Instance.MAXIMIZE,
    )
    with pytest.raises(RuntimeError) as e:
        instance.convert_inequality_to_equality_with_integer_slack(0, 32)
    assert (
        str(e.value)
        == "The bound of `f(x)` in inequality constraint(ConstraintID(0)) `f(x) <= 0` is positive: Bound[1, 10]"
    )


def test_convert_inequality_to_equality_with_integer_slack_trivial():
    x = [
        DecisionVariable.integer(i, lower=0, upper=3, name="x", subscripts=[i])
        for i in range(3)
    ]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints=[
            (x[0] + 2 * x[1] >= 0).set_id(0)  # Trivially satisfied
        ],
        sense=Instance.MAXIMIZE,
    )
    instance.convert_inequality_to_equality_with_integer_slack(
        constraint_id=0, max_integer_range=32
    )
    assert instance.constraints == []
    removed = instance.removed_constraints[0]
    assert removed.id == 0


def test_add_integer_slack_to_inequality_infeasible():
    x = [
        DecisionVariable.integer(i, lower=0, upper=3, name="x", subscripts=[i])
        for i in range(3)
    ]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints=[
            (x[0] + 2 * x[1] <= -1).set_id(
                0
            )  # Never satisfied since both x0 and x1 are non-negative
        ],
        sense=Instance.MAXIMIZE,
    )
    with pytest.raises(RuntimeError) as e:
        instance.add_integer_slack_to_inequality(0, 4)
    assert (
        str(e.value)
        == "The bound of `f(x)` in inequality constraint(ConstraintID(0)) `f(x) <= 0` is positive: Bound[1, 10]"
    )


def test_add_integer_slack_to_inequality_trivial():
    x = [
        DecisionVariable.integer(i, lower=0, upper=3, name="x", subscripts=[i])
        for i in range(3)
    ]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints=[
            (x[0] + 2 * x[1] >= 0).set_id(0)  # Trivially satisfied
        ],
        sense=Instance.MAXIMIZE,
    )
    b = instance.add_integer_slack_to_inequality(0, 4)
    assert b is None

    # Check that the constraint is removed
    assert instance.constraints == []
    removed = instance.removed_constraints[0]
    assert removed.id == 0


def test_add_integer_slack_to_inequality_continuous():
    x = [DecisionVariable.continuous(i, lower=-1.23, upper=4.56) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints=[(x[0] + x[1] >= 7.89).set_id(0)],
        sense=Instance.MAXIMIZE,
    )
    with pytest.raises(RuntimeError) as e:
        instance.add_integer_slack_to_inequality(0, 4)
    assert (
        str(e.value)
        == "The constraint contains continuous decision variables: ID=VariableID(0)"
    )


def test_to_qubo_penalty_weight():
    x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(2)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0],
        constraints=[(x[0] == 0).set_id(123), (x[1] == 1).set_id(456)],
        sense=Instance.MINIMIZE,
    )
    # QUBO = x0 + 1 * (x0)^2 + 2 * (x1 - 1)^2 = 2*x0 - 2*x1 + 1
    qubo, offset = instance.to_qubo(penalty_weights={123: 1.0, 456: 2.0})
    assert qubo == {(0, 0): 2.0, (1, 1): -2.0}
    assert offset == 2.0


def test_to_qubo_continuous():
    x = [DecisionVariable.continuous(i, lower=-1.23, upper=4.56) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints=[(x[0] + x[1] >= 7.89).set_id(0)],
        sense=Instance.MAXIMIZE,
    )
    with pytest.raises(ValueError) as e:
        instance.to_qubo()
    assert (
        str(e.value)
        == "Continuous variables are not supported in QUBO conversion: IDs=[0, 1, 2]"
    )


def test_to_qubo_invalid_penalty_option():
    x = [
        DecisionVariable.integer(i, lower=0, upper=2, name="x", subscripts=[i])
        for i in range(2)
    ]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints=[(x[0] + 2 * x[1] <= 3).set_id(0)],
        sense=Instance.MAXIMIZE,
    )

    with pytest.raises(ValueError) as e:
        instance.to_qubo(uniform_penalty_weight=1.0, penalty_weights={0: 2.0})
    assert (
        str(e.value)
        == "Both uniform_penalty_weight and penalty_weights are specified. Please choose one."
    )


def test_hubo_3rd_degree():
    x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=(x[0] + x[0] * x[0] + x[0] * x[1] * x[2]),
        constraints=[],
        sense=Instance.MINIMIZE,
    )
    hubo, offset = instance.to_hubo()
    assert hubo == {(0,): 2.0, (0, 1, 2): 1.0}
    assert offset == 0.0


def test_to_hubo_penalty_weight():
    x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(2)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0],
        constraints=[(x[0] == 0).set_id(123), (x[1] == 1).set_id(456)],
        sense=Instance.MINIMIZE,
    )
    # QUBO = x0 + 1 * (x0)^2 + 2 * (x1 - 1)^2 = 2*x0 - 2*x1 + 1
    hubo, offset = instance.to_hubo(penalty_weights={123: 1.0, 456: 2.0})
    assert hubo == {(0,): 2.0, (1,): -2.0}
    assert offset == 2.0


def test_to_hubo_continuous():
    x = [DecisionVariable.continuous(i, lower=-1.23, upper=4.56) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints=[(x[0] + x[1] >= 7.89).set_id(0)],
        sense=Instance.MAXIMIZE,
    )
    with pytest.raises(ValueError) as e:
        instance.to_hubo()
    assert (
        str(e.value)
        == "Continuous variables are not supported in HUBO conversion: IDs=[0, 1, 2]"
    )


def test_to_hubo_invalid_penalty_option():
    x = [
        DecisionVariable.integer(i, lower=0, upper=2, name="x", subscripts=[i])
        for i in range(2)
    ]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints=[(x[0] + 2 * x[1] <= 3).set_id(0)],
        sense=Instance.MAXIMIZE,
    )

    with pytest.raises(ValueError) as e:
        instance.to_hubo(uniform_penalty_weight=1.0, penalty_weights={0: 2.0})
    assert (
        str(e.value)
        == "Both uniform_penalty_weight and penalty_weights are specified. Please choose one."
    )


def test_evaluate_irrelevant_binary_variable():
    x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0],
        constraints=[(x[1] == 1).set_id(0)],
        sense=Instance.MINIMIZE,
    )
    solution = instance.evaluate({0: 1, 1: 0})
    assert solution.extract_decision_variables("x") == {
        (0,): 1.0,
        (1,): 0.0,
        (2,): 0.0,  # Irrelevant variable
    }


def test_evaluate_irrelevant_integer_variables():
    x = [
        DecisionVariable.integer(0, lower=-3, upper=3, name="x", subscripts=[0]),
        DecisionVariable.integer(1, lower=-3, upper=3, name="x", subscripts=[1]),
        DecisionVariable.integer(
            2, lower=2, upper=5, name="x", subscripts=[2]
        ),  # 0 is not allowed
        DecisionVariable.integer(
            3, lower=-5, upper=-2, name="x", subscripts=[3]
        ),  # 0 is not allowed
        DecisionVariable.integer(4, name="x", subscripts=[4]),  # (-inf, inf)
    ]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0],
        constraints=[(x[1] == 1).set_id(0)],
        sense=Instance.MINIMIZE,
    )
    solution = instance.evaluate({0: 1, 1: 0})
    assert solution.extract_decision_variables("x") == {
        (0,): 1.0,
        (1,): 0.0,
        # Irrelevant variables
        (2,): 2.0,
        (3,): -2.0,
        (4,): 0.0,
    }


def test_restore_constraint_hint():
    x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0],
        constraints=[(x[0] + x[1] + x[2] == 1).set_id(0)],  # one-hot constraint
        sense=Instance.MINIMIZE,
        constraint_hints=ConstraintHints(
            one_hot_constraints=[OneHot(id=0, variables=[0, 1, 2])]
        ),
    )
    instance_bytes = instance.to_bytes()
    parsed_instance = Instance.from_bytes(instance_bytes)
    assert parsed_instance.constraint_hints.one_hot_constraints == [
        OneHot(id=0, variables=[0, 1, 2])
    ]


def test_restore_constraint_hint_relaxed():
    x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0],
        constraints=[(x[0] + x[1] + x[2] == 1).set_id(0)],  # one-hot constraint
        sense=Instance.MINIMIZE,
        constraint_hints=ConstraintHints(
            one_hot_constraints=[OneHot(id=0, variables=[0, 1, 2])]
        ),
    )
    instance.relax_constraint(0, reason="test")
    instance_bytes = instance.to_bytes()
    parsed_instance = Instance.from_bytes(instance_bytes)
    assert parsed_instance.constraint_hints.one_hot_constraints == [
        OneHot(id=0, variables=[0, 1, 2])
    ]


def test_stats_empty_instance():
    instance = Instance.from_components(
        decision_variables=[],
        objective=Function(0),
        constraints=[],
        sense=Instance.MINIMIZE,
    )
    stats = instance.stats()

    assert stats["decision_variables"]["total"] == 0
    assert stats["decision_variables"]["by_kind"]["binary"] == 0
    assert stats["decision_variables"]["by_kind"]["integer"] == 0
    assert stats["decision_variables"]["by_kind"]["continuous"] == 0
    assert stats["decision_variables"]["by_usage"]["used"] == 0
    assert stats["decision_variables"]["by_usage"]["fixed"] == 0
    assert stats["decision_variables"]["by_usage"]["dependent"] == 0
    assert stats["decision_variables"]["by_usage"]["irrelevant"] == 0

    assert stats["constraints"]["total"] == 0
    assert stats["constraints"]["active"] == 0
    assert stats["constraints"]["removed"] == 0


def test_stats_with_variables():
    x = [
        DecisionVariable.binary(0, name="x", subscripts=[0]),
        DecisionVariable.binary(1, name="x", subscripts=[1]),
        DecisionVariable.integer(2, lower=0, upper=10, name="x", subscripts=[2]),
        DecisionVariable.continuous(3, lower=0.0, upper=1.0, name="x", subscripts=[3]),
    ]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0] + x[1],  # Use only binary variables in objective
        constraints=[],
        sense=Instance.MINIMIZE,
    )
    stats = instance.stats()

    assert stats["decision_variables"]["total"] == 4
    assert stats["decision_variables"]["by_kind"]["binary"] == 2
    assert stats["decision_variables"]["by_kind"]["integer"] == 1
    assert stats["decision_variables"]["by_kind"]["continuous"] == 1
    assert stats["decision_variables"]["by_usage"]["used_in_objective"] == 2
    assert stats["decision_variables"]["by_usage"]["used"] == 2
    assert stats["decision_variables"]["by_usage"]["irrelevant"] == 2  # x[2] and x[3]


def test_stats_with_constraints():
    x = [
        DecisionVariable.binary(0, name="x", subscripts=[0]),
        DecisionVariable.binary(1, name="x", subscripts=[1]),
        DecisionVariable.binary(2, name="x", subscripts=[2]),
    ]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0],
        constraints=[
            (x[0] + x[1] == 1).set_id(0),
            (x[1] + x[2] == 1).set_id(1),
        ],
        sense=Instance.MINIMIZE,
    )

    # Remove one constraint
    instance.relax_constraint(1, reason="test removal")

    stats = instance.stats()

    assert stats["constraints"]["total"] == 2
    assert stats["constraints"]["active"] == 1
    assert stats["constraints"]["removed"] == 1
    # x[0] is used in objective, x[0] and x[1] are used in active constraint
    assert stats["decision_variables"]["by_usage"]["used_in_constraints"] == 2


def test_multiple_log_encodes():
    x = [
        DecisionVariable.integer(0, lower=0, upper=10, name="x", subscripts=[0]),
        DecisionVariable.integer(1, lower=0, upper=10, name="x", subscripts=[1]),
        DecisionVariable.integer(2, lower=0, upper=10, name="x", subscripts=[2]),
    ]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0],
        constraints=[
            (x[0] + x[1] <= 5).set_id(0),
            (x[1] + x[2] <= 5).set_id(1),
        ],
        sense=Instance.MAXIMIZE,
    )

    instance.log_encode()
    first_encode = instance.decision_variables
    instance.log_encode()
    second_encode = instance.decision_variables
    assert first_encode == second_encode
