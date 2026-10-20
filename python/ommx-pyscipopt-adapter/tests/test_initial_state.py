import pytest

from ommx import DecisionVariable, Instance, NamedState, Sense, State

from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter


@pytest.mark.parametrize("variable_ids", [(31, 107), (900, 41)])
def test_named_initial_state_is_stored_before_optimization(variable_ids):
    x = DecisionVariable.integer(
        variable_ids[0], lower=0, upper=5, name="x", subscripts=[0]
    )
    y = DecisionVariable.integer(
        variable_ids[1], lower=0, upper=5, name="x", subscripts=[1]
    )
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=2 * x + y,
        constraints={0: x + y <= 5},
        sense=Sense.Maximize,
    )
    # The same user input works even when IDs and their ordering change.
    values: NamedState = {"x": {(0,): 1.0, (1,): 4.0}}
    initial_state = instance.state_from_named_values(values)
    adapter = OMMXPySCIPOptAdapter(instance, initial_state=initial_state)
    model = adapter.solver_input

    # Check SCIP's stored solution before solving, so a solver-found optimum
    # cannot mask incorrect initial-value routing.
    assert model.getNSols() == 1
    initial_sol = model.getSols()[0]
    variables = {variable.name: variable for variable in model.getVars()}
    assert model.getSolVal(initial_sol, variables[str(x.id)]) == 1.0
    assert model.getSolVal(initial_sol, variables[str(y.id)]) == 4.0
    assert model.getSolObjVal(initial_sol) == 6.0

    model.optimize()
    solution = adapter.decode(model)

    assert solution.feasible
    assert solution.state.entries == {x.id: 5.0, y.id: 0.0}
    assert solution.objective == 10.0


def test_adapter_class_with_initial_state():
    # Objective function: x - y (Maximize)
    # x, y: integer variables with range [0, 5]
    # Constraint: x + y <= 5
    # Initial state: x = 3, y = 2
    # Optimal solution: x = 5, y = 0
    x = DecisionVariable.integer(1, lower=0, upper=5)
    y = DecisionVariable.integer(2, lower=0, upper=5)

    ommx_instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x - y,
        constraints={0: x + y <= 5},
        sense=Sense.Maximize,
    )
    initial_state = State(
        entries={
            1: 3.0,
            2: 2.0,
        }
    )
    adapter = OMMXPySCIPOptAdapter(ommx_instance, initial_state=initial_state)
    model = adapter.solver_input
    sols = model.getSols()
    sol = sols[0]
    var_dict = {var.name: var for var in model.getVars()}

    # Verify the initial state was correctly set in the model
    assert model.getSolVal(sol, var_dict["1"]) == 3.0
    assert model.getSolVal(sol, var_dict["2"]) == 2.0


def test_solve_with_initial_state():
    # Objective function: x - y (Maximize)
    # x, y: integer variables with range [0, 5]
    # Constraint: x + y <= 5
    # Initial state: x = 3, y = 2
    # Optimal solution: x = 5, y = 0
    x = DecisionVariable.integer(1, lower=0, upper=5)
    y = DecisionVariable.integer(2, lower=0, upper=5)

    ommx_instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x - y,
        constraints={0: x + y <= 5},
        sense=Sense.Maximize,
    )
    initial_state = State(
        entries={
            1: 3.0,
            2: 2.0,
        }
    )
    solution = OMMXPySCIPOptAdapter.solve(ommx_instance, initial_state=initial_state)
    # The solution should be the optimal one, not the initial one
    assert solution.state.entries[1] == 5.0
    assert solution.state.entries[2] == 0.0
    assert solution.objective == 5.0


def test_adapter_class_with_initial_state_from_mapping():
    # Objective function: x - y (Maximize)
    # x, y: integer variables with range [0, 5]
    # Constraint: x + y <= 5
    # Initial state: x = 3, y = 2
    # Optimal solution: x = 5, y = 0
    x = DecisionVariable.integer(1, lower=0, upper=5)
    y = DecisionVariable.integer(2, lower=0, upper=5)

    ommx_instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x - y,
        constraints={0: x + y <= 5},
        sense=Sense.Maximize,
    )
    initial_mapping = {
        1: 3.0,
        2: 2.0,
    }
    adapter = OMMXPySCIPOptAdapter(ommx_instance, initial_state=initial_mapping)
    model = adapter.solver_input
    sols = model.getSols()
    sol = sols[0]
    var_dict = {var.name: var for var in model.getVars()}
    # Verify the initial state was correctly set in the model
    assert model.getSolVal(sol, var_dict["1"]) == 3.0
    assert model.getSolVal(sol, var_dict["2"]) == 2.0


def test_solve_with_initial_state_from_mapping():
    # Objective function: x - y (Maximize)
    # x, y: integer variables with range [0, 5]
    # Constraint: x + y <= 5
    # Initial state: x = 3, y = 2
    # Optimal solution: x = 5, y = 0
    x = DecisionVariable.integer(1, lower=0, upper=5)
    y = DecisionVariable.integer(2, lower=0, upper=5)

    ommx_instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x - y,
        constraints={0: x + y <= 5},
        sense=Sense.Maximize,
    )
    initial_mapping = {
        1: 3.0,
        2: 2.0,
    }
    solution = OMMXPySCIPOptAdapter.solve(ommx_instance, initial_state=initial_mapping)
    # The solution should be the optimal one, not the initial one
    assert solution.state.entries[1] == 5.0
    assert solution.state.entries[2] == 0.0
    assert solution.objective == 5.0
