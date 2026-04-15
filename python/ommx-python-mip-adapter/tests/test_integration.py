import pytest

from ommx.v1 import Instance, DecisionVariable, Solution
from ommx.adapter import InfeasibleDetected, UnboundedDetected, NoSolutionReturned
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
    # Knapsack Problem
    # fmt: off
    base_v = [
        9, 28, 26, 5, 26, 29, 13, 20, 24, 10, 23, 15, 5, 27, 21, 8, 7, 8, 21, 13,
        24, 5, 5, 6, 20, 16, 25, 12, 28, 20, 20, 6, 6, 10, 29, 29, 17, 12, 26, 20,
        22, 20, 22, 23, 6, 23, 28, 5, 18, 17, 22, 13, 24, 17, 18, 11, 8, 9, 23, 21,
        17, 7, 10, 15, 7, 20, 17, 29, 22, 29, 15, 5, 24, 7, 5, 12, 11, 28, 13, 6,
        9, 8, 8, 13, 6, 15, 27, 7, 15, 29, 9, 26, 25, 27, 6, 26, 29, 26, 9, 10,
    ]
    base_w = [
        21, 37, 35, 19, 39, 36, 32, 32, 22, 20, 29, 21, 12, 39, 24, 8, 9, 19, 22, 12,
        38, 21, 7, 9, 24, 23, 35, 27, 29, 21, 18, 10, 20, 21, 30, 35, 36, 10, 45, 36,
        38, 22, 34, 23, 4, 29, 28, 12, 37, 23, 39, 32, 32, 18, 28, 26, 6, 10, 23, 29,
        20, 18, 14, 26, 23, 20, 36, 37, 31, 27, 18, 23, 30, 22, 8, 26, 16, 37, 26, 10,
        24, 12, 11, 21, 4, 14, 34, 12, 15, 34, 24, 27, 36, 31, 23, 37, 45, 44, 7, 20,
    ]
    # fmt: on
    v = base_v * 100
    w = base_w * 100
    n = len(v)
    x = [DecisionVariable.binary(i) for i in range(n)]
    constraint = sum(w[i] * x[i] for i in range(n)) <= 100 * 100
    assert not isinstance(constraint, bool)
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(v[i] * x[i] for i in range(n)),
        constraints=[constraint],
        sense=Instance.MAXIMIZE,
    )
    adapter = OMMXPythonMIPAdapter(instance)
    model = adapter.solver_input
    # Set a very small time limit to force the solver to stop before finding any solution
    model.max_seconds = 0.01
    model.optimize()

    with pytest.raises(
        NoSolutionReturned,
        match="No solution was returned during the search",
    ):
        adapter.decode_to_state(model)


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
