from ommx.diff import match_instance_ids
from ommx.v1 import (
    Constraint,
    DecisionVariable,
    IndicatorConstraint,
    Instance,
    OneHotConstraint,
    Sos1Constraint,
)


def test_match_instance_ids_recovers_relabelled_regular_constraints():
    x = [DecisionVariable.binary(i) for i in range(3)]
    source = Instance.from_components(
        decision_variables=x,
        objective=5 * x[0] + 7 * x[1] + 11 * x[2],
        constraints={
            10: x[0] + x[1] + x[2] <= 1,
            20: 2 * x[1] - x[2] == 0,
            30: x[0] + 3 * x[2] <= 2,
        },
        sense=Instance.MINIMIZE,
    )

    y = {
        0: DecisionVariable.binary(102),
        1: DecisionVariable.binary(101),
        2: DecisionVariable.binary(103),
    }
    target = Instance.from_components(
        decision_variables=[y[0], y[1], y[2]],
        objective=5 * y[0] + 7 * y[1] + 11 * y[2],
        constraints={
            200: y[0] + y[1] + y[2] <= 1,
            100: 2 * y[1] - y[2] == 0,
            300: y[0] + 3 * y[2] <= 2,
        },
        sense=Instance.MINIMIZE,
    )

    mapping = match_instance_ids(source, target)

    assert mapping.verified
    assert mapping.objective_matches
    assert mapping.decision_variables == {0: 102, 1: 101, 2: 103}
    assert mapping.constraints == {10: 200, 20: 100, 30: 300}
    assert mapping.score == 1.0


def test_match_instance_ids_handles_structural_constraints():
    x0 = DecisionVariable.binary(0)
    x1 = DecisionVariable.binary(1)
    x2 = DecisionVariable.binary(2)
    source = Instance.from_components(
        decision_variables=[x0, x1, x2],
        objective=x1 + 2 * x2,
        constraints={},
        indicator_constraints={
            7: IndicatorConstraint(
                indicator_variable=x0,
                function=x1 + x2 - 1,
                equality=Constraint.LESS_THAN_OR_EQUAL_TO_ZERO,
            )
        },
        one_hot_constraints={8: OneHotConstraint(variables=[0, 2])},
        sos1_constraints={9: Sos1Constraint(variables=[1, 2])},
        sense=Instance.MINIMIZE,
    )

    y0 = DecisionVariable.binary(100)
    y1 = DecisionVariable.binary(200)
    y2 = DecisionVariable.binary(300)
    target = Instance.from_components(
        decision_variables=[y0, y1, y2],
        objective=y1 + 2 * y2,
        constraints={},
        indicator_constraints={
            70: IndicatorConstraint(
                indicator_variable=y0,
                function=y1 + y2 - 1,
                equality=Constraint.LESS_THAN_OR_EQUAL_TO_ZERO,
            )
        },
        one_hot_constraints={80: OneHotConstraint(variables=[100, 300])},
        sos1_constraints={90: Sos1Constraint(variables=[200, 300])},
        sense=Instance.MINIMIZE,
    )

    mapping = match_instance_ids(source, target)

    assert mapping.verified
    assert mapping.decision_variables == {0: 100, 1: 200, 2: 300}
    assert mapping.indicator_constraints == {7: 70}
    assert mapping.one_hot_constraints == {8: 80}
    assert mapping.sos1_constraints == {9: 90}


def test_match_instance_ids_reports_unverified_for_nonmatching_objective():
    x = [DecisionVariable.binary(i) for i in range(2)]
    source = Instance.from_components(
        decision_variables=x,
        objective=x[0] + x[1],
        constraints={1: x[0] + x[1] <= 1},
        sense=Instance.MINIMIZE,
    )

    y = [DecisionVariable.binary(10), DecisionVariable.binary(11)]
    target = Instance.from_components(
        decision_variables=y,
        objective=y[0] + 2 * y[1],
        constraints={100: y[0] + y[1] <= 1},
        sense=Instance.MINIMIZE,
    )

    mapping = match_instance_ids(source, target)

    assert not mapping.verified
    assert not mapping.objective_matches
    assert mapping.score < 1.0 or mapping.diagnostics
