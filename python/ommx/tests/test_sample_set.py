from ommx import Instance, DecisionVariable, Sense
import pandas as pd
import pytest


def test_evaluate_samples_type_check():
    """
    Reported case in bug report https://github.com/Jij-Inc/ommx/issues/393
    """
    x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0] + 2 * x[1] + 3 * x[2],
        active_constraints={0: x[1] + x[2] <= 1},
        sense=Sense.Maximize,
    )

    samples = [
        {0: 1, 1: 0, 2: 0},
        {0: 0, 1: 0, 2: 1},
        {0: 1, 1: 1, 2: 0},
    ]
    sample_set = instance.evaluate_samples(samples)

    assert sample_set.extract_decision_variables("x", 0) == {(0,): 1, (1,): 0, (2,): 0}
    assert sample_set.extract_decision_variables("x", 1) == {(0,): 0, (1,): 0, (2,): 1}
    assert sample_set.extract_decision_variables("x", 2) == {(0,): 1, (1,): 1, (2,): 0}


def test_sample_set_sense_minimize():
    instance = Instance.from_components(
        decision_variables=[],
        objective=0,
        active_constraints={},
        sense=Sense.Minimize,
    )
    sample_set = instance.evaluate_samples([{}])
    assert sample_set.sense == Sense.Minimize

    # Check that the sense is correct when creating a Solution
    # from a SampleSet using `.best_feasible`.
    solution = sample_set.best_feasible
    assert solution.sense == Sense.Minimize


def test_sample_set_sense_maximize():
    instance = Instance.from_components(
        decision_variables=[],
        objective=0,
        active_constraints={},
        sense=Sense.Maximize,
    )
    sample_set = instance.evaluate_samples([{}])
    assert sample_set.sense == Sense.Maximize

    # Check that the sense is correct when creating a Solution
    # from a SampleSet using `.get()`.
    sokution = sample_set.get(0)
    assert sokution.sense == Sense.Maximize


@pytest.mark.parametrize("sample_id", [7, 2**64 - 1])
def test_summary_schema_and_pandas_operations(sample_id):
    model = Instance.minimize()
    x = model.new_binary()
    model.objective = x
    samples = model.evaluate_samples({sample_id: {x.id: 1}})
    nonempty = samples.summary
    empty = model.evaluate_samples([]).summary
    expected_empty = pd.DataFrame(
        {
            "objective": pd.Series(dtype="float64"),
            "feasible": pd.Series(dtype="bool"),
        },
        index=pd.Index([], dtype="uint64", name="sample_id"),
    )
    pd.testing.assert_frame_equal(empty, expected_empty)
    pd.testing.assert_frame_equal(empty, nonempty.iloc[:0])
    assert nonempty.index.tolist() == [sample_id]
    assert nonempty.loc[sample_id, "objective"] == samples.get(sample_id).objective
    assert nonempty["feasible"].to_dict() == samples.feasible

    for frame in (empty, nonempty):
        assert list(frame.columns) == ["objective", "feasible"]
        assert frame["objective"].dtype == "float64"
        assert frame["feasible"].dtype == "bool"
        assert frame.index.dtype == "uint64"
        assert frame.index.name == "sample_id"
        feasible = frame[frame["feasible"]]
        infeasible = frame[~frame["feasible"]]
        assert isinstance(feasible, pd.DataFrame)
        assert isinstance(infeasible, pd.DataFrame)
        pd.testing.assert_frame_equal(feasible, frame)
        pd.testing.assert_frame_equal(infeasible, empty)
        pd.testing.assert_frame_equal(pd.concat([empty, frame]), frame)
        pd.testing.assert_frame_equal(pd.concat([frame, empty]), frame)

    nonempty.loc[sample_id, "objective"] = -123.0
    nonempty.loc[sample_id, "feasible"] = False
    assert samples.get(sample_id).objective == 1.0
    assert samples.feasible == {sample_id: True}
    assert samples.summary.loc[sample_id, "objective"] == 1.0


@pytest.mark.parametrize(
    "sense,expected_ids",
    [
        (Sense.Minimize, [30, 20, 40, 10, 90]),
        (Sense.Maximize, [20, 40, 30, 90, 10]),
    ],
)
def test_summary_sort_and_candidate_identity(sense, expected_ids):
    x = DecisionVariable.integer(0, lower=0, upper=3)
    model = Instance.from_components(
        decision_variables=[x],
        objective=x,
        active_constraints={0: x <= 1},
        sense=sense,
    )
    assignments = {90: {0: 3}, 40: {0: 1}, 30: {0: 0}, 20: {0: 1}, 10: {0: 2}}
    # Tied candidates retain their IDs, independently of insertion order.
    for states in (assignments, dict(reversed(list(assignments.items())))):
        samples = model.evaluate_samples(states)
        frame = samples.summary
        assert frame.index.tolist() == expected_ids
        assert frame["feasible"].to_dict() == samples.feasible
        assert frame["objective"].to_dict() == {
            sample_id: samples.get(sample_id).objective for sample_id in states
        }
        assert frame[frame["feasible"]].index.tolist() == expected_ids[:3]


def test_summary_all_infeasible_filter_preserves_schema():
    model = Instance.minimize()
    x = model.new_binary()
    model.objective = x
    samples = model.evaluate_samples([{x.id: 0.5}])
    frame = samples.summary
    assert frame["feasible"].tolist() == [False]
    feasible = frame[frame["feasible"]]
    assert isinstance(feasible, pd.DataFrame)
    pd.testing.assert_frame_equal(feasible, model.evaluate_samples([]).summary)
