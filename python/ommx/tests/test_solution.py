import pytest

from ommx import DecisionVariable, Instance, Sense


def test_solution_sense_minimize():
    instance = Instance.from_components(
        decision_variables=[],
        objective=0,
        active_constraints={},
        sense=Sense.Minimize,
    )
    solution = instance.evaluate({})
    assert solution.sense == Sense.Minimize


def test_solution_sense_maximize():
    instance = Instance.from_components(
        decision_variables=[],
        objective=0,
        active_constraints={},
        sense=Sense.Maximize,
    )
    solution = instance.evaluate({})
    assert solution.sense == Sense.Maximize


@pytest.mark.parametrize("slope", [1, -1])
def test_solution_state_iteration_follows_variable_id_order(slope):
    ids = [42, 3, 100, 17, 0, 28]
    assignment = {i: float(slope * i + 1) for i in ids}
    instance = Instance.from_components(
        decision_variables=[DecisionVariable.continuous(i) for i in ids],
        objective=0,
        active_constraints={},
        sense=Sense.Minimize,
    )
    solution = instance.evaluate(assignment)

    expected = sorted(assignment.items())
    assert (
        list(zip(solution.state.keys(), solution.state.values(), strict=True))
        == expected
    )
    assert solution.state.items() == expected
    assert list(solution.state.entries.items()) == expected
    assert (
        str(solution.state)
        == repr(solution.state)
        == f"State(entries={dict(expected)})"
    )
