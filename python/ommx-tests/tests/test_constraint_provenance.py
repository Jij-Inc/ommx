"""Tests for Constraint.provenance metadata (issue #819)."""

from ommx.v1 import (
    Constraint,
    DecisionVariable,
    Instance,
    OneHotConstraint,
    Provenance,
    ProvenanceKind,
    Samples,
    Sos1Constraint,
    State,
)


def test_user_authored_constraint_has_empty_provenance():
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={
            5: Constraint(function=x, equality=Constraint.LESS_THAN_OR_EQUAL_TO_ZERO),
        },
        sense=Instance.MINIMIZE,
    )
    c = instance.get_constraint_by_id(5)
    assert c.provenance == []


def test_convert_one_hot_populates_provenance():
    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        one_hot_constraints={10: OneHotConstraint(variables=[1, 2, 3])},
        sense=Instance.MINIMIZE,
    )
    new_id = instance.convert_one_hot_to_constraint(10)

    c = instance.get_constraint_by_id(new_id)
    assert len(c.provenance) == 1
    assert c.provenance[0].kind == ProvenanceKind.OneHotConstraint
    assert c.provenance[0].original_id == 10


def test_convert_sos1_populates_provenance_on_every_generated_constraint():
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        sos1_constraints={7: Sos1Constraint(variables=[0, 1, 2])},
        sense=Instance.MINIMIZE,
    )
    new_ids = instance.convert_sos1_to_constraints(7)

    assert new_ids, "convert_sos1_to_constraints should emit at least one constraint"
    for nid in new_ids:
        c = instance.get_constraint_by_id(nid)
        assert c.provenance, f"constraint {nid} has no provenance"
        last = c.provenance[-1]
        assert last.kind == ProvenanceKind.Sos1Constraint
        assert last.original_id == 7


def test_convert_indicator_populates_provenance():
    y = DecisionVariable.binary(0, name="y")
    x1 = DecisionVariable.continuous(1, lower=0, upper=10)
    x2 = DecisionVariable.continuous(2, lower=0, upper=10)

    body = Constraint(
        function=x1 + x2 - 5,
        equality=Constraint.LESS_THAN_OR_EQUAL_TO_ZERO,
    )
    indicator = body.with_indicator(y)

    instance = Instance.from_components(
        decision_variables=[y, x1, x2],
        objective=x1 + x2,
        constraints={},
        indicator_constraints={42: indicator},
        sense=Instance.MINIMIZE,
    )
    new_ids = instance.convert_indicator_to_constraint(42)

    assert new_ids
    for nid in new_ids:
        c = instance.get_constraint_by_id(nid)
        assert c.provenance
        last = c.provenance[-1]
        assert last.kind == ProvenanceKind.IndicatorConstraint
        assert last.original_id == 42


def test_provenance_is_preserved_through_evaluate():
    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        one_hot_constraints={10: OneHotConstraint(variables=[1, 2, 3])},
        sense=Instance.MINIMIZE,
    )
    new_id = instance.convert_one_hot_to_constraint(10)

    solution = instance.evaluate(State({1: 0.0, 2: 1.0, 3: 0.0}))
    evaluated = solution.get_constraint_by_id(new_id)

    assert len(evaluated.provenance) == 1
    assert evaluated.provenance[0].kind == ProvenanceKind.OneHotConstraint
    assert evaluated.provenance[0].original_id == 10


def test_provenance_is_preserved_through_evaluate_samples():
    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        one_hot_constraints={10: OneHotConstraint(variables=[1, 2, 3])},
        sense=Instance.MINIMIZE,
    )
    new_id = instance.convert_one_hot_to_constraint(10)

    samples = Samples(
        {
            0: {1: 1.0, 2: 0.0, 3: 0.0},
            1: {1: 0.0, 2: 1.0, 3: 0.0},
        }
    )
    sample_set = instance.evaluate_samples(samples)
    sampled = sample_set.get_constraint_by_id(new_id)

    assert len(sampled.provenance) == 1
    assert sampled.provenance[0].kind == ProvenanceKind.OneHotConstraint
    assert sampled.provenance[0].original_id == 10


def test_provenance_equality_and_hash():
    # sanity: Provenance is importable as a concrete type from ommx.v1
    assert Provenance is not None

    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        one_hot_constraints={10: OneHotConstraint(variables=[1, 2, 3])},
        sense=Instance.MINIMIZE,
    )
    new_id = instance.convert_one_hot_to_constraint(10)
    p1 = instance.get_constraint_by_id(new_id).provenance[0]
    p2 = instance.get_constraint_by_id(new_id).provenance[0]

    assert p1 == p2
    assert ProvenanceKind.OneHotConstraint == ProvenanceKind.OneHotConstraint
    assert ProvenanceKind.OneHotConstraint != ProvenanceKind.Sos1Constraint

    # Equal values must hash equally — required for dict / set correctness.
    assert hash(p1) == hash(p2)
    assert {p1, p2} == {p1}
    assert {p1: "tag"}[p2] == "tag"
