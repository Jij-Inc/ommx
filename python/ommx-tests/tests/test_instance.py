from ommx.v1 import Instance, DecisionVariable, Function
import math
import pytest


def test_set_objective():
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints=[],
        sense=Instance.MAXIMIZE,
    )
    assert instance.objective.almost_equal(Function(sum(x)))

    instance.objective = x[1]
    assert instance.objective.almost_equal(Function(x[1]))


def test_convert_inequality_to_equality_with_integer_slack_limit():
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints=[(math.pi * x[0] + math.e * x[1] >= 1).set_id(0)],
        sense=Instance.MAXIMIZE,
    )
    with pytest.raises(RuntimeError) as e:
        instance.convert_inequality_to_equality_with_integer_slack(0, 32)
    assert (
        str(e.value)
        == "The range of the slack variable exceeds the limit: evaluated(15174216961756088) > limit(32)"
    )


def test_convert_inequality_to_equality_with_integer_slack_continuous():
    x = [DecisionVariable.continuous(i, lower=-1.23, upper=4.56) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints=[(x[0] + x[1] >= 7.89).set_id(0)],
        sense=Instance.MAXIMIZE,
    )
    with pytest.raises(RuntimeError) as e:
        instance.convert_inequality_to_equality_with_integer_slack(0, 32)
    assert (
        str(e.value)
        == "The constraint contains continuous decision variables: ID=VariableID(0)"
    )
