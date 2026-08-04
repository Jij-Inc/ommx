from ommx import (
    DecisionVariable,
    Instance,
    OneHotConstraint,
    ProvenanceKind,
)
from ommx_openjij_adapter import OMMXOpenJijSAAdapter


def test_preparation_explicitly_lowers_special_constraints() -> None:
    x = [DecisionVariable.binary(i) for i in range(2)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        one_hot_constraints={10: OneHotConstraint(variables=x)},
        sense=Instance.MINIMIZE,
    )

    instance.prepare(
        OMMXOpenJijSAAdapter.INPUT_CLASS,
        OMMXOpenJijSAAdapter.recommended_preparation_policy(
            uniform_penalty_weight=2.0,
        ),
    )

    assert instance.active_special_constraint_kinds == set()
    assert instance.one_hot_constraints == {}
    constraints = list(instance.removed_constraints.values())
    assert len(constraints) == 1
    assert constraints[0].provenance[-1].kind == ProvenanceKind.OneHotConstraint
    assert constraints[0].provenance[-1].original_id == 10
