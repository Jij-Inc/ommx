"""Model collections keep membership separate from typed constraint IDs."""

from typing import Any

import pytest
from typing_extensions import assert_type

from ommx import (
    AttachedConstraint,
    ConstraintID,
    ConstraintKind,
    ConstraintView,
    DecisionVariable,
    InactiveConstraint,
    InactiveIndicatorConstraint,
    InactiveOneHotConstraint,
    InactiveSos1Constraint,
    Instance,
    OneHotConstraint,
    ParametricInstance,
    Sense,
    Sos1Constraint,
)


def test_all_state_mapping_and_stale_attached_handle():
    model = Instance.minimize()
    x = model.new_binary("x")
    attached = model.add_constraint(x == 1)
    cid = attached.constraint_id
    rows = model.constraints
    assert_type(rows[cid], ConstraintView)
    assert_type(rows.get(ConstraintID(99)), ConstraintView | None)
    assert_type(rows.of_kind(ConstraintKind.Regular)[cid], ConstraintView)
    assert isinstance(rows[cid], AttachedConstraint)
    model.deactivate_constraint(cid, "report", source="test")
    assert isinstance(rows[cid], InactiveConstraint)
    with pytest.raises(KeyError):
        model.active_constraints[cid]
    with pytest.raises(RuntimeError, match="no longer active"):
        attached.set_name("stale")
    snapshot = model.inactive_constraints[cid]
    assert_type(rows.get(cid, snapshot), ConstraintView)
    assert snapshot.inactive_reason == "report"
    assert snapshot.inactive_reason_parameters == {"source": "test"}
    model.activate_constraint(cid)
    assert isinstance(rows[cid], AttachedConstraint)
    attached.set_name("active again")
    assert rows[cid].name == "active again"
    assert snapshot.name is None
    assert snapshot.inactive_reason == "report"


@pytest.mark.parametrize("host", [Instance, ParametricInstance])
def test_initial_inactive_components_and_all_family_transitions(host):
    x, y = DecisionVariable.binary(0), DecisionVariable.binary(1)
    kwargs: dict[str, Any] = dict(
        sense=Sense.Minimize,
        objective=0,
        decision_variables=[x, y],
        active_constraints={},
        inactive_constraints={1: InactiveConstraint(x == 1, "report")},
        inactive_indicator_constraints={
            2: InactiveIndicatorConstraint((y == 1).with_indicator(x), "report")
        },
        inactive_one_hot_constraints={
            3: InactiveOneHotConstraint(OneHotConstraint(variables=[x, y]), "report")
        },
        inactive_sos1_constraints={
            4: InactiveSos1Constraint(Sos1Constraint(variables=[x, y]), "report")
        },
    )
    if host is ParametricInstance:
        kwargs["parameters"] = []
    model = host.from_components(**kwargs)
    assert len(model.constraints) == len(model.inactive_constraints) == 4
    assert not model.active_constraints
    for family, cid in zip(("", "indicator_", "one_hot_", "sos1_"), range(1, 5)):
        getattr(model, f"activate_{family}constraint")(cid)
        assert len(model.active_constraints) == 1
        getattr(model, f"deactivate_{family}constraint")(cid, "again")
        assert not model.active_constraints
    assert len(model.constraints) == len(model.inactive_constraints) == 4
    restored = host.from_v2_bytes(model.to_v2_bytes())
    assert set(restored.constraints) == set(model.constraints)
    assert len(restored.inactive_constraints) == 4
    assert not restored.active_constraints
    instance = (
        model.with_parameters({}) if isinstance(model, ParametricInstance) else model
    )
    evaluated = instance.evaluate({x.id: 1, y.id: 1})
    assert not evaluated.feasible
    assert evaluated.feasible_relaxed
    assert evaluated.total_violation() == 2
