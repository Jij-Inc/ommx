import pytest

from ommx.v1 import Instance, DecisionVariable, Solution
from ommx.adapter import InfeasibleDetected, UnboundedDetected, NoSolutionObtained
from ommx.testing import SingleFeasibleLPGenerator, DataType

from ommx_python_mip_adapter import OMMXPythonMIPAdapter
from ommx_python_mip_adapter.exception import OMMXPythonMIPAdapterError


@pytest.mark.parametrize(
    "generater",
    [
        SingleFeasibleLPGenerator(10, DataType.INT),
        SingleFeasibleLPGenerator(10, DataType.FLOAT),
    ],
)
def test_integration_lp(generater):
    # Objective function: 0
    # Constraints:
    #     A @ x = b    (A: regular matrix, b: constant vector)
    ommx_instance_bytes = generater.get_v1_instance()

    adapter = OMMXPythonMIPAdapter(ommx_instance_bytes)
    model = adapter.solver_input
    model.optimize()
    ommx_state = adapter.decode_to_state(model)
    expected_solution = generater.get_v1_state()
    assert ommx_state.entries.keys() == expected_solution.entries.keys()
    for key in ommx_state.entries.keys():
        assert ommx_state.entries[key] == pytest.approx(
            expected_solution.entries[key], abs=1e-6
        )


def test_integration_milp():
    # Objective function: - x1 - x2
    # Constraints:
    #     3x1 - x2 - 6 <= 0
    #     -x1 + 3x2 - 6 <= 0
    #     0 <= x1 <= 10    (x: integer)
    #     0 <= x2 <= 10    (x: continuous)
    # Optimal solution: x1 = 3, x2 = 3
    LOWER_BOUND = 0
    UPPER_BOUND = 10
    x1 = DecisionVariable.integer(1, lower=LOWER_BOUND, upper=UPPER_BOUND)
    x2 = DecisionVariable.continuous(2, lower=LOWER_BOUND, upper=UPPER_BOUND)
    ommx_instance = Instance.from_components(
        decision_variables=[x1, x2],
        objective=-x1 - x2,
        constraints=[
            3 * x1 - x2 <= 6,
            -x1 + 3 * x2 <= 6,
        ],
        sense=Instance.MINIMIZE,
    )

    adapter = OMMXPythonMIPAdapter(ommx_instance)
    model = adapter.solver_input
    model.optimize()
    ommx_state = adapter.decode_to_state(model)

    assert ommx_state.entries[1] == pytest.approx(3)
    assert ommx_state.entries[2] == pytest.approx(3)


def test_solution_optimality():
    x = DecisionVariable.integer(1, lower=0, upper=5)
    y = DecisionVariable.integer(2, lower=0, upper=5)
    ommx_instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x + y,
        constraints=[],
        sense=Instance.MAXIMIZE,
    )

    solution = OMMXPythonMIPAdapter.solve(ommx_instance)
    assert solution.optimality == Solution.OPTIMAL


def test_partial_evaluate():
    x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0] + x[1] + x[2],
        constraints=[(x[0] + x[1] + x[2] <= 1).set_id(0)],  # one-hot constraint
        sense=Instance.MINIMIZE,
    )
    assert instance.used_decision_variables == x
    partial = instance.partial_evaluate({0: 1})
    # x[0] is no longer present in the problem
    assert partial.used_decision_variables == x[1:]

    solution = OMMXPythonMIPAdapter.solve(partial)
    assert [var.value for var in solution.decision_variables] == [1, 0, 0]
    assert solution.optimality == Solution.OPTIMAL

    partial = instance.partial_evaluate({1: 1})
    solution = OMMXPythonMIPAdapter.solve(partial)
    assert [var.value for var in solution.decision_variables] == [0, 1, 0]

    partial = instance.partial_evaluate({2: 1})
    solution = OMMXPythonMIPAdapter.solve(partial)
    assert [var.value for var in solution.decision_variables] == [0, 0, 1]


def test_relax_constraint():
    x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0] + x[1],
        constraints=[(x[0] + 2 * x[1] <= 1).set_id(0), (x[1] + x[2] <= 1).set_id(1)],
        sense=Instance.MINIMIZE,
    )

    assert instance.used_decision_variables == x
    instance.relax_constraint(1, "relax")
    # id for x[2] is listed as irrelevant
    assert instance.decision_variable_analysis().irrelevant() == {x[2].id}

    solution = OMMXPythonMIPAdapter.solve(instance)
    # x[2] is still present as part of the evaluate/decoding process but has a value of 0
    assert [var.value for var in solution.decision_variables] == [0, 0, 0]


def test_integration_timelimit():
    # KnapSack Problem
    p = [10, 13, 18, 32, 7, 15, 12, 6, 22, 20, 19, 13, 11, 39, 10]
    w = [11, 15, 20, 35, 10, 33, 28, 23, 11, 10, 12, 16, 17, 26, 29]
    n = len(p)
    x = [DecisionVariable.binary(i) for i in range(n)]
    constraint = sum(w[i] * x[i] for i in range(n)) <= sum(w) // 2
    assert not isinstance(constraint, bool)
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(p[i] * x[i] for i in range(n)),
        constraints=[constraint],
        sense=Instance.MAXIMIZE,
    )
    adapter = OMMXPythonMIPAdapter(instance)
    model = adapter.solver_input
    # Set a very small time limit to force the solver to stop before finding any solution
    model.max_seconds = 0.0001
    model.optimize()

    with pytest.raises(
        NoSolutionObtained,
        match=r"No solution was obtained during the search",
    ):
        adapter.decode(model)


def test_infeasible_problem():
    # x must be >= 4, but upper bound is 3 -> infeasible
    x = DecisionVariable.integer(0, lower=0, upper=3)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints=[x >= 4],
        sense=Instance.MAXIMIZE,
    )
    adapter = OMMXPythonMIPAdapter(instance)
    model = adapter.solver_input
    model.optimize()

    with pytest.raises(InfeasibleDetected):
        adapter.decode_to_state(model)


def test_unbounded_problem():
    # x has no upper bound, maximize x -> unbounded
    x = DecisionVariable.integer(0, lower=0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints=[],
        sense=Instance.MAXIMIZE,
    )
    adapter = OMMXPythonMIPAdapter(instance)
    model = adapter.solver_input
    model.optimize()

    with pytest.raises(UnboundedDetected):
        adapter.decode_to_state(model)


def test_decode_before_optimize():
    x = DecisionVariable.integer(0, lower=0, upper=5)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints=[],
        sense=Instance.MINIMIZE,
    )
    adapter = OMMXPythonMIPAdapter(instance)
    model = adapter.solver_input
    # Do not call model.optimize()

    with pytest.raises(
        OMMXPythonMIPAdapterError,
        match=r"The model may not be optimized. \[status: LOADED\]",
    ):
        adapter.decode_to_state(model)
