import pytest

from ommx import (
    DecisionVariable,
    Instance,
    OneHotConstraint,
    Parameter,
    ParametricInstance,
    Sense,
)


def _special_instance() -> Instance:
    variables = [DecisionVariable.binary(i) for i in range(3)]
    return Instance.from_components(
        sense=Instance.MINIMIZE,
        objective=sum(variables),
        decision_variables=variables,
        constraints={},
        one_hot_constraints={10: OneHotConstraint(variables=variables)},
    )


def test_instance_v2_bytes_roundtrip_special_constraints():
    instance = _special_instance()

    with pytest.raises(Exception, match="to_v2_bytes"):
        instance.to_v1_bytes()

    restored = Instance.from_v2_bytes(instance.to_v2_bytes())

    assert restored.one_hot_constraints[10].variables == [0, 1, 2]
    assert restored.to_v2_bytes() == instance.to_v2_bytes()


def test_parametric_instance_v2_bytes_roundtrip():
    x = DecisionVariable.binary(0)
    instance = ParametricInstance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[x],
        constraints={},
        parameters=[Parameter(id=100, name="alpha")],
    )

    restored = ParametricInstance.from_v2_bytes(instance.to_v2_bytes())

    assert restored.decision_variable_ids == {0}
    assert [(parameter.id, parameter.name) for parameter in restored.parameters] == [
        (100, "alpha")
    ]
    assert restored.to_v2_bytes() == instance.to_v2_bytes()


def test_solution_v2_bytes_roundtrip_special_constraints():
    solution = _special_instance().evaluate({0: 0, 1: 1, 2: 0})

    restored = type(solution).from_v2_bytes(solution.to_v2_bytes())

    assert restored.feasible
    assert len(restored.constraints_df(kind="one_hot")) == 1
    assert restored.to_v2_bytes() == solution.to_v2_bytes()


def test_sample_set_v2_bytes_roundtrip_special_constraints():
    sample_set = _special_instance().evaluate_samples(
        [
            {0: 0, 1: 1, 2: 0},
            {0: 1, 1: 1, 2: 0},
        ]
    )

    restored = type(sample_set).from_v2_bytes(sample_set.to_v2_bytes())

    assert len(restored.constraints_df(kind="one_hot")) == 1
    assert restored.to_v2_bytes() == sample_set.to_v2_bytes()
