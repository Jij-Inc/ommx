from ommx import (
    DecisionVariable,
    Instance,
    OneHotConstraint,
    ProvenanceKind,
    SpecialConstraintKind,
)
from ommx.adapter import ConstraintRef
from ommx_openjij_adapter import (
    OMMXOpenJijSAAdapter,
    OpenJijPreparationPolicy,
)


def test_preparation_explicitly_lowers_special_constraints() -> None:
    x = [DecisionVariable.binary(i) for i in range(2)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        one_hot_constraints={10: OneHotConstraint(variables=x)},
        sense=Instance.MINIMIZE,
    )

    preparation = OMMXOpenJijSAAdapter.prepare(
        instance,
        policy=OpenJijPreparationPolicy(uniform_penalty_weight=2.0),
    )
    adapter_input = preparation.input

    assert adapter_input.active_special_constraint_kinds == set()
    assert adapter_input.one_hot_constraints == {}
    constraints = list(adapter_input.removed_constraints.values())
    assert len(constraints) == 1
    assert constraints[0].provenance[-1].kind == ProvenanceKind.OneHotConstraint
    assert constraints[0].provenance[-1].original_id == 10
    [transform] = [
        transform
        for transform in preparation.report.transforms
        if transform.name == "one_hot_lowering"
    ]
    assert transform.special_constraint_kinds == frozenset(
        {SpecialConstraintKind.OneHot}
    )


def test_preparation_policy_can_forbid_active_special_constraint_lowering() -> None:
    x = [DecisionVariable.binary(i) for i in range(2)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        one_hot_constraints={10: OneHotConstraint(variables=x)},
        sense=Instance.MINIMIZE,
    )
    before = instance.to_v2_bytes()

    report = OMMXOpenJijSAAdapter.check_preparation(
        instance,
        policy=OpenJijPreparationPolicy(
            allowed_special_constraint_lowerings=frozenset(),
            uniform_penalty_weight=2.0,
        ),
    )

    assert report.transforms == ()
    [failure] = report.preparation_failures
    assert failure.name == "one_hot_lowering"
    assert failure.reason == "openjij.special_constraint_lowering.policy"
    assert failure.constraint_refs == frozenset({ConstraintRef("one_hot", 10)})
    assert instance.to_v2_bytes() == before


def test_adapter_declares_ordered_special_constraint_lowering_candidates() -> None:
    assert OMMXOpenJijSAAdapter.PREPARATION_SPECIAL_CONSTRAINT_LOWERINGS == (
        SpecialConstraintKind.Indicator,
        SpecialConstraintKind.OneHot,
        SpecialConstraintKind.Sos1,
    )
