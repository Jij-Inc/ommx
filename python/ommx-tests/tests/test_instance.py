from ommx.v1 import Instance, DecisionVariable, Function
import math
import pytest
from ommx.v1.constraint_hints_pb2 import ConstraintHints
from ommx.v1.one_hot_pb2 import OneHot
from ommx.v1.k_hot_pb2 import KHot


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
        == "The bound of `f(x)` in inequality constraint(ConstraintID(0)) `f(x) <= 0` is positive: Bound { lower: 1.0, upper: 10.0 }"
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
    assert instance.get_constraints() == []
    removed = instance.get_removed_constraints()[0]
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
        == "The bound of `f(x)` in inequality constraint(ConstraintID(0)) `f(x) <= 0` is positive: Bound { lower: 1.0, upper: 10.0 }"
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
    assert instance.get_constraints() == []
    removed = instance.get_removed_constraints()[0]
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


def test_constraint_hints_one_hot_constraints():
    """
    Test that constraint_hints.one_hot_constraints() contains one-hot constraints from
    both constraint_hints.raw.one_hot_constraints and constraint_hints.raw.k_hot_constraints[1].
    """
    # Create an Instance with binary decision variables
    x = [DecisionVariable.binary(i) for i in range(5)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints=[],
        sense=Instance.MAXIMIZE,
    )

    # Create constraint hints with one-hot constraints in both sources
    # Access the raw protobuf message
    if not instance.raw.HasField("constraint_hints"):
        instance.raw.constraint_hints.Clear()  # Initialize if not present

    # Add constraints to the deprecated one_hot_constraints field
    one_hot1 = instance.raw.constraint_hints.one_hot_constraints.add()
    one_hot1.constraint_id = 1
    one_hot1.decision_variables.extend([0, 1, 2])

    one_hot2 = instance.raw.constraint_hints.one_hot_constraints.add()
    one_hot2.constraint_id = 2
    one_hot2.decision_variables.extend([2, 3, 4])

    # Add constraints to the k_hot_constraints[1] field
    k_hot_list = instance.raw.constraint_hints.k_hot_constraints[1]
    k_hot1 = k_hot_list.constraints.add()
    k_hot1.constraint_id = 3
    k_hot1.decision_variables.extend([0, 3, 4])
    k_hot1.num_hot_vars = 1

    # Add a duplicate constraint (same ID as one_hot1) to ensure no duplicates
    k_hot2 = k_hot_list.constraints.add()
    k_hot2.constraint_id = 1  # Same as one_hot1
    k_hot2.decision_variables.extend([0, 1, 2])
    k_hot2.num_hot_vars = 1

    # Get the constraint hints and call one_hot_constraints()
    constraint_hints = instance.constraint_hints()
    one_hot_constraints = constraint_hints.one_hot_constraints()

    # Verify the results
    # Should have 3 constraints (one_hot1, one_hot2, k_hot1)
    # k_hot2 should be excluded as it has the same ID as one_hot1
    assert len(one_hot_constraints) == 3

    # Verify constraint IDs
    constraint_ids = {c.raw.constraint_id for c in one_hot_constraints}
    assert constraint_ids == {1, 2, 3}

    # Verify decision variables for each constraint
    for c in one_hot_constraints:
        if c.raw.constraint_id == 1:
            assert set(c.raw.decision_variables) == {0, 1, 2}
        elif c.raw.constraint_id == 2:
            assert set(c.raw.decision_variables) == {2, 3, 4}
        elif c.raw.constraint_id == 3:
            assert set(c.raw.decision_variables) == {0, 3, 4}


def test_constraint_hints_k_hot_constraints():
    """
    Test that constraint_hints.k_hot_constraints() contains all elements from
    constraint_hints.raw.k_hot_constraints together with constraint_hints.raw.one_hot_constraints
    in key 1.
    """
    # Create an Instance with binary decision variables
    x = [DecisionVariable.binary(i) for i in range(5)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints=[],
        sense=Instance.MAXIMIZE,
    )

    # Create constraint hints with various constraints
    # Access the raw protobuf message
    if not instance.raw.HasField("constraint_hints"):
        instance.raw.constraint_hints.Clear()  # Initialize if not present

    # Add constraints to the deprecated one_hot_constraints field
    one_hot1 = instance.raw.constraint_hints.one_hot_constraints.add()
    one_hot1.constraint_id = 1
    one_hot1.decision_variables.extend([0, 1, 2])

    # Add constraints to the k_hot_constraints[1] field (k=1)
    k_hot_list1 = instance.raw.constraint_hints.k_hot_constraints[1]
    k_hot1 = k_hot_list1.constraints.add()
    k_hot1.constraint_id = 2
    k_hot1.decision_variables.extend([2, 3, 4])
    k_hot1.num_hot_vars = 1

    # Add constraints to the k_hot_constraints[2] field (k=2)
    k_hot_list2 = instance.raw.constraint_hints.k_hot_constraints[2]
    k_hot2 = k_hot_list2.constraints.add()
    k_hot2.constraint_id = 3
    k_hot2.decision_variables.extend([0, 1, 2, 3])
    k_hot2.num_hot_vars = 2

    # Add a duplicate constraint (same ID as one_hot1) to k_hot_constraints[1]
    k_hot3 = k_hot_list1.constraints.add()
    k_hot3.constraint_id = 1  # Same as one_hot1
    k_hot3.decision_variables.extend([0, 1, 2])
    k_hot3.num_hot_vars = 1

    # Get the constraint hints and call k_hot_constraints()
    import pyo3

    constraint_hints = instance.constraint_hints(pyo3._python.Python.acquire_gil())
    k_hot_constraints = constraint_hints.k_hot_constraints()

    # Verify the results
    # Should have entries for k=1 and k=2
    assert set(k_hot_constraints.keys()) == {1, 2}

    # k=1 should have 2 constraints (one_hot1 and k_hot1)
    # k_hot3 should be excluded as it has the same ID as one_hot1
    assert len(k_hot_constraints[1]) == 2

    # k=2 should have 1 constraint (k_hot2)
    assert len(k_hot_constraints[2]) == 1

    # Verify constraint IDs for k=1
    k1_constraint_ids = {c.raw.constraint_id for c in k_hot_constraints[1]}
    assert k1_constraint_ids == {1, 2}

    # Verify constraint IDs for k=2
    k2_constraint_ids = {c.raw.constraint_id for c in k_hot_constraints[2]}
    assert k2_constraint_ids == {3}

    # Verify decision variables for each constraint
    for c in k_hot_constraints[1]:
        if c.raw.constraint_id == 1:
            assert set(c.raw.decision_variables) == {0, 1, 2}
        elif c.raw.constraint_id == 2:
            assert set(c.raw.decision_variables) == {2, 3, 4}

    for c in k_hot_constraints[2]:
        assert c.raw.constraint_id == 3
        assert set(c.raw.decision_variables) == {0, 1, 2, 3}
