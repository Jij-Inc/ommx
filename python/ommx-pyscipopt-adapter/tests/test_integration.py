import pytest

from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

from ommx.v1 import Constraint, Instance, DecisionVariable, Quadratic, Linear
from ommx.adapter import InfeasibleDetected
from ommx.testing import SingleFeasibleLPGenerator, DataType


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
    instance = generater.get_v1_instance()

    adapter = OMMXPySCIPOptAdapter(instance)
    model = adapter.solver_input
    model.optimize()
    state = adapter.decode_to_state(model)
    expected = generater.get_v1_state()

    actual_entries = state.entries
    expected_entries = expected.entries

    # Check the solution of each decision variable
    for key, actual_value in actual_entries.items():
        expected_value = expected_entries[key]
        assert actual_value == pytest.approx(expected_value, abs=1e-6)


def test_integration_milp():
    # Objective function: - x1 - x2
    # Constraints:
    #     3x1 - x2 - 6 <= 0
    #     -x1 + 3x2 - 6 <= 0
    #     0 <= x1 <= 10    (x1: integer)
    #     0 <= x2 <= 10    (x2: continuous)
    # Optimal solution: x1 = 3, x2 = 3
    LOWER_BOUND = 0
    UPPER_BOUND = 10
    x1 = DecisionVariable.integer(1, lower=LOWER_BOUND, upper=UPPER_BOUND)
    x2 = DecisionVariable.continuous(2, lower=LOWER_BOUND, upper=UPPER_BOUND)
    instance = Instance.from_components(
        decision_variables=[x1, x2],
        objective=-x1 - x2,
        constraints=[
            3 * x1 - x2 <= 6,
            -x1 + 3 * x2 <= 6,
        ],
        sense=Instance.MINIMIZE,
    )

    adapter = OMMXPySCIPOptAdapter(instance)
    model = adapter.solver_input
    model.optimize()
    state = adapter.decode_to_state(model)

    actual_entries = state.entries
    assert actual_entries[1] == pytest.approx(3)
    assert actual_entries[2] == pytest.approx(3)


def test_integration_binary():
    # Objective function: - x1 + x2
    #     x1, x2: binary
    # Optimal solution: x1 = 1, x2 = 0
    x1 = DecisionVariable.binary(1)
    x2 = DecisionVariable.binary(2)
    instance = Instance.from_components(
        decision_variables=[x1, x2],
        objective=-x1 + x2,
        constraints=[],
        sense=Instance.MINIMIZE,
    )

    adapter = OMMXPySCIPOptAdapter(instance)
    model = adapter.solver_input
    model.optimize()
    state = adapter.decode_to_state(model)

    actual_entries = state.entries
    assert actual_entries[1] == pytest.approx(1)
    assert actual_entries[2] == pytest.approx(0)


def test_integration_maximize():
    # Objective function: - x1 + x2（Maximize）
    #     x1, x2: binary
    # Optimal solution: x1 = 0, x2 = 1
    x1 = DecisionVariable.binary(1)
    x2 = DecisionVariable.binary(2)
    instance = Instance.from_components(
        decision_variables=[x1, x2],
        objective=-x1 + x2,
        constraints=[],
        sense=Instance.MAXIMIZE,
    )

    adapter = OMMXPySCIPOptAdapter(instance)
    model = adapter.solver_input
    model.optimize()
    state = adapter.decode_to_state(model)

    actual_entries = state.entries
    assert actual_entries[1] == pytest.approx(0)
    assert actual_entries[2] == pytest.approx(1)


def test_integration_constant_objective():
    # Objective function: 0
    # Constraints:
    #     x1 + x2 - 5 = 0
    #     0 <= x1 <= 10    (x1: integer)
    #     0 <= x2 <= 10    (x2: continuous)
    # Optimal solution: x1, x2, such that x1 + x2 = 5
    LOWER_BOUND = 0
    UPPER_BOUND = 10
    x1 = DecisionVariable.integer(1, lower=LOWER_BOUND, upper=UPPER_BOUND)
    x2 = DecisionVariable.continuous(2, lower=LOWER_BOUND, upper=UPPER_BOUND)
    instance = Instance.from_components(
        decision_variables=[x1, x2],
        objective=0,
        constraints=[x1 + x2 - 5 == 0],
        sense=Instance.MINIMIZE,
    )

    adapter = OMMXPySCIPOptAdapter(instance)
    model = adapter.solver_input
    model.optimize()
    # check objective
    assert model.getObjVal() == 0
    state = adapter.decode_to_state(model)

    actual_entries = state.entries
    assert actual_entries[1] + actual_entries[2] == pytest.approx(5)


def test_integration_quadratic_objective():
    # Objective function: x1 * x1 + x2 * x2
    # Constraints:
    #     x1 + x2 - 4 = 0
    #     0 <= x1 <= 10    (x1: integer)
    #     0 <= x2 <= 10    (x2: continuous)
    # Optimal solution: x1 = 2, x2 = 2
    LOWER_BOUND = 0
    UPPER_BOUND = 10
    x1 = DecisionVariable.integer(1, lower=LOWER_BOUND, upper=UPPER_BOUND)
    x2 = DecisionVariable.continuous(2, lower=LOWER_BOUND, upper=UPPER_BOUND)
    instance = Instance.from_components(
        sense=Instance.MINIMIZE,
        decision_variables=[x1, x2],
        objective=Quadratic(
            rows=[1, 2],
            columns=[1, 2],
            values=[1, 1],
        ),
        constraints=[x1 + x2 == 4],
    )

    adapter = OMMXPySCIPOptAdapter(instance)
    model = adapter.solver_input
    model.optimize()
    state = adapter.decode_to_state(model)

    actual_entries = state.entries
    assert actual_entries[1] == pytest.approx(2)
    assert actual_entries[2] == pytest.approx(2)


def test_integration_quadratic_constraint():
    # Objective function: - x1 - x2
    # Constraints:
    #     x1 * x1 + x2 * x2 - 100 <= 0
    #     0 <= x1 <= 10    (x1: integer)
    #     0 <= x2 <= 10    (x2: continuous)
    # Optimal solution: x1 = 7, x2 = sqrt(51)
    LOWER_BOUND = 0
    UPPER_BOUND = 10
    x1 = DecisionVariable.integer(1, lower=LOWER_BOUND, upper=UPPER_BOUND)
    x2 = DecisionVariable.continuous(2, lower=LOWER_BOUND, upper=UPPER_BOUND)
    instance = Instance.from_components(
        sense=Instance.MINIMIZE,
        decision_variables=[x1, x2],
        objective=-x1 - x2,
        constraints=[
            Constraint(
                function=Quadratic(
                    columns=[1, 2],
                    rows=[1, 2],
                    values=[1, 1],
                    linear=Linear(terms={}, constant=-100),
                ),
                equality=Constraint.LESS_THAN_OR_EQUAL_TO_ZERO,
            ),
        ],
    )

    adapter = OMMXPySCIPOptAdapter(instance)
    model = adapter.solver_input
    model.optimize()
    state = adapter.decode_to_state(model)

    actual_entries = state.entries
    assert actual_entries[1] == pytest.approx(7)
    assert actual_entries[2] == pytest.approx(51**0.5)


def test_integration_feasible_constant_constraint():
    # Objective function: - x1 - x2
    # Constraints:
    #     3x1 - x2 - 6 <= 0
    #     -x1 + 3x2 - 6 <= 0
    #     0 <= x1 <= 10    (x1: integer)
    #     0 <= x2 <= 10    (x2: continuous)
    #     -1 <= 0          (feasible constant constraint)
    # Optimal solution: x1 = 3, x2 = 3
    LOWER_BOUND = 0
    UPPER_BOUND = 10
    x1 = DecisionVariable.integer(1, lower=LOWER_BOUND, upper=UPPER_BOUND)
    x2 = DecisionVariable.continuous(2, lower=LOWER_BOUND, upper=UPPER_BOUND)
    instance = Instance.from_components(
        decision_variables=[x1, x2],
        objective=-x1 - x2,
        constraints=[
            3 * x1 - x2 <= 6,
            -x1 + 3 * x2 <= 6,
            Constraint(
                function=-1,
                equality=Constraint.LESS_THAN_OR_EQUAL_TO_ZERO,
            ),
        ],
        sense=Instance.MINIMIZE,
    )

    adapter = OMMXPySCIPOptAdapter(instance)
    model = adapter.solver_input
    model.optimize()
    state = adapter.decode_to_state(model)

    actual_entries = state.entries
    assert actual_entries[1] == pytest.approx(3)
    assert actual_entries[2] == pytest.approx(3)


def test_integration_timelimit():
    # Objective function: x1 + x2
    # x1, x2: binary
    x1 = DecisionVariable.binary(1)
    x2 = DecisionVariable.binary(2)
    instance = Instance.from_components(
        decision_variables=[x1, x2],
        objective=-x1 + x2,
        constraints=[],
        sense=Instance.MAXIMIZE,
    )

    adapter = OMMXPySCIPOptAdapter(instance)
    model = adapter.solver_input
    # Set a very small time limit to force the solver to stop before finding the optimal solution
    model.setParam("limits/time", 0.00001)
    model.optimize()

    with pytest.raises(
        InfeasibleDetected, match=r"Model was infeasible \[status: timelimit\]"
    ):
        adapter.decode_to_state(model)


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

    solution = OMMXPySCIPOptAdapter.solve(partial)
    assert [var.value for var in solution.decision_variables] == [1, 0, 0]

    partial = instance.partial_evaluate({1: 1})
    solution = OMMXPySCIPOptAdapter.solve(partial)
    assert [var.value for var in solution.decision_variables] == [0, 1, 0]

    partial = instance.partial_evaluate({2: 1})
    solution = OMMXPySCIPOptAdapter.solve(partial)
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

    solution = OMMXPySCIPOptAdapter.solve(instance)
    # x[2] is still present as part of the evaluate/decoding process but has a value of 0
    assert [var.value for var in solution.decision_variables] == [0, 0, 0]
