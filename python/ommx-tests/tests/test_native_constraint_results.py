"""Identity survives every family, evaluation stage and wire round trip."""

from ommx import ConstraintKind

import operator
import subprocess
import sys
from typing_extensions import assert_type

import pytest

from ommx import (
    ConstraintID,
    DecisionVariable,
    Equality,
    EvaluatedConstraint,
    EvaluatedIndicatorConstraint,
    EvaluatedOneHotConstraint,
    EvaluatedSos1Constraint,
    IndicatorConstraint,
    IndicatorConstraintID,
    Instance,
    OneHotConstraint,
    OneHotConstraintID,
    SampledConstraint,
    SampledIndicatorConstraint,
    SampledOneHotConstraint,
    SampledSos1Constraint,
    SampleSet,
    Sense,
    Solution,
    Sos1Constraint,
    Sos1ConstraintID,
)

FAMILIES = [
    (ConstraintKind.Regular, ConstraintID, EvaluatedConstraint, SampledConstraint),
    (
        ConstraintKind.Indicator,
        IndicatorConstraintID,
        EvaluatedIndicatorConstraint,
        SampledIndicatorConstraint,
    ),
    (
        ConstraintKind.OneHot,
        OneHotConstraintID,
        EvaluatedOneHotConstraint,
        SampledOneHotConstraint,
    ),
    (
        ConstraintKind.Sos1,
        Sos1ConstraintID,
        EvaluatedSos1Constraint,
        SampledSos1Constraint,
    ),
]


NUMBERS = {id_type: 7 + i for i, (_, id_type, _, _) in enumerate(FAMILIES)}


def model():
    x, y = DecisionVariable.binary(0), DecisionVariable.binary(1)
    return Instance.from_components(
        sense=Sense.Minimize,
        objective=x + y,
        decision_variables=[x, y],
        constraints={
            ConstraintID(7): (x + y == 1)
            .set_name("balance")
            .set_parameters({"group": "a"})
        },
        indicator_constraints={
            IndicatorConstraintID(8): IndicatorConstraint(
                indicator_variable=x, function=y - 1, equality=Equality.EqualToZero
            )
        },
        one_hot_constraints={OneHotConstraintID(9): OneHotConstraint(variables=[x, y])},
        sos1_constraints={Sos1ConstraintID(10): Sos1Constraint(variables=[x, y])},
    )


def test_family_identity_and_no_integer_escape():
    ids = [constructor(7) for _, constructor, _, _ in FAMILIES]
    assert len(set(ids + [7])) == 5
    for id_, (_, constructor, _, _) in zip(ids, FAMILIES):
        assert id_ == constructor(7)
        assert hash(id_) == hash(constructor(7))
        assert {id_: "found"}[constructor(7)] == "found"
        assert 7 not in {id_: "found"}
        assert repr(id_) == f"{constructor.__name__}(7)"
        with pytest.raises(TypeError):
            int(id_)
        with pytest.raises(TypeError):
            operator.index(id_)
        with pytest.raises(OverflowError):
            constructor(-1)
        with pytest.raises(OverflowError):
            constructor(2**64)


def test_reject_wrong_family_before_mutation_and_ambiguous_integer():
    instance = model()
    before = instance.to_v2_bytes()
    for wrong in [
        IndicatorConstraintID(7),
        OneHotConstraintID(7),
        Sos1ConstraintID(7),
    ]:
        with pytest.raises(TypeError):
            instance.constraints.of_kind(ConstraintKind.Regular)[wrong]
        with pytest.raises(TypeError):
            instance.relax_constraint(wrong, "manual")
        solution = instance.evaluate({0: 1, 1: 0})
        with pytest.raises(TypeError):
            solution.set_dual_variable(wrong, 1.0)
    assert instance.to_v2_bytes() == before
    solution = instance.evaluate({0: 1, 1: 0})
    with pytest.raises(TypeError):
        solution.constraint_violation(7)  # type: ignore[arg-type] -- test rejection of bare integers
    assert solution.constraint_violation(ConstraintID(7)) == 0
    assert solution.constraint_violation(IndicatorConstraintID(8)) == 1
    assert solution.constraint_violation(Sos1ConstraintID(10)) == 0


@pytest.mark.parametrize("kind,id_type,evaluated_type,sampled_type", FAMILIES)
def test_all_families_native_dataframe_and_roundtrip(
    kind, id_type, evaluated_type, sampled_type
):
    instance = model()
    samples = instance.evaluate_samples({3: {0: 1, 1: 0}, 91: {0: 1, 1: 1}})
    for result in [samples, SampleSet.from_v2_bytes(samples.to_v2_bytes())]:
        rows = result.constraints
        assert set(rows) == {
            constructor(NUMBERS[constructor]) for _, constructor, _, _ in FAMILIES
        }
        row = rows[id_type(NUMBERS[id_type])]
        assert isinstance(row, sampled_type)
        assert set(row.violations) == {3, 91}
        assert set(row.feasible(atol=1e-6)) == {3, 91}
        assert row.used_decision_variable_ids == {0, 1}
        frame = result.constraints_df(kind=kind)
        assert set(frame.index) == {key for key in rows if isinstance(key, id_type)}
        for sample_id in [3, 91]:
            solution = result.get(sample_id)
            for restored in [solution, Solution.from_v2_bytes(solution.to_v2_bytes())]:
                evaluated = restored.constraints[id_type(NUMBERS[id_type])]
                assert isinstance(evaluated, evaluated_type)
                assert evaluated.violation() == row.violations[sample_id]
                assert (
                    evaluated.is_feasible(atol=1e-6)
                    == row.feasible(atol=1e-6)[sample_id]
                )
                assert set(restored.constraints_df(kind=kind).index) == {
                    key for key in rows if isinstance(key, id_type)
                }
        with pytest.raises(AttributeError):
            getattr(rows, "clear")()
        assert set(rows) == {
            constructor(NUMBERS[constructor]) for _, constructor, _, _ in FAMILIES
        }


def test_removed_context_provenance_and_owned_snapshots():
    instance = model()
    generated = instance.convert_one_hot_to_constraint(OneHotConstraintID(9))
    instance.relax_constraint(ConstraintID(7), "manual", phase="test")
    samples = instance.evaluate_samples({3: {0: 1, 1: 0}})
    for root in [samples, samples.get(3)]:
        rows = root.constraints
        assert {key for key in rows if isinstance(key, ConstraintID)} == {
            ConstraintID(7),
            generated,
        }
        removed = rows[ConstraintID(7)]
        assert removed.name == "balance"
        assert removed.parameters == {"group": "a"}
        assert removed.removed_reason == "manual"
        assert removed.removed_reason_parameters == {"phase": "test"}
        assert rows[generated].provenance[0].original_id == OneHotConstraintID(9)
        assert root.constraints[OneHotConstraintID(9)].removed_reason is not None
        frame = root.constraint_provenance_df(kind=ConstraintKind.Regular)
        assert frame.iloc[0]["source_id"] == OneHotConstraintID(9)
        assert isinstance(frame.iloc[0]["regular_constraint_id"], ConstraintID)
    solution = samples.get(3)
    snapshot = solution.constraints[generated]
    snapshot.dual_variable = 12
    assert solution.get_dual_variable(generated) is None
    solution.set_dual_variable(generated, 5)
    assert snapshot.dual_variable == 12
    assert solution.constraints[generated].dual_variable == 5


def test_kind_required_keyword_only_and_invalid_kind():
    instance = model()
    for root in [
        instance.evaluate({0: 1, 1: 0}),
        instance.evaluate_samples({3: {0: 1, 1: 0}}),
    ]:
        for method in [root.constraints_df]:
            with pytest.raises(TypeError):
                method()
            with pytest.raises(TypeError):
                method("regular")
            with pytest.raises(TypeError, match="ConstraintKind"):
                method(kind="typo")


def test_native_query_does_not_import_pandas():
    subprocess.run(
        [
            sys.executable,
            "-c",
            """
import sys
sys.modules['pandas'] = None
from ommx import ConstraintKind, Instance
instance = Instance.empty()
x = instance.new_binary("x")
c = instance.add_constraint(x == 1)
regular = instance.constraints.of_kind(ConstraintKind.Regular)
assert regular[c.constraint_id] == c
assert list(regular.values()) == [c]
solution = instance.evaluate({x.id: 1})
assert solution.constraints[c.constraint_id].violation() == 0
assert solution.constraints.of_kind(ConstraintKind.Regular)[c.constraint_id].violation() == 0
samples = instance.evaluate_samples({4: {x.id: 1}})
assert samples.constraints[c.constraint_id].violations == {4: 0}
assert samples.constraints.of_kind(ConstraintKind.Regular)[c.constraint_id].violations == {4: 0}
""",
        ],
        check=True,
    )


def test_static_overloads():
    instance = model()
    solution = instance.evaluate({0: 1, 1: 0})
    samples = instance.evaluate_samples({3: {0: 1, 1: 0}})
    assert_type(solution.constraints[ConstraintID(7)], EvaluatedConstraint)
    assert_type(
        solution.constraints[IndicatorConstraintID(8)], EvaluatedIndicatorConstraint
    )
    assert_type(solution.constraints[OneHotConstraintID(9)], EvaluatedOneHotConstraint)
    assert_type(solution.constraints[Sos1ConstraintID(10)], EvaluatedSos1Constraint)
    assert_type(samples.constraints[ConstraintID(7)], SampledConstraint)
    assert_type(
        samples.constraints[IndicatorConstraintID(8)], SampledIndicatorConstraint
    )
    assert_type(samples.constraints[OneHotConstraintID(9)], SampledOneHotConstraint)
    assert_type(samples.constraints[Sos1ConstraintID(10)], SampledSos1Constraint)
