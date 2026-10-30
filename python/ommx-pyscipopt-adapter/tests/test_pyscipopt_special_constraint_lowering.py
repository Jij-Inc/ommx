from ommx import ConstraintID
from ommx import IndicatorConstraintID, OneHotConstraintID, Sos1ConstraintID
import copy

import pytest

from ommx import (
    DecisionVariable,
    Instance,
    OneHotConstraint,
    ProvenanceKind,
    Sense,
    Sos1Constraint,
    SpecialConstraintKind,
)
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter
from ommx.adapter import AdapterNotApplicableError


def test_recommended_preparation_lowers_only_one_hot() -> None:
    indicator = DecisionVariable.binary(0)
    x = [DecisionVariable.binary(i) for i in range(1, 3)]
    value = DecisionVariable.continuous(3, lower=0, upper=2)
    instance = Instance.from_components(
        decision_variables=[indicator, *x, value],
        objective=value,
        constraints={},
        indicator_constraints={30: (value <= 1).with_indicator(indicator)},
        one_hot_constraints={10: OneHotConstraint(variables=x)},
        sos1_constraints={20: Sos1Constraint(variables=x)},
        sense=Sense.Maximize,
    )
    before = instance.to_v2_bytes()

    with pytest.raises(AdapterNotApplicableError):
        OMMXPySCIPOptAdapter.solve_without_preparation(instance)
    assert instance.to_v2_bytes() == before

    solution = OMMXPySCIPOptAdapter.solve(instance)
    assert solution.feasible
    assert instance.to_v2_bytes() == before

    prepared = copy.copy(instance)
    input_class = OMMXPySCIPOptAdapter.INPUT_CLASS
    prepared.prepare(
        input_class,
        OMMXPySCIPOptAdapter.recommended_preparation_policy(),
    )

    assert set(
        (cid for cid in instance.constraints if isinstance(cid, OneHotConstraintID))
    ) == {OneHotConstraintID(10)}
    assert prepared.active_special_constraint_kinds == {
        SpecialConstraintKind.Indicator,
        SpecialConstraintKind.Sos1,
    }
    assert not any(isinstance(cid, OneHotConstraintID) for cid in prepared.constraints)
    assert set(
        (cid for cid in prepared.constraints if isinstance(cid, IndicatorConstraintID))
    ) == {IndicatorConstraintID(30)}
    assert set(
        (cid for cid in prepared.constraints if isinstance(cid, Sos1ConstraintID))
    ) == {Sos1ConstraintID(20)}

    lowered = [
        constraint
        for constraint in (
            prepared.constraints[cid]
            for cid in prepared.constraints
            if isinstance(cid, ConstraintID)
        )
        if constraint.provenance
        and constraint.provenance[-1].kind == ProvenanceKind.OneHotConstraint
    ]
    assert len(lowered) == 1
    assert lowered[0].provenance[-1].original_id == OneHotConstraintID(10)

    assert input_class.contains(prepared)
    report = OMMXPySCIPOptAdapter.check_applicability(prepared)
    assert report.is_member
    adapter = OMMXPySCIPOptAdapter(prepared)
    assert adapter.instance is prepared

    constraint_names = {constraint.name for constraint in adapter.model.getConss()}
    assert f"ind_{IndicatorConstraintID(30)}" in constraint_names
    assert f"sos1_{Sos1ConstraintID(20)}" in constraint_names
