from ommx.v1 import Instance, DecisionVariable, State
import ommx
import pytest


def test_default_atol_get_set():
    """Test basic get/set functionality of default ATol."""
    # Get initial default ATol (should be 1e-6)
    initial_atol = ommx.get_default_atol()
    assert initial_atol == 1e-6

    # Set ATol to different value
    ommx.set_default_atol(1e-4)

    # Verify the change took effect
    new_atol = ommx.get_default_atol()
    assert new_atol == 1e-4

    # Reset ATol back to original value
    ommx.set_default_atol(initial_atol)

    # Verify reset worked
    assert ommx.get_default_atol() == initial_atol


def test_default_atol_constraint_evaluation():
    """Test that ATol default value affects constraint evaluation.

    Creates an instance with a constraint that is violated by 1e-5,
    which should be infeasible with default ATol (1e-6) but feasible
    when ATol is set to 1e-4.
    """
    # Create a simple instance: minimize x subject to x <= 0
    x = DecisionVariable.continuous(1, lower=-10, upper=10)

    # Create constraint: x <= 0 using expression syntax
    constraint = (x <= 0).set_id(1)

    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,  # minimize x
        constraints=[constraint],
        sense=Instance.MINIMIZE,
    )

    # Create a state that violates the constraint by exactly 1e-5
    # x = 1e-5, so x <= 0 is violated by 1e-5
    state = State({1: 1e-5})

    # Get initial default ATol (should be 1e-6)
    initial_atol = ommx.get_default_atol()
    assert initial_atol == 1e-6

    # Evaluate with default ATol - should be infeasible
    solution = instance.evaluate(state)

    # The constraint should be infeasible because violation (1e-5) > ATol (1e-6)
    assert not solution.feasible

    # Set ATol to 1e-4 (larger than the violation)
    ommx.set_default_atol(1e-4)

    # Verify the change took effect
    new_atol = ommx.get_default_atol()
    assert new_atol == 1e-4

    # Evaluate again - should now be feasible
    solution_with_larger_atol = instance.evaluate(state)

    # The constraint should now be feasible because violation (1e-5) <= ATol (1e-4)
    assert solution_with_larger_atol.feasible

    # Reset ATol back to original value for other tests
    ommx.set_default_atol(initial_atol)

    # Verify reset worked
    assert ommx.get_default_atol() == initial_atol


def test_set_default_atol_validation():
    """Test that set_default_atol validates input values."""
    initial_atol = ommx.get_default_atol()

    # Should accept positive values
    ommx.set_default_atol(1e-3)
    assert ommx.get_default_atol() == 1e-3

    # Should reject zero
    with pytest.raises(Exception):  # Should raise an error for non-positive values
        ommx.set_default_atol(0.0)

    # Should reject negative values
    with pytest.raises(Exception):
        ommx.set_default_atol(-1e-6)

    # Reset for other tests
    ommx.set_default_atol(initial_atol)


def test_instance_evaluate_with_custom_atol():
    """Test that instance evaluation respects custom atol parameter.

    Tests constraint x1 <= 1.0 with state x1 = 1.0 + 1e-6:
    - With atol=1e-3: violation (1e-6) < atol, should be feasible
    - With atol=1e-9: violation (1e-6) > atol, should be infeasible
    """
    # Create a decision variable
    x1 = DecisionVariable.continuous(1, lower=0, upper=10)

    # Create constraint: x1 <= 1.0, which is equivalent to x1 - 1.0 <= 0
    constraint = (x1 <= 1.0).set_id(1)

    # Create an instance with the constraint
    instance = Instance.from_components(
        decision_variables=[x1],
        objective=x1,
        constraints=[constraint],
        sense=Instance.MINIMIZE,
    )

    # Create a state where x1 = 1.0 + 1e-6 (violates constraint by 1e-6)
    state = State({1: 1.0 + 1e-6})

    # Test with large atol (1e-3): violation (1e-6) is within tolerance
    solution_large_atol = instance.evaluate(state, atol=1e-3)
    # With atol=1e-3, violation of 1e-6 should be considered feasible
    assert solution_large_atol.feasible, "Solution should be feasible with large atol"

    # Test with small atol (1e-9): violation (1e-6) exceeds tolerance
    solution_small_atol = instance.evaluate(state, atol=1e-9)
    # With atol=1e-9, violation of 1e-6 should be considered infeasible
    assert not solution_small_atol.feasible, (
        "Solution should be infeasible with small atol"
    )
