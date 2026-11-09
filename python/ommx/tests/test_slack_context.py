import pytest

from ommx import (
    ConstraintID,
    DecisionVariable,
    Equality,
    Instance,
    ProvenanceKind,
    Sense,
    Solution,
    SampleSet,
)


@pytest.mark.parametrize("exact", [True, False])
def test_slack_retains_source_and_records_generated_row(exact):
    x = DecisionVariable.integer(0, lower=0, upper=2)
    source = (x <= 1).set_name("capacity").set_subscripts([7])
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[x],
        constraints={0: source},
    )
    if exact:
        instance.convert_inequality_to_equality_with_integer_slack(0, 10)
    else:
        assert instance.add_integer_slack_to_inequality(0, 2) == 0.5

    for restored in (instance, Instance.from_v2_bytes(instance.to_v2_bytes())):
        retained = restored.removed_constraints[ConstraintID(0)]
        generated_id = ConstraintID(
            int(retained.removed_reason_parameters["constraint_id"])
        )
        generated = restored.constraints[generated_id]
        assert retained.function == source.function
        assert retained.equality == Equality.LessThanOrEqualToZero
        assert retained.name == generated.name == "capacity"
        assert retained.subscripts == generated.subscripts == [7]
        assert generated.provenance[-1].kind == ProvenanceKind.Constraint
        assert generated.provenance[-1].original_id == ConstraintID(0)
        assert repr(generated.provenance[-1]) == "Provenance(Constraint(0))"

        solution = restored.evaluate({0: 1, 1: 1})
        for result in (solution, Solution.from_v2_bytes(solution.to_v2_bytes())):
            assert result.constraints[ConstraintID(0)].is_feasible(atol=1e-6)
            assert not result.constraints[generated_id].is_feasible(atol=1e-6)
            assert result.constraints[generated_id].provenance == generated.provenance
        samples = restored.evaluate_samples({3: {0: 1, 1: 1}})
        for result in (samples, SampleSet.from_v2_bytes(samples.to_v2_bytes())):
            assert result.constraints[ConstraintID(0)].feasible(atol=1e-6) == {3: True}
            assert result.constraints[generated_id].feasible(atol=1e-6) == {3: False}
            assert result.constraints[generated_id].provenance == generated.provenance


def test_slack_chain_keeps_original_rows_after_encoding():
    x = DecisionVariable.integer(0, lower=0, upper=2)
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[x],
        constraints={0: x <= 1},
    )
    instance.add_integer_slack_to_inequality(0, 1)
    intermediate = instance.constraints[ConstraintID(1)].function
    instance.convert_inequality_to_equality_with_integer_slack(1, 10)
    assert [
        p.original_id for p in instance.constraints[ConstraintID(2)].provenance
    ] == [ConstraintID(0), ConstraintID(1)]
    instance.log_encode()
    assert instance.removed_constraints[ConstraintID(1)].function == intermediate


@pytest.mark.parametrize("method", ["to_qubo", "to_hubo"])
@pytest.mark.parametrize("max_range", [1, 10])
def test_preparation_carries_source_weight_to_generated_row(method, max_range):
    def model():
        x = DecisionVariable.integer(0, lower=0, upper=4)
        return Instance.from_components(
            sense=Sense.Minimize,
            objective=x,
            decision_variables=[x],
            constraints={7: x <= 2},
        )

    kwargs = {"inequality_integer_slack_max_range": max_range}
    expected = getattr(model(), method)(uniform_penalty_weight=3, **kwargs)
    assert getattr(model(), method)(penalty_weights={7: 3}, **kwargs) == expected
    assert getattr(model(), method)(penalty_weights={8: 3}, **kwargs) == expected
    for weights in ({99: 3}, {7: 3, 8: 3}):
        with pytest.raises(ValueError, match="Fixed penalty weights must match"):
            getattr(model(), method)(penalty_weights=weights, **kwargs)
