from ommx.v1 import Instance, DecisionVariable, State

from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter


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
        constraints=[x + y <= 5],
        sense=Instance.MAXIMIZE,
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
        constraints=[x + y <= 5],
        sense=Instance.MAXIMIZE,
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
        constraints=[x + y <= 5],
        sense=Instance.MAXIMIZE,
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
        constraints=[x + y <= 5],
        sense=Instance.MAXIMIZE,
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
