import copy

from ommx import (
    AdditionalCapability,
    DecisionVariable,
    Instance,
    OneHotConstraint,
    ProvenanceKind,
    Sos1Constraint,
)
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter


def test_adapter_accepts_an_explicitly_prepared_input() -> None:
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

    prepared = copy.copy(instance)
    prepared.reduce_capabilities(
        {
            AdditionalCapability.Indicator,
            AdditionalCapability.Sos1,
        }
    )

    assert set(instance.one_hot_constraints) == {10}
    assert prepared.required_capabilities == {
        AdditionalCapability.Indicator,
        AdditionalCapability.Sos1,
    }
    assert prepared.one_hot_constraints == {}
    assert set(prepared.indicator_constraints) == {30}
    assert set(prepared.sos1_constraints) == {20}

    lowered = [
        constraint
        for constraint in prepared.constraints.values()
        if constraint.provenance
        and constraint.provenance[-1].kind == ProvenanceKind.OneHotConstraint
    ]
    assert len(lowered) == 1
    assert lowered[0].provenance[-1].original_id == 10

    report = OMMXPySCIPOptAdapter.check_applicability(prepared)
    assert report.is_applicable
    adapter = OMMXPySCIPOptAdapter(prepared)
    assert adapter.instance is prepared

    constraint_names = {constraint.name for constraint in adapter.model.getConss()}
    assert "ind_30" in constraint_names
    assert "sos1_20" in constraint_names
