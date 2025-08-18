"""Test PySCIPOpt adapter behavior with partial_evaluate and SOS1 constraints."""

import pytest
from ommx.v1 import Instance, DecisionVariable, Sos1, ConstraintHints, State
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter


@pytest.fixture
def sos1_instance_setup():
    """Common setup for SOS1 constraint tests."""
    # Create continuous decision variables x0, x1, x2
    x = [DecisionVariable.continuous(i, lower=0, upper=10) for i in range(1, 4)]
    # Create binary auxiliary variables y0, y1, y2
    y = [DecisionVariable.binary(i, name="y", subscripts=[i - 1]) for i in range(4, 7)]
    # Create independent variable z (ID=7)
    z = DecisionVariable.continuous(7, lower=0, upper=5, name="z")

    objective = sum(x) + 0.5 * z

    # SOS1 constraint setup: y0 + y1 + y2 <= 1 (binary constraint)
    binary_constraint = (y[0] + y[1] + y[2] <= 1).set_id(1)
    # Big-M constraints: x0 <= 10*y0, x1 <= 10*y1, x2 <= 10*y2
    big_m1 = (x[0] <= 10 * y[0]).set_id(2)
    big_m2 = (x[1] <= 10 * y[1]).set_id(3)
    big_m3 = (x[2] <= 10 * y[2]).set_id(4)

    # Independent constraint not related to SOS1
    independent_constraint = (z >= 1).set_id(5)

    sos1_hint = Sos1(
        binary_constraint_id=1,
        big_m_constraint_ids=[2, 3, 4],
        variables=[1, 2, 3],  # x0, x1, x2 (continuous variables)
    )
    constraint_hints = ConstraintHints(sos1_constraints=[sos1_hint])

    return Instance.from_components(
        decision_variables=x + y + [z],
        objective=objective,
        constraints=[binary_constraint, big_m1, big_m2, big_m3, independent_constraint],
        sense=Instance.MINIMIZE,
        constraint_hints=constraint_hints,
    )


def test_adapter_handles_sos1_variable_fixed_nonzero(sos1_instance_setup):
    """Test that PySCIPOpt adapter handles instances when SOS1 variable is fixed to non-zero value."""
    instance = sos1_instance_setup

    # Apply partial_evaluate - fix x1 (ID=2) to 5.0 (non-zero)
    initial_state = State({2: 5.0})
    evaluated_instance = instance.partial_evaluate(initial_state)

    # Adapter Test: Should solve without SOS1 reference errors
    solution = OMMXPySCIPOptAdapter.solve(evaluated_instance)

    # Verify all decision variables are included in solution
    decision_var_ids = {var.id for var in instance.decision_variables}
    solution_var_ids = {var.id for var in solution.decision_variables}
    assert decision_var_ids == solution_var_ids, (
        "Solution should contain all decision variables"
    )

    # Verify fixed variable has the correct value
    fixed_var = solution.get_decision_variable_by_id(2)
    assert fixed_var.value == 5.0, "Fixed variable should have the specified value"


def test_adapter_handles_sos1_variable_fixed_to_zero(sos1_instance_setup):
    """Test adapter behavior when SOS1 variable is fixed to zero."""
    instance = sos1_instance_setup

    # Apply partial_evaluate - fix x2 (ID=3) to 0
    initial_state = State({3: 0.0})
    evaluated_instance = instance.partial_evaluate(initial_state)

    # Adapter Test: Should solve without SOS1 reference errors
    solution = OMMXPySCIPOptAdapter.solve(evaluated_instance)

    # Verify all decision variables are included in solution
    decision_var_ids = {var.id for var in instance.decision_variables}
    solution_var_ids = {var.id for var in solution.decision_variables}
    assert decision_var_ids == solution_var_ids, (
        "Solution should contain all decision variables"
    )

    # Verify fixed variable has the correct value
    fixed_var = solution.get_decision_variable_by_id(3)
    assert fixed_var.value == 0.0, "Fixed variable should have the specified value"


def test_adapter_handles_independent_variable_fixed(sos1_instance_setup):
    """Test adapter behavior when independent (non-SOS1) variable is fixed."""
    instance = sos1_instance_setup

    # Apply partial_evaluate - fix independent variable z (ID=7)
    initial_state = State({7: 2.0})
    evaluated_instance = instance.partial_evaluate(initial_state)

    # Adapter Test: Should solve without errors
    solution = OMMXPySCIPOptAdapter.solve(evaluated_instance)

    # Verify all decision variables are included in solution
    decision_var_ids = {var.id for var in instance.decision_variables}
    solution_var_ids = {var.id for var in solution.decision_variables}
    assert decision_var_ids == solution_var_ids, (
        "Solution should contain all decision variables"
    )

    # Verify fixed variable has the correct value
    fixed_var = solution.get_decision_variable_by_id(7)
    assert fixed_var.value == 2.0, "Fixed variable should have the specified value"
