import highspy
import pytest
from highspy.highs import highs_var

from ommx.v1 import Constraint, Instance, DecisionVariable, Solution
from ommx.testing import SingleFeasibleLPGenerator, DataType

from ommx_highs_adapter import OMMXHighsAdapter


@pytest.mark.parametrize("sense", [Instance.MINIMIZE, Instance.MAXIMIZE])
@pytest.mark.parametrize("objective_kind", ["linear", "constant", "zero"])
def test_constructor_preserves_model_without_solving(
    monkeypatch, sense, objective_kind
):
    # Noncontiguous IDs must map to HiGHS columns, including constraint-only variables.
    x = DecisionVariable.continuous(11, lower=0, upper=5)
    y = DecisionVariable.integer(29, lower=0, upper=2)
    objective = {"linear": 2 * x - 3 * y + 7, "constant": 7, "zero": 0}[objective_kind]
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=objective,
        constraints=[
            (x + y == 3).set_id(10),
            (x - 2 * y <= 0).set_id(20),
            Constraint(id=30, function=0, equality=Constraint.EQUAL_TO_ZERO),
            Constraint(
                id=40, function=-2, equality=Constraint.LESS_THAN_OR_EQUAL_TO_ZERO
            ),
        ],
        sense=sense,
    )
    before = instance.to_bytes()

    def unexpected_run(*args, **kwargs):
        pytest.fail("Constructing an adapter must not run HiGHS")

    with monkeypatch.context() as context:
        context.setattr(highspy.Highs, "run", unexpected_run)
        context.setattr(highspy.Highs, "solve", unexpected_run)
        adapter = OMMXHighsAdapter(instance)

    model = adapter.solver_input
    assert instance.to_bytes() == before
    assert model.getModelStatus() == highspy.HighsModelStatus.kNotset
    assert model.getRunTime() == 0
    assert model.getNumCol() == 2
    assert model.getNumRow() == 2  # Satisfied constant constraints are omitted.
    expected_sense = (
        highspy.ObjSense.kMinimize
        if sense == Instance.MINIMIZE
        else highspy.ObjSense.kMaximize
    )
    assert model.getObjectiveSense() == (highspy.HighsStatus.kOk, expected_sense)
    offset = 0 if objective_kind == "zero" else 7
    assert model.getObjectiveOffset() == (highspy.HighsStatus.kOk, offset)
    lp = model.getLp()
    highs_x = adapter.highs_vars[x.id]
    highs_y = adapter.highs_vars[y.id]
    assert isinstance(highs_x, highs_var)
    assert isinstance(highs_y, highs_var)
    assert lp.col_cost_[highs_x.index] == (2 if objective_kind == "linear" else 0)
    assert lp.col_cost_[highs_y.index] == (-3 if objective_kind == "linear" else 0)

    assert model.run() == highspy.HighsStatus.kOk
    solution = adapter.decode(model)
    expected = (
        (3 if sense == Instance.MINIMIZE else 8)
        if objective_kind == "linear"
        else offset
    )
    assert solution.feasible
    assert solution.objective == pytest.approx(expected)
    assert model.getObjectiveValue() == pytest.approx(expected)


def test_integration_lp():
    x1 = DecisionVariable.continuous(1, lower=0, upper=5)
    x2 = DecisionVariable.continuous(2, lower=-1, upper=5)

    instance = Instance.from_components(
        decision_variables=[x1, x2],
        objective=x1 + 2 * x2,
        constraints=[x1 + x2 <= 5],
        sense=Instance.MINIMIZE,
    )
    adapter = OMMXHighsAdapter(instance)

    model = adapter.solver_input
    model.run()

    state = adapter.decode_to_state(model)
    assert state.entries[1] == pytest.approx(0)
    assert state.entries[2] == pytest.approx(-1)


def test_integration_milp():
    """混合整数計画問題のテスト"""
    x1 = DecisionVariable.integer(1, lower=0, upper=5)
    x2 = DecisionVariable.continuous(2, lower=0, upper=5)

    instance = Instance.from_components(
        decision_variables=[x1, x2],
        objective=-x1 - x2,
        constraints=[3 * x1 - x2 <= 6, -x1 + 3 * x2 <= 6],
        sense=Instance.MINIMIZE,
    )

    adapter = OMMXHighsAdapter(instance)

    model = adapter.solver_input
    model.run()

    state = adapter.decode_to_state(model)
    assert state.entries[1] == pytest.approx(3)
    assert state.entries[2] == pytest.approx(3)


def test_solution_optimality():
    x = DecisionVariable.integer(1, lower=0, upper=5)
    y = DecisionVariable.integer(2, lower=0, upper=5)
    ommx_instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x + y,
        constraints=[],
        sense=Instance.MAXIMIZE,
    )

    solution = OMMXHighsAdapter.solve(ommx_instance)
    assert solution.optimality == Solution.OPTIMAL


@pytest.mark.parametrize(
    "generator",
    [
        SingleFeasibleLPGenerator(10, DataType.INT),
        SingleFeasibleLPGenerator(10, DataType.FLOAT),
    ],
)
def test_with_test_generator(generator):
    instance = generator.get_v1_instance()

    adapter = OMMXHighsAdapter(instance)
    model = adapter.solver_input
    model.run()
    state = adapter.decode_to_state(model)
    expected = generator.get_v1_state()

    for key in state.entries:
        assert state.entries[key] == pytest.approx(expected.entries[key], abs=1e-6)


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

    solution = OMMXHighsAdapter.solve(partial)
    assert [var.value for var in solution.decision_variables] == [1, 0, 0]

    partial = instance.partial_evaluate({1: 1})
    solution = OMMXHighsAdapter.solve(partial)
    assert [var.value for var in solution.decision_variables] == [0, 1, 0]

    partial = instance.partial_evaluate({2: 1})
    solution = OMMXHighsAdapter.solve(partial)
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

    solution = OMMXHighsAdapter.solve(instance)
    # x[2] is still present as part of the evaluate/decoding process but has a value of 0
    assert [var.value for var in solution.decision_variables] == [0, 0, 0]
