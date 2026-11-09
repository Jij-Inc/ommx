"""Ordered activeness selects one coherent constraint set across query APIs."""

from typing import Any

import pytest
from typing_extensions import assert_type

from ommx import (
    AttachedConstraint,
    ConstraintActiveness as Activeness,
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
    Solution,
    SampleSet,
    Sos1Constraint,
)


def test_ordered_selection_and_evaluated_skipped_rows():
    assert Activeness.SKIP_FEASIBILITY_CHECK < Activeness.INACTIVE < Activeness.ACTIVE
    model = Instance.minimize()
    x = model.new_binary("x")
    active = model.add_constraint(x <= 1)
    checked = model.add_constraint(x == 1)
    skipped = model.add_constraint(x >= 2)
    model.deactivate_constraint(checked.constraint_id, "report")
    model.deactivate_constraint(
        skipped.constraint_id, "diagnostic", skip_feasibility_check=True
    )
    solution = model.evaluate({x.id: 0})
    assert solution.feasible(min_activeness=Activeness.ACTIVE)
    assert solution.feasible_relaxed
    assert not solution.feasible()
    assert solution.total_violation(min_activeness=Activeness.ACTIVE) == 0
    assert solution.total_violation() == 1
    assert (
        solution.total_violation(min_activeness=Activeness.SKIP_FEASIBILITY_CHECK) == 3
    )
    assert solution.constraints[skipped.constraint_id].violation() == 2
    assert (
        solution.constraints[skipped.constraint_id].activeness
        == Activeness.SKIP_FEASIBILITY_CHECK
    )
    for owner in (model, solution, model.evaluate_samples([{x.id: 0}])):
        table = owner.constraints_df(kind=ConstraintKind.Regular)
        assert set(table.index) == {
            active.constraint_id,
            checked.constraint_id,
            skipped.constraint_id,
        }
        assert set(
            owner.constraints_df(
                kind=ConstraintKind.Regular, min_activeness=Activeness.INACTIVE
            ).index
        ) == {active.constraint_id, checked.constraint_id}
        assert list(
            owner.constraints_df(
                kind=ConstraintKind.Regular, min_activeness=Activeness.ACTIVE
            ).index
        ) == [active.constraint_id]


def test_all_state_mapping_and_stale_attached_handle():
    model = Instance.minimize()
    x = model.new_binary("x")
    attached = model.add_constraint(x == 1)
    cid = attached.constraint_id
    rows = model.constraints
    assert_type(rows[cid], ConstraintView)
    assert_type(rows.get(ConstraintID(99)), ConstraintView | None)
    assert_type(rows.get(ConstraintID(99), rows[cid]), ConstraintView)
    assert_type(rows.of_kind(ConstraintKind.Regular)[cid], ConstraintView)
    assert isinstance(rows[cid], AttachedConstraint)
    model.deactivate_constraint(cid, "report", source="test")
    assert isinstance(rows[cid], InactiveConstraint)
    assert attached.activeness == Activeness.INACTIVE
    with pytest.raises(KeyError):
        model.active_constraints[cid]
    with pytest.raises(RuntimeError, match="no longer active"):
        attached.set_name("stale")
    snapshot = model.inactive_constraints[cid]
    model.set_constraint_skip_feasibility_check(cid, True)
    assert rows[cid].activeness == Activeness.SKIP_FEASIBILITY_CHECK
    assert attached.evaluate({x.id: 0}).activeness == Activeness.SKIP_FEASIBILITY_CHECK
    assert snapshot.activeness == Activeness.INACTIVE
    model.activate_constraint(cid)
    assert isinstance(rows[cid], AttachedConstraint)
    assert attached.activeness == Activeness.ACTIVE
    attached.set_name("active again")
    assert rows[cid].name == "active again"


@pytest.mark.parametrize("host", [Instance, ParametricInstance])
def test_initial_inactive_components_and_all_family_transitions(host):
    x, y = DecisionVariable.binary(0), DecisionVariable.binary(1)
    kwargs: dict[str, Any] = dict(
        sense=Sense.Minimize,
        objective=0,
        decision_variables=[x, y],
        active_constraints={},
        inactive_constraints={
            1: InactiveConstraint(x == 1, "report", skip_feasibility_check=True)
        },
        inactive_indicator_constraints={
            2: InactiveIndicatorConstraint(
                (y == 1).with_indicator(x), "report", skip_feasibility_check=True
            )
        },
        inactive_one_hot_constraints={
            3: InactiveOneHotConstraint(
                OneHotConstraint(variables=[x, y]),
                "report",
                skip_feasibility_check=True,
            )
        },
        inactive_sos1_constraints={
            4: InactiveSos1Constraint(
                Sos1Constraint(variables=[x, y]), "report", skip_feasibility_check=True
            )
        },
    )
    if host is ParametricInstance:
        kwargs["parameters"] = []
    model = host.from_components(**kwargs)
    assert len(model.constraints) == len(model.inactive_constraints) == 4
    assert not model.active_constraints
    for family, cid in zip(("", "indicator_", "one_hot_", "sos1_"), range(1, 5)):
        getattr(model, f"set_{family}constraint_skip_feasibility_check")(cid, False)
        getattr(model, f"activate_{family}constraint")(cid)
        getattr(model, f"deactivate_{family}constraint")(
            cid, "again", skip_feasibility_check=True
        )
    assert all(
        row.activeness == Activeness.SKIP_FEASIBILITY_CHECK
        for row in model.constraints.values()
    )
    restored = host.from_v2_bytes(model.to_v2_bytes())
    assert all(
        row.activeness == Activeness.SKIP_FEASIBILITY_CHECK
        for row in restored.constraints.values()
    )
    instance = (
        model.with_parameters({}) if isinstance(model, ParametricInstance) else model
    )
    evaluated = instance.evaluate({x.id: 1, y.id: 1})
    assert evaluated.feasible()
    assert not evaluated.feasible(min_activeness=Activeness.SKIP_FEASIBILITY_CHECK)
    assert evaluated.total_violation() == 0
    assert (
        evaluated.total_violation(min_activeness=Activeness.SKIP_FEASIBILITY_CHECK) == 2
    )
    with pytest.raises(RuntimeError, match="skip_feasibility_check"):
        model.to_v1_bytes()


def test_sample_selection_and_wire_round_trip():
    model = Instance.maximize()
    x = model.new_binary("x")
    model.objective = x
    row = model.add_constraint(x == 0)
    model.deactivate_constraint(row.constraint_id, "report")
    samples = model.evaluate_samples({0: {x.id: 0}, 1: {x.id: 1}})
    assert samples.feasible() == {0: True, 1: False}
    assert (
        samples.feasible(min_activeness=Activeness.ACTIVE) == samples.feasible_relaxed
    )
    assert samples.best_feasible_id() == 0
    assert samples.best_feasible_id(min_activeness=Activeness.ACTIVE) == 1
    assert samples.best_feasible(min_activeness=Activeness.ACTIVE).objective == 1
    assert samples.feasible_ids(min_activeness=Activeness.ACTIVE) == {0, 1}
    model.set_constraint_skip_feasibility_check(row.constraint_id, True)
    solution = model.evaluate({x.id: 1})
    samples = model.evaluate_samples({1: {x.id: 1}})
    for result, cls in ((solution, Solution), (samples, SampleSet)):
        restored = cls.from_v2_bytes(result.to_v2_bytes())
        assert (
            restored.constraints[row.constraint_id].activeness
            == Activeness.SKIP_FEASIBILITY_CHECK
        )
        with pytest.raises(RuntimeError, match="skip_feasibility_check"):
            result.to_v1_bytes()


def test_empty_constraint_frames_keep_activeness_and_requested_reason():
    model = Instance.minimize()
    for owner in (model, model.evaluate({}), model.evaluate_samples([{}])):
        frame = owner.constraints_df(
            kind=ConstraintKind.Regular, include=["inactive_reason"]
        )
        assert frame.empty
        assert "activeness" in frame.columns
        assert "inactive_reason" in frame.columns
