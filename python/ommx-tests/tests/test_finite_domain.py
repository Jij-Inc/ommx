import math

import pytest

from ommx import DecisionVariable, Instance


def test_create_finite_domain_variable():
    x = DecisionVariable.finite_domain(
        1,
        values=[1.0, 0.1, 0.5, 0.3],
        name="x",
    )

    assert x.kind == DecisionVariable.FINITE_DOMAIN
    assert x.values == [0.1, 0.3, 0.5, 1.0]
    assert x.bound.lower == 0.1
    assert x.bound.upper == 1.0
    assert x.name == "x"


@pytest.mark.parametrize(
    "values, match",
    [
        ([], "at least one value"),
        ([0.1, 0.1], "must be unique"),
        ([0.1, math.nan], "must be finite"),
        ([0.1, math.inf], "must be finite"),
    ],
)
def test_reject_invalid_finite_domains(values, match):
    with pytest.raises(ValueError, match=match):
        DecisionVariable.finite_domain(1, values)


def test_finite_domain_evaluation_and_wire_roundtrip():
    x = DecisionVariable.finite_domain(1, [0.1, 0.3, 0.5, 1.0], name="x")
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Instance.MINIMIZE,
    )

    assert instance.evaluate({1: 0.3}).feasible
    assert not instance.evaluate({1: 0.4}).feasible

    restored_v1 = Instance.from_v1_bytes(instance.to_v1_bytes())
    restored_v2 = Instance.from_v2_bytes(instance.to_v2_bytes())
    for restored in (restored_v1, restored_v2):
        variable = restored.decision_variables[0]
        assert variable.kind == DecisionVariable.FINITE_DOMAIN
        assert variable.values == [0.1, 0.3, 0.5, 1.0]
        assert variable.bound.lower == 0.1
        assert variable.bound.upper == 1.0


def test_attached_finite_domain_preserves_exact_values():
    instance = Instance.minimize()
    attached = instance.add_decision_variable(
        DecisionVariable.finite_domain(4, [0.1, 0.3, 0.5, 1.0])
    )

    assert attached.kind == DecisionVariable.FINITE_DOMAIN
    assert attached.values == [0.1, 0.3, 0.5, 1.0]
    assert attached.detach().values == [0.1, 0.3, 0.5, 1.0]


def test_instance_new_finite_domain():
    instance = Instance.minimize()

    x = instance.new_finite_domain([1.0, 0.1, 0.5], "x")

    assert x.id == 0
    assert x.name == "x"
    assert x.kind == DecisionVariable.FINITE_DOMAIN
    assert x.values == [0.1, 0.5, 1.0]


def test_finite_domain_is_visible_in_dataframes():
    x = DecisionVariable.finite_domain(0, [0.1, 0.5, 1.0])
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Instance.MINIMIZE,
    )

    assert instance.decision_variables_df(include=[]).loc[0, "values"] == [
        0.1,
        0.5,
        1.0,
    ]
    solution = instance.evaluate({0: 0.5})
    assert solution.decision_variables_df(include=[]).loc[0, "values"] == [
        0.1,
        0.5,
        1.0,
    ]
