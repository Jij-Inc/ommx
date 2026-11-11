"""Ordered roles select one coherent constraint set across query APIs."""

import pickle
from typing import Any

import pytest
from typing_extensions import assert_type

from ommx import (
    AttachedConstraint,
    ConstraintRole as Role,
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
    OneHotConstraintID,
    ParametricInstance,
    Sense,
    Solution,
    SampleSet,
    Sos1Constraint,
    Sos1ConstraintID,
)


@pytest.mark.parametrize(
    ("member", "name", "code"),
    [
        (Role.EVALUATION_ONLY, "EVALUATION_ONLY", 0),
        (Role.FEASIBILITY_CHECK, "FEASIBILITY_CHECK", 1),
        (Role.ACTIVE, "ACTIVE", 2),
    ],
)
def test_role_member_names_and_pickle(member, name, code):
    assert repr(member) == f"ConstraintRole.{name}"
    assert str(member) == f"ConstraintRole.{name}"
    assert int(member) == code
    assert pickle.loads(pickle.dumps(member)) == getattr(Role, name)


def test_role_old_member_names_are_absent():
    assert not hasattr(Role, "INACTIVE")
    assert not hasattr(Role, "SKIP_FEASIBILITY_CHECK")


def test_ordered_selection_and_evaluated_skipped_rows():
    assert Role.EVALUATION_ONLY < Role.FEASIBILITY_CHECK < Role.ACTIVE
    model = Instance.minimize()
    x = model.new_binary("x")
    active = model.add_constraint(x <= 1)
    checked = model.add_constraint(x == 1)
    skipped = model.add_constraint(x >= 2)
    model.deactivate_constraint(checked.constraint_id, "report")
    model.deactivate_constraint(
        skipped.constraint_id, "diagnostic", skip_feasibility_check=True
    )
    for host in (model, model.as_parametric_instance()):
        assert set(host.inactive_constraints) == {
            checked.constraint_id,
            skipped.constraint_id,
        }
        assert host.inactive_constraints[checked.constraint_id].role == (
            Role.FEASIBILITY_CHECK
        )
        assert host.inactive_constraints[skipped.constraint_id].role == (
            Role.EVALUATION_ONLY
        )
    solution = model.evaluate({x.id: 0})
    assert_type(solution.feasible, bool)
    assert solution.feasible is False
    assert solution.feasible_relaxed is True
    assert solution.feasible_unrelaxed is False
    assert solution.total_violation(min_role=Role.ACTIVE) == 0
    assert solution.total_violation() == 1
    assert solution.total_violation(min_role=Role.EVALUATION_ONLY) == 3
    assert solution.constraints[skipped.constraint_id].violation() == 2
    assert solution.constraints[skipped.constraint_id].role == Role.EVALUATION_ONLY
    checked_feasible = model.evaluate({x.id: 1})
    assert checked_feasible.feasible is True
    assert (
        checked_feasible.constraints[skipped.constraint_id].is_feasible(
            atol=checked_feasible.feasibility_atol
        )
        is False
    )
    samples = model.evaluate_samples({0: {x.id: 0}, 1: {x.id: 1}})
    assert_type(samples.feasible, dict[int, bool])
    assert_type(samples.best_feasible, Solution)
    assert_type(samples.best_feasible_id, int)
    assert samples.feasible == {0: False, 1: True}
    assert samples.feasible_ids() == {1}
    assert samples.best_feasible_id == 1
    assert samples.best_feasible.state.entries == {x.id: 1.0}
    for owner in (model, solution, model.evaluate_samples([{x.id: 0}])):
        table = owner.constraints_df(kind=ConstraintKind.Regular)
        assert set(table.index) == {
            active.constraint_id,
            checked.constraint_id,
            skipped.constraint_id,
        }
        assert table["role"].to_dict() == {
            active.constraint_id: Role.ACTIVE,
            checked.constraint_id: Role.FEASIBILITY_CHECK,
            skipped.constraint_id: Role.EVALUATION_ONLY,
        }
        assert set(
            owner.constraints_df(
                kind=ConstraintKind.Regular, min_role=Role.FEASIBILITY_CHECK
            ).index
        ) == {active.constraint_id, checked.constraint_id}
        assert list(
            owner.constraints_df(
                kind=ConstraintKind.Regular, min_role=Role.ACTIVE
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
    assert attached.role == Role.FEASIBILITY_CHECK
    with pytest.raises(KeyError):
        model.active_constraints[cid]
    with pytest.raises(RuntimeError, match="no longer active"):
        attached.set_name("stale")
    snapshot = model.inactive_constraints[cid]
    model.set_constraint_skip_feasibility_check(cid, True)
    assert rows[cid].role == Role.EVALUATION_ONLY
    assert attached.evaluate({x.id: 0}).role == Role.EVALUATION_ONLY
    assert snapshot.role == Role.FEASIBILITY_CHECK
    model.activate_constraint(cid)
    assert isinstance(rows[cid], AttachedConstraint)
    assert attached.role == Role.ACTIVE
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
    assert all(row.role == Role.EVALUATION_ONLY for row in model.constraints.values())
    restored = host.from_v2_bytes(model.to_v2_bytes())
    assert all(
        row.role == Role.EVALUATION_ONLY for row in restored.constraints.values()
    )
    instance = (
        model.with_parameters({}) if isinstance(model, ParametricInstance) else model
    )
    evaluated = instance.evaluate({x.id: 1, y.id: 1})
    assert evaluated.feasible is True
    assert evaluated.feasible_relaxed is True
    assert (
        evaluated.constraints[OneHotConstraintID(3)].is_feasible(
            atol=evaluated.feasibility_atol
        )
        is False
    )
    assert (
        evaluated.constraints[Sos1ConstraintID(4)].is_feasible(
            atol=evaluated.feasibility_atol
        )
        is False
    )
    assert evaluated.total_violation() == 0
    assert evaluated.total_violation(min_role=Role.EVALUATION_ONLY) == 2
    with pytest.raises(RuntimeError, match="skip_feasibility_check"):
        model.to_v1_bytes()


def test_sample_selection_and_wire_round_trip():
    model = Instance.maximize()
    x = model.new_binary("x")
    model.objective = x
    row = model.add_constraint(x == 0)
    model.deactivate_constraint(row.constraint_id, "report")
    samples = model.evaluate_samples({0: {x.id: 0}, 1: {x.id: 1}})
    assert samples.feasible == {0: True, 1: False}
    assert samples.feasible_relaxed == {0: True, 1: True}
    assert samples.feasible_unrelaxed == samples.feasible
    assert samples.best_feasible_id == 0
    assert samples.best_feasible.objective == 0
    assert samples.best_feasible_relaxed_id == 1
    assert samples.best_feasible_relaxed.objective == 1
    assert samples.feasible_ids() == {0}
    assert samples.feasible_relaxed_ids() == {0, 1}
    model.set_constraint_skip_feasibility_check(row.constraint_id, True)
    solution = model.evaluate({x.id: 1})
    samples = model.evaluate_samples({1: {x.id: 1}})
    assert solution.feasible is True
    assert samples.feasible == {1: True}
    assert samples.best_feasible_id == samples.best_feasible_relaxed_id == 1
    for result, cls in ((solution, Solution), (samples, SampleSet)):
        restored = cls.from_v2_bytes(result.to_v2_bytes())
        assert restored.constraints[row.constraint_id].role == Role.EVALUATION_ONLY
        with pytest.raises(RuntimeError, match="skip_feasibility_check"):
            result.to_v1_bytes()


def test_empty_constraint_frames_keep_role_and_requested_reason():
    model = Instance.minimize()
    for owner in (model, model.evaluate({}), model.evaluate_samples([{}])):
        frame = owner.constraints_df(
            kind=ConstraintKind.Regular, include=["inactive_reason"]
        )
        assert frame.empty
        assert "role" in frame.columns
        assert "inactive_reason" in frame.columns
