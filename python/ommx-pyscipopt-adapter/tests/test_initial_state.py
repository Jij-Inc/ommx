from ommx.v1 import Instance, DecisionVariable, State
from ommx import NamedState
import pytest

from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter


@pytest.mark.parametrize("sense", [Instance.MINIMIZE, Instance.MAXIMIZE])
@pytest.mark.parametrize("input_kind", ["named", "state", "ids"])
@pytest.mark.parametrize("partial", [False, True])
def test_quadratic_initial_state_initializes_solver_objective(
    sense, input_kind, partial
):
    x = DecisionVariable.integer(31, lower=1, upper=5, name="x", subscripts=[0])
    y = DecisionVariable.integer(107, lower=0, upper=5, name="x", subscripts=[1])
    sign = 1 if sense == Instance.MINIMIZE else -1
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=sign * (x * x + 2 * x * y + 3 * y + 7),
        constraints=[],
        sense=sense,
    )
    named: NamedState = {"x": {(0,): 2.0}} if partial else {"x": {(0,): 2.0, (1,): 3.0}}
    state = instance.state_from_named_values(named)
    initial_state = {"named": named, "state": state, "ids": state.entries}[input_kind]
    adapter = OMMXPySCIPOptAdapter(instance, initial_state=initial_state)
    model = adapter.solver_input
    solution = model.getSols()[0]
    auxiliary = next(
        var
        for var in model.getVars()
        if var.name == "auxiliary_for_linearized_objective"
    )
    expected = sign * (11.0 if partial else 32.0)

    assert model.getSolVal(solution, auxiliary) == pytest.approx(expected)
    assert model.getSolObjVal(solution) == pytest.approx(expected)
    assert model.checkSol(solution, printreason=False)
    model.optimize()
    assert adapter.decode(model).objective == pytest.approx(sign * 8.0)


@pytest.mark.parametrize("variable_ids", [(31, 107), (900, 41)])
@pytest.mark.parametrize("convert_explicitly", [False, True])
def test_named_initial_state_is_stored_before_optimization(
    variable_ids, convert_explicitly
):
    x = DecisionVariable.integer(
        variable_ids[0], lower=0, upper=5, name="x", subscripts=[0]
    )
    y = DecisionVariable.integer(
        variable_ids[1], lower=0, upper=5, name="x", subscripts=[1]
    )
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=2 * x + y,
        constraints=[x + y <= 5],
        sense=Instance.MAXIMIZE,
    )
    values: NamedState = {"x": {(0,): 1.0, (1,): 4.0}}
    initial_state = (
        instance.state_from_named_values(values) if convert_explicitly else values
    )
    adapter = OMMXPySCIPOptAdapter(instance, initial_state=initial_state)
    model = adapter.solver_input

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


def test_solve_accepts_named_initial_state():
    x = DecisionVariable.integer(31, lower=0, upper=5, name="x")
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints=[],
        sense=Instance.MAXIMIZE,
    )
    solution = OMMXPySCIPOptAdapter.solve(instance, initial_state={"x": {(): 2.0}})
    assert solution.state.entries == {x.id: 5.0}


@pytest.mark.parametrize("ambiguous", [False, True])
def test_named_initial_state_reports_label_errors(ambiguous):
    variables = [DecisionVariable.binary(31, name="x", subscripts=[0])]
    if ambiguous:
        variables.append(DecisionVariable.binary(107, name="x", subscripts=[0]))
    instance = Instance.from_components(
        decision_variables=variables,
        objective=0,
        constraints=[],
        sense=Instance.MINIMIZE,
    )
    values: NamedState = {"x": {(0 if ambiguous else 1,): 1.0}}
    with pytest.raises(ValueError if ambiguous else KeyError):
        OMMXPySCIPOptAdapter(instance, initial_state=values)


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
