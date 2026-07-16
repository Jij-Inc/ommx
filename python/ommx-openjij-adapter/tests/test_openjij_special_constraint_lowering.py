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

    preparation = OMMXOpenJijSAAdapter.prepare(
        instance,
        uniform_penalty_weight=2.0,
    )
    adapter_input = preparation.input

    assert adapter_input.required_capabilities == set()
    assert adapter_input.one_hot_constraints == {}
    constraints = list(adapter_input.removed_constraints.values())
    assert len(constraints) == 1
    assert constraints[0].provenance[-1].kind == ProvenanceKind.OneHotConstraint
    assert constraints[0].provenance[-1].original_id == 10
