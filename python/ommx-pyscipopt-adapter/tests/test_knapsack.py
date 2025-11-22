"""Test knapsack problem with pyscipopt adapter"""
import pytest
from ommx.v1 import Instance, DecisionVariable, Solution
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter


def test_knapsack_problem():
    """
    Test the classic knapsack problem example from the adapter docstring.

    Items with profit p and weight w:
    - Item 0: p=10, w=11
    - Item 1: p=13, w=15
    - Item 2: p=18, w=20
    - Item 3: p=32, w=35
    - Item 4: p=7,  w=10
    - Item 5: p=15, w=33

    Capacity: 47

    Expected optimal solution:
    - Select items 0 and 3
    - Total profit: 10 + 32 = 42
    - Total weight: 11 + 35 = 46 <= 47
    """
    # Item profits and weights
    p = [10, 13, 18, 32, 7, 15]
    w = [11, 15, 20, 35, 10, 33]
    capacity = 47

    # Create binary decision variables (one per item)
    x = [DecisionVariable.binary(i) for i in range(6)]

    # Create instance
    # Maximize: sum of profits
    # Subject to: sum of weights <= capacity
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(p[i] * x[i] for i in range(6)),
        constraints=[(sum(w[i] * x[i] for i in range(6)) <= capacity).set_id(0)],
        sense=Instance.MAXIMIZE,
    )

    # Solve
    solution = OMMXPySCIPOptAdapter.solve(instance)

    # Verify solution
    assert solution.feasible, "Solution should be feasible"
    assert solution.optimality == Solution.OPTIMAL, "Should find optimal solution"

    # Expected: x0=1, x1=0, x2=0, x3=1, x4=0, x5=0
    expected_selection = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]
    for i in range(6):
        actual = solution.state.entries[i]
        assert actual == pytest.approx(expected_selection[i]), \
            f"Item {i}: expected {expected_selection[i]}, got {actual}"

    # Verify objective value (total profit)
    expected_profit = p[0] + p[3]  # 10 + 32 = 42
    assert solution.objective == pytest.approx(42.0), \
        f"Expected profit 42, got {solution.objective}"

    # Verify constraint value (capacity constraint)
    # Constraint: sum(w[i] * x[i]) - capacity <= 0
    # With x0=1, x3=1: w[0] + w[3] = 11 + 35 = 46
    # Constraint value: 46 - 47 = -1 <= 0 (feasible)
    constraint_value = solution.get_constraint_value(0)
    assert constraint_value == pytest.approx(-1.0), \
        f"Expected constraint value -1.0, got {constraint_value}"

    # Verify total weight doesn't exceed capacity
    total_weight = sum(w[i] * expected_selection[i] for i in range(6))
    assert total_weight <= capacity, \
        f"Total weight {total_weight} exceeds capacity {capacity}"
    assert total_weight == pytest.approx(46.0), \
        f"Expected total weight 46, got {total_weight}"


def test_knapsack_small():
    """Test a smaller knapsack problem for quick verification"""
    # Simple 3-item knapsack
    # Items: (value=5, weight=3), (value=3, weight=2), (value=4, weight=4)
    # Capacity: 5
    # Optimal: select items 0 and 1 (value=8, weight=5)

    p = [5, 3, 4]
    w = [3, 2, 4]
    capacity = 5

    x = [DecisionVariable.binary(i) for i in range(3)]

    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(p[i] * x[i] for i in range(3)),
        constraints=[(sum(w[i] * x[i] for i in range(3)) <= capacity).set_id(0)],
        sense=Instance.MAXIMIZE,
    )

    solution = OMMXPySCIPOptAdapter.solve(instance)

    assert solution.feasible
    assert solution.optimality == Solution.OPTIMAL

    # Expected: x0=1, x1=1, x2=0
    expected = [1.0, 1.0, 0.0]
    for i in range(3):
        assert solution.state.entries[i] == pytest.approx(expected[i])

    # Total value should be 8
    assert solution.objective == pytest.approx(8.0)


def test_knapsack_no_solution_fits():
    """Test knapsack where no single item fits"""
    # All items too heavy
    p = [10, 20, 30]
    w = [15, 25, 35]
    capacity = 10  # Too small for any item

    x = [DecisionVariable.binary(i) for i in range(3)]

    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(p[i] * x[i] for i in range(3)),
        constraints=[(sum(w[i] * x[i] for i in range(3)) <= capacity).set_id(0)],
        sense=Instance.MAXIMIZE,
    )

    solution = OMMXPySCIPOptAdapter.solve(instance)

    assert solution.feasible
    assert solution.optimality == Solution.OPTIMAL

    # Should select no items
    for i in range(3):
        assert solution.state.entries[i] == pytest.approx(0.0)

    # Total value should be 0
    assert solution.objective == pytest.approx(0.0)


if __name__ == "__main__":
    # Run tests directly
    print("Running knapsack problem test...")
    test_knapsack_problem()
    print("✓ Main knapsack test passed")

    print("\nRunning small knapsack test...")
    test_knapsack_small()
    print("✓ Small knapsack test passed")

    print("\nRunning no-fit knapsack test...")
    test_knapsack_no_solution_fits()
    print("✓ No-fit knapsack test passed")

    print("\n✅ All knapsack tests passed!")
