"""Test SOS1 functionality with valid constraints."""

from ommx.v1 import Instance, DecisionVariable, Sos1, ConstraintHints
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter


def test_sos1_constraint_functionality():
    """Test that SOS1 constraints work with valid constraint references."""
    # Create decision variables
    x = [DecisionVariable.continuous(i, lower=0, upper=1) for i in range(1, 3)]
    x.append(DecisionVariable.binary(3))
    selectors = [DecisionVariable.binary(i) for i in (4, 5)]

    # A nonzero optimum exercises selector reconstruction.
    objective = x[0] + 2 * x[1] + x[2]

    cardinality = (sum(selectors) + x[2] <= 1).set_id(1)

    # Create additional constraints for the SOS1 big-M
    bigm1 = (x[0] <= selectors[0]).set_id(2)
    bigm2 = (x[1] <= selectors[1]).set_id(3)

    # Create SOS1 hint with valid constraint references
    sos1_hint = Sos1(
        binary_constraint_id=1, big_m_constraint_ids=[2, 3], variables=[1, 2, 3]
    )
    constraint_hints = ConstraintHints(sos1_constraints=[sos1_hint])

    instance = Instance.from_components(
        decision_variables=x + selectors,
        objective=objective,
        constraints=[cardinality, bigm1, bigm2],
        sense=Instance.MAXIMIZE,
        constraint_hints=constraint_hints,
    )

    # Create adapter and verify SOS1 constraint is added
    adapter = OMMXPySCIPOptAdapter(instance, use_sos1="auto")
    model = adapter.solver_input

    # Check that SOS1 constraint was created
    constraint_names = [cons.name for cons in model.getConss()]
    sos1_names = [name for name in constraint_names if name.startswith("sos1_")]

    assert len(sos1_names) > 0, "SOS1 constraint should be created"

    # Check that referenced constraints are excluded
    assert "1" not in constraint_names, "Referenced constraint should be excluded"
    assert "2" not in constraint_names, "Referenced Big-M constraint should be excluded"
    assert "3" not in constraint_names, "Referenced Big-M constraint should be excluded"

    # Check the returned solution against the original formulation as well.
    model.optimize()
    solution = adapter.decode(model)
    assert solution.feasible
    assert solution.objective == 2

    # The important part is that SOS1 constraint was correctly added
    assert len(sos1_names) == 1, "Exactly one SOS1 constraint should be created"


def test_sos1_constraint_naming():
    """Test that SOS1 constraints get proper names."""
    # Create decision variables
    x = [DecisionVariable.continuous(i, lower=0, upper=1) for i in range(1, 3)]
    selectors = [DecisionVariable.binary(i) for i in (3, 4)]

    objective = sum(x)

    # Create constraints for SOS1 to reference
    constraint1 = (selectors[0] + selectors[1] <= 1).set_id(10)
    bigm1 = (x[0] <= selectors[0]).set_id(20)
    bigm2 = (x[1] <= selectors[1]).set_id(30)

    # Test SOS1 with cardinality and Big-M constraints.
    sos1_hint = Sos1(
        binary_constraint_id=10, big_m_constraint_ids=[20, 30], variables=[1, 2]
    )
    constraint_hints = ConstraintHints(sos1_constraints=[sos1_hint])

    instance = Instance.from_components(
        decision_variables=x + selectors,
        objective=objective,
        constraints=[constraint1, bigm1, bigm2],
        sense=Instance.MINIMIZE,
        constraint_hints=constraint_hints,
    )

    adapter = OMMXPySCIPOptAdapter(instance, use_sos1="auto")
    model = adapter.solver_input

    # Check SOS1 constraint naming
    constraint_names = [cons.name for cons in model.getConss()]
    sos1_names = [name for name in constraint_names if name.startswith("sos1_")]

    assert len(sos1_names) == 1
    # The name should include binary constraint ID and big-M constraint IDs
    expected_name = "sos1_10_20_30"
    assert sos1_names[0] == expected_name, (
        f"Expected {expected_name}, got {sos1_names[0]}"
    )


def test_sos1_constraint_naming_no_bigm():
    """Test SOS1 constraint naming when no big-M constraints are specified."""
    # Create decision variables
    x = [DecisionVariable.binary(i) for i in range(1, 3)]

    objective = sum(x)

    # Create constraint for SOS1 to reference (just binary constraint)
    constraint1 = (sum(x) <= 1).set_id(5)  # type: ignore

    # SOS1 with only binary constraint, no big-M
    sos1_hint = Sos1(
        binary_constraint_id=5,
        big_m_constraint_ids=[],  # No big-M constraints
        variables=[1, 2],
    )
    constraint_hints = ConstraintHints(sos1_constraints=[sos1_hint])

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        constraints=[constraint1],
        sense=Instance.MINIMIZE,
        constraint_hints=constraint_hints,
    )

    adapter = OMMXPySCIPOptAdapter(instance, use_sos1="auto")
    model = adapter.solver_input

    # Check SOS1 constraint naming
    constraint_names = [cons.name for cons in model.getConss()]
    sos1_names = [name for name in constraint_names if name.startswith("sos1_")]

    assert len(sos1_names) == 1
    # When no big-M constraints, should just be sos1_{binary_constraint_id}
    expected_name = "sos1_5"
    assert sos1_names[0] == expected_name, (
        f"Expected {expected_name}, got {sos1_names[0]}"
    )
