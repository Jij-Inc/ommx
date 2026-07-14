from ommx import (
    Instance,
    DecisionVariable,
    Function,
    Linear,
    Parameter,
    ParametricInstance,
)
import math
import pytest


def test_incremental_modeling_workflow():
    instance = Instance.maximize()
    x = instance.new_binary("x")
    y = instance.new_binary("y")

    instance.objective = x + y
    constraint = instance.add_constraint(x - y == 1, "c1")

    assert instance.sense == Instance.MAXIMIZE
    assert (x.id, x.name) == (0, "x")
    assert (y.id, y.name) == (1, "y")
    assert instance.objective.almost_equal(Function(x + y))
    assert constraint.constraint_id == 0
    assert constraint.name == "c1"


def test_minimize_creates_empty_minimization_instance():
    instance = Instance.minimize()

    assert instance.sense == Instance.MINIMIZE
    assert instance.decision_variables == []
    assert instance.constraints == {}
    assert instance.objective.almost_equal(Function(0))


def test_new_binary_accepts_full_modeling_label():
    instance = Instance.minimize()
    x = instance.new_binary(
        "x",
        subscripts=[1, 2],
        parameters={"region": "east"},
        description="dispatch decision",
    )

    reread = instance.attached_decision_variable(x.id)
    assert reread.name == "x"
    assert reread.subscripts == [1, 2]
    assert reread.parameters == {"region": "east"}
    assert reread.description == "dispatch decision"

    unnamed = instance.new_binary()
    assert unnamed.name == ""
    assert unnamed.subscripts == []
    assert unnamed.parameters == {}
    assert unnamed.description == ""


def test_add_constraint_optional_name_preserves_existing_usage():
    instance = Instance.empty()
    x = instance.add_decision_variable(DecisionVariable.binary(42))
    snapshot = (x == 1).set_name("existing")

    preserved = instance.add_constraint(snapshot)
    overridden = instance.add_constraint(snapshot, "override")

    assert preserved.name == "existing"
    assert overridden.name == "override"
    assert snapshot.name == "existing"


def test_add_constraint_accepts_full_modeling_label():
    instance = Instance.minimize()
    x = instance.new_binary("x")
    snapshot = (
        (x == 1)
        .set_name("existing")
        .set_subscripts([0])
        .set_parameters({"source": "snapshot"})
        .set_description("existing description")
    )

    attached = instance.add_constraint(
        snapshot,
        "balance",
        subscripts=[3, 4],
        parameters={"region": "east"},
        description="regional balance",
    )
    reread = instance.constraints[attached.constraint_id]

    assert reread.name == "balance"
    assert reread.subscripts == [3, 4]
    assert reread.parameters == {"region": "east"}
    assert reread.description == "regional balance"
    assert snapshot.name == "existing"
    assert snapshot.subscripts == [0]
    assert snapshot.parameters == {"source": "snapshot"}
    assert snapshot.description == "existing description"


def test_set_objective():
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
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
        constraints={0: math.pi * x[0] + math.e * x[1] >= 1},
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
        constraints={0: x[0] + x[1] >= 7.89},
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
        constraints={
            # Never satisfied since both x0 and x1 are non-negative
            0: (x[0] + 2 * x[1] <= -1),
        },
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
        constraints={0: x[0] + 2 * x[1] >= 0},  # Trivially satisfied
        sense=Instance.MAXIMIZE,
    )
    instance.convert_inequality_to_equality_with_integer_slack(
        constraint_id=0, max_integer_range=32
    )
    assert instance.constraints == {}
    assert 0 in instance.removed_constraints


def test_add_integer_slack_to_inequality_infeasible():
    x = [
        DecisionVariable.integer(i, lower=0, upper=3, name="x", subscripts=[i])
        for i in range(3)
    ]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={
            # Never satisfied since both x0 and x1 are non-negative
            0: (x[0] + 2 * x[1] <= -1),
        },
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
        constraints={0: x[0] + 2 * x[1] >= 0},  # Trivially satisfied
        sense=Instance.MAXIMIZE,
    )
    b = instance.add_integer_slack_to_inequality(0, 4)
    assert b is None

    # Check that the constraint is removed
    assert instance.constraints == {}
    assert 0 in instance.removed_constraints


def test_add_integer_slack_to_inequality_continuous():
    x = [DecisionVariable.continuous(i, lower=-1.23, upper=4.56) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={0: x[0] + x[1] >= 7.89},
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
        constraints={123: x[0] == 0, 456: x[1] == 1},
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
        constraints={0: x[0] + x[1] >= 7.89},
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
        constraints={0: x[0] + 2 * x[1] <= 3},
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
        constraints={},
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
        constraints={123: x[0] == 0, 456: x[1] == 1},
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
        constraints={0: x[0] + x[1] >= 7.89},
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
        constraints={0: x[0] + 2 * x[1] <= 3},
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
        constraints={0: x[1] == 1},
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
        constraints={0: x[1] == 1},
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


def test_stats_empty_instance():
    instance = Instance.from_components(
        decision_variables=[],
        objective=Function(0),
        constraints={},
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
        constraints={},
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
        constraints={0: x[0] + x[1] == 1, 1: x[1] + x[2] == 1},
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
        constraints={0: x[0] + x[1] <= 5, 1: x[1] + x[2] <= 5},
        sense=Instance.MAXIMIZE,
    )

    instance.log_encode()
    first_encode = instance.decision_variables
    instance.log_encode()
    second_encode = instance.decision_variables
    assert first_encode == second_encode


def _variables_named(instance, name):
    return [
        variable for variable in instance.decision_variables if variable.name == name
    ]


def test_encode_methods_reject_explicit_non_integer_variables():
    variables = [
        DecisionVariable.binary(0, name="x"),
        DecisionVariable.continuous(0, lower=0, upper=3, name="x"),
        DecisionVariable.semi_integer(0, lower=0, upper=3, name="x"),
        DecisionVariable.semi_continuous(0, lower=0, upper=3, name="x"),
    ]

    for x in variables:
        log_instance = Instance.from_components(
            decision_variables=[x],
            objective=x,
            constraints={},
            sense=Instance.MAXIMIZE,
        )
        with pytest.raises(RuntimeError, match="must be integer"):
            log_instance.log_encode({0})
        assert _variables_named(log_instance, "ommx.log_encode") == []

        unary_instance = Instance.from_components(
            decision_variables=[x],
            objective=x,
            constraints={},
            sense=Instance.MAXIMIZE,
        )
        with pytest.raises(RuntimeError, match="must be integer"):
            unary_instance.unary_encode({0})
        assert _variables_named(unary_instance, "ommx.unary_encode") == []


def test_encode_methods_reject_fixed_variables():
    x = DecisionVariable.integer(0, lower=0, upper=3, name="x")

    log_instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Instance.MAXIMIZE,
    ).partial_evaluate({0: 1})
    with pytest.raises(RuntimeError, match="fixed decision variable"):
        log_instance.log_encode({0})
    assert log_instance.fixed_decision_variables() == {0: 1.0}
    assert log_instance.dependent_decision_variable_ids() == set()
    assert _variables_named(log_instance, "ommx.log_encode") == []

    unary_instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Instance.MAXIMIZE,
    ).partial_evaluate({0: 1})
    with pytest.raises(RuntimeError, match="fixed decision variable"):
        unary_instance.unary_encode({0})
    assert unary_instance.fixed_decision_variables() == {0: 1.0}
    assert unary_instance.dependent_decision_variable_ids() == set()
    assert _variables_named(unary_instance, "ommx.unary_encode") == []


def test_log_encode_auto_detect_is_transactional_on_failure():
    x = [
        DecisionVariable.integer(0, lower=0, upper=3, name="x", subscripts=[0]),
        DecisionVariable.integer(1, name="x", subscripts=[1]),
    ]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        sense=Instance.MAXIMIZE,
    )
    before = instance.decision_variables

    with pytest.raises(RuntimeError, match="bound must be finite"):
        instance.log_encode()

    assert instance.decision_variables == before
    assert _variables_named(instance, "ommx.log_encode") == []


def test_unary_encode():
    x = DecisionVariable.integer(0, lower=2, upper=5, name="x")
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Instance.MAXIMIZE,
    )

    instance.unary_encode({0})

    encoded = sorted(
        [
            variable
            for variable in instance.decision_variables
            if variable.name == "ommx.unary_encode"
        ],
        key=lambda variable: variable.id,
    )
    assert len(encoded) == 3
    assert [variable.subscripts for variable in encoded] == [[0, 0], [0, 1], [0, 2]]
    assert instance.objective.almost_equal(
        Function(Linear(terms={variable.id: 1.0 for variable in encoded}, constant=2.0))
    )

    for bit_sum in range(4):
        state = {
            variable.id: (1 if i < bit_sum else 0) for i, variable in enumerate(encoded)
        }
        solution = instance.evaluate(state)
        assert solution.feasible
        assert solution.objective == pytest.approx(2 + bit_sum)


def test_unary_encode_respects_max_range_on_auto_detect():
    x = DecisionVariable.integer(0, lower=0, upper=6, name="x")
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Instance.MAXIMIZE,
    )

    with pytest.raises(RuntimeError, match=r"max_range\(5\)"):
        instance.unary_encode(max_range=5)

    assert [
        variable
        for variable in instance.decision_variables
        if variable.name == "ommx.unary_encode"
    ] == []

    instance.unary_encode(max_range=6)
    assert (
        len(
            [
                variable
                for variable in instance.decision_variables
                if variable.name == "ommx.unary_encode"
            ]
        )
        == 6
    )


def test_unary_encode_auto_detect_is_transactional_on_failure():
    x = [
        DecisionVariable.integer(0, lower=0, upper=3, name="x", subscripts=[0]),
        DecisionVariable.integer(1, lower=0, upper=20, name="x", subscripts=[1]),
    ]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        sense=Instance.MAXIMIZE,
    )
    before = instance.decision_variables

    with pytest.raises(RuntimeError, match=r"max_range\(16\)"):
        instance.unary_encode()

    assert instance.decision_variables == before
    assert _variables_named(instance, "ommx.unary_encode") == []


def test_encoding_methods_validate_atol():
    x = DecisionVariable.integer(0, lower=0, upper=3, name="x")
    log_instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Instance.MAXIMIZE,
    )
    unary_instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Instance.MAXIMIZE,
    )

    with pytest.raises(RuntimeError, match="ATol must be positive"):
        log_instance.log_encode(atol=0.0)
    with pytest.raises(RuntimeError, match="ATol must be positive"):
        unary_instance.unary_encode(atol=0.0)


def test_multiple_unary_encodes():
    x = [
        DecisionVariable.integer(0, lower=0, upper=10, name="x", subscripts=[0]),
        DecisionVariable.integer(1, lower=0, upper=10, name="x", subscripts=[1]),
        DecisionVariable.integer(2, lower=0, upper=10, name="x", subscripts=[2]),
    ]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0],
        constraints={0: x[0] + x[1] <= 5, 1: x[1] + x[2] <= 5},
        sense=Instance.MAXIMIZE,
    )

    instance.unary_encode()
    first_encode = instance.decision_variables
    instance.unary_encode()
    second_encode = instance.decision_variables
    assert first_encode == second_encode


def test_substitute_unary_encode():
    """Custom unary (thermometer) encoding of an integer variable via `substitute`.

    An integer x in [0, 4] is replaced by the linear expression x = sum(b_i).
    Validity (a "thermometer" pattern) is enforced by ordering constraints
    b_{i+1} <= b_i, which the user adds separately with `add_constraint`.
    """
    x = DecisionVariable.integer(0, lower=0, upper=4, name="x")
    b = [DecisionVariable.binary(i + 1, name="b", subscripts=[i]) for i in range(4)]
    instance = Instance.from_components(
        decision_variables=[x, *b],
        objective=x,
        constraints={},
        sense=Instance.MAXIMIZE,
    )

    instance.substitute({0: b[0] + b[1] + b[2] + b[3]})  # x = sum(b_i)
    for i in range(3):
        instance.add_constraint(b[i + 1] - b[i] <= 0)  # b_{i+1} <= b_i

    # Thermometer assignments are feasible and decode to the original integer.
    for k in range(5):  # number of bits set to 1
        state = {i + 1: (1 if i < k else 0) for i in range(4)}
        solution = instance.evaluate(state)
        assert solution.feasible
        assert solution.objective == pytest.approx(k)

    # A non-thermometer assignment violates the ordering constraints.
    assert not instance.evaluate({1: 0, 2: 1, 3: 0, 4: 0}).feasible


def test_substitute_one_hot_encode():
    """Custom one-hot encoding of an integer variable via `substitute`.

    An integer x in [0, 3] is replaced by x = sum(v * b_v), with a one-hot
    constraint sum(b_v) == 1 added separately by the user. Unlike unary, the
    decode is still linear but the structural constraint is an equality.
    """
    x = DecisionVariable.integer(0, lower=0, upper=3, name="x")
    b = [DecisionVariable.binary(i + 1, name="b", subscripts=[i]) for i in range(4)]
    instance = Instance.from_components(
        decision_variables=[x, *b],
        objective=x,
        constraints={},
        sense=Instance.MAXIMIZE,
    )

    instance.substitute({0: b[1] + 2 * b[2] + 3 * b[3]})  # x = sum(v * b_v)
    instance.add_constraint((b[0] + b[1] + b[2] + b[3]) == 1)  # one-hot constraint

    # One-hot assignments are feasible and decode to the selected domain value.
    for v in range(4):
        state = {i + 1: (1 if i == v else 0) for i in range(4)}
        solution = instance.evaluate(state)
        assert solution.feasible
        assert solution.objective == pytest.approx(v)

    # Multiple bits set or all bits unset violate the one-hot constraint.
    assert not instance.evaluate({1: 1, 2: 1, 3: 0, 4: 0}).feasible
    assert not instance.evaluate({1: 0, 2: 0, 3: 0, 4: 0}).feasible


def test_substitute_recursive_assignment_raises():
    """`substitute` rejects an assignment that depends on the variable itself."""
    x = [DecisionVariable.integer(i, lower=0, upper=3, name="x") for i in range(2)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0] + x[1],
        constraints={},
        sense=Instance.MAXIMIZE,
    )
    objective_before = Function(instance.objective)
    decision_variable_ids_before = {v.id for v in instance.decision_variables}

    with pytest.raises(ValueError):
        instance.substitute({0: x[0] + x[1]})

    assert {v.id for v in instance.decision_variables} == decision_variable_ids_before
    assert instance.objective.almost_equal(objective_before)


def test_parametric_instance_substitute_with_parameterized_rhs():
    """ParametricInstance.substitute keeps parameters symbolic until materialization."""
    x = DecisionVariable.integer(0, lower=0, upper=10, name="x")
    y = DecisionVariable.binary(1, name="y")
    p = Parameter(100, name="p")
    parametric = ParametricInstance.from_components(
        decision_variables=[x, y],
        parameters=[p],
        objective=x + p * y,
        constraints={0: x <= p + 1},
        sense=Instance.MAXIMIZE,
    )

    parametric.substitute({0: p * y + 1})

    instance = parametric.with_parameters({100: 2.0})
    assert instance.objective.almost_equal(Function(4 * y + 1))
    solution = instance.evaluate({1: 1})
    assert solution.feasible
    assert solution.objective == pytest.approx(5.0)


def test_parametric_instance_substitute_recursive_assignment_raises():
    x = [DecisionVariable.integer(i, lower=0, upper=3, name="x") for i in range(2)]
    p = Parameter(100, name="p")
    parametric = ParametricInstance.from_components(
        decision_variables=x,
        parameters=[p],
        objective=x[0] + p * x[1],
        constraints={},
        sense=Instance.MAXIMIZE,
    )
    objective_before = Function(parametric.objective)

    with pytest.raises(ValueError):
        parametric.substitute({0: x[0] + p})

    assert parametric.objective.almost_equal(objective_before)


def test_parametric_instance_substitute_parameter_id_raises():
    x = DecisionVariable.binary(0, name="x")
    p = Parameter(100, name="p")
    parametric = ParametricInstance.from_components(
        decision_variables=[x],
        parameters=[p],
        objective=x + p,
        constraints={},
        sense=Instance.MAXIMIZE,
    )
    objective_before = Function(parametric.objective)

    with pytest.raises(ValueError, match="Cannot substitute parameter"):
        parametric.substitute({100: x})

    assert parametric.objective.almost_equal(objective_before)


def test_parametric_instance_substitute_undefined_rhs_id_raises():
    x = DecisionVariable.binary(0, name="x")
    parametric = ParametricInstance.from_components(
        decision_variables=[x],
        parameters=[],
        objective=x,
        constraints={},
        sense=Instance.MAXIMIZE,
    )
    objective_before = Function(parametric.objective)

    with pytest.raises(
        ValueError, match="Undefined variable ID is used in substitution"
    ):
        parametric.substitute({0: DecisionVariable.binary(999)})

    assert parametric.objective.almost_equal(objective_before)
