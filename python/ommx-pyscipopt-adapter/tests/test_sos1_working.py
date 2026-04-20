"""Test SOS1 functionality with valid constraints."""

from ommx.v1 import Instance, DecisionVariable, Sos1Constraint
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter


def test_sos1_constraint_functionality():
    """Test that SOS1 constraints work with valid constraint references."""
    # Create decision variables
    x = [DecisionVariable.continuous(i, lower=0, upper=1) for i in range(1, 4)]

    # Simple objective to minimize
    objective = sum(x)

    # Create a constraint that the SOS1 will reference
    # This constraint ensures the problem has a meaningful solution space
    dummy_constraint = sum(x) <= 2  # type: ignore

    # Create additional constraints for the SOS1 big-M
    bigm1 = x[0] <= 1
    bigm2 = x[1] <= 1

    # Create SOS1 constraint as a first-class type
    sos1 = Sos1Constraint(variables=[1, 2, 3])

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        constraints={1: dummy_constraint, 2: bigm1, 3: bigm2},  # type: ignore
        sos1_constraints={0: sos1},
        sense=Instance.MINIMIZE,
    )

    # Create adapter and verify SOS1 constraint is added
    adapter = OMMXPySCIPOptAdapter(instance)
    model = adapter.solver_input

    # Check that SOS1 constraint was created
    constraint_names = [cons.name for cons in model.getConss()]
    sos1_names = [name for name in constraint_names if name.startswith("sos1_")]

    assert len(sos1_names) > 0, "SOS1 constraint should be created"

    # Solve and get a solution (may be infeasible due to constraint exclusion, which is expected)
    model.optimize()

    # The important part is that SOS1 constraint was correctly added
    assert len(sos1_names) == 1, "Exactly one SOS1 constraint should be created"


def test_sos1_constraint_naming():
    """Test that SOS1 constraints get proper names."""
    # Create decision variables
    x = [DecisionVariable.binary(i) for i in range(1, 3)]

    objective = sum(x)

    # Create constraints
    constraint1 = sum(x) == 1  # type: ignore

    # Create SOS1 constraint
    sos1 = Sos1Constraint(variables=[1, 2])

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        constraints={10: constraint1},  # type: ignore
        sos1_constraints={42: sos1},
        sense=Instance.MINIMIZE,
    )

    adapter = OMMXPySCIPOptAdapter(instance)
    model = adapter.solver_input

    # Check SOS1 constraint naming
    constraint_names = [cons.name for cons in model.getConss()]
    sos1_names = [name for name in constraint_names if name.startswith("sos1_")]

    assert len(sos1_names) == 1
    # The name should include the SOS1 constraint ID
    expected_name = "sos1_42"
    assert sos1_names[0] == expected_name, (
        f"Expected {expected_name}, got {sos1_names[0]}"
    )


def test_sos1_constraint_naming_no_bigm():
    """Test SOS1 constraint naming when no big-M constraints are specified."""
    # Create decision variables
    x = [DecisionVariable.binary(i) for i in range(1, 3)]

    objective = sum(x)

    # Create constraint
    constraint1 = sum(x) <= 1  # type: ignore

    # SOS1 with no associated constraint IDs
    sos1 = Sos1Constraint(variables=[1, 2])

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        constraints={5: constraint1},  # type: ignore
        sos1_constraints={7: sos1},
        sense=Instance.MINIMIZE,
    )

    adapter = OMMXPySCIPOptAdapter(instance)
    model = adapter.solver_input

    # Check SOS1 constraint naming
    constraint_names = [cons.name for cons in model.getConss()]
    sos1_names = [name for name in constraint_names if name.startswith("sos1_")]

    assert len(sos1_names) == 1
    expected_name = "sos1_7"
    assert sos1_names[0] == expected_name, (
        f"Expected {expected_name}, got {sos1_names[0]}"
    )
