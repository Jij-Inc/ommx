from ommx import (
    AdditionalCapability,
    DecisionVariable,
    Instance,
    OneHotConstraint,
    ProvenanceKind,
    Sos1Constraint,
)
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter


def test_adapter_lowers_only_unsupported_special_constraint_families() -> None:
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
        sense=Instance.MAXIMIZE,
    )

    adapter = OMMXPySCIPOptAdapter(instance)

    assert adapter.instance.required_capabilities == {
        AdditionalCapability.Indicator,
        AdditionalCapability.Sos1,
    }
    assert adapter.instance.one_hot_constraints == {}
    assert set(adapter.instance.indicator_constraints) == {30}
    assert set(adapter.instance.sos1_constraints) == {20}

    lowered = [
        constraint
        for constraint in adapter.instance.constraints.values()
        if constraint.provenance
        and constraint.provenance[-1].kind == ProvenanceKind.OneHotConstraint
    ]
    assert len(lowered) == 1
    assert lowered[0].provenance[-1].original_id == 10

    constraint_names = {constraint.name for constraint in adapter.model.getConss()}
    assert "ind_30" in constraint_names
    assert "sos1_20" in constraint_names
