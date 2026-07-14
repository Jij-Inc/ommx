from ommx import DecisionVariable, Instance, SampleSet, Sense
from ommx.experiment import Experiment

from ommx_openjij_adapter import OMMXOpenJijSAAdapter


def test_log_sample_records_source_instance_when_preparation_is_enabled() -> None:
    x = DecisionVariable.binary(0, name="x", subscripts=[0])
    source = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: x == 1},
        sense=Sense.Maximize,
    )
    source_bytes = source.to_v2_bytes()
    experiment = Experiment.with_temp_local_registry()
    sample_set: SampleSet | None = None

    with experiment.run() as run:
        sample_set = run.log_sample(
            OMMXOpenJijSAAdapter,
            source,
            preparation=True,
            uniform_penalty_weight=2.0,
            num_reads=4,
            seed=0,
        )

    assert sample_set is not None
    assert source.to_v2_bytes() == source_bytes
    assert sample_set.sense == Sense.Maximize
    assert len(sample_set.constraints) == 1
    for sample_id in sample_set.sample_ids():
        value = sample_set.extract_decision_variables("x", sample_id)[(0,)]
        expected = source.evaluate({0: value})
        actual = sample_set.get(sample_id)
        assert actual.objective == expected.objective
        assert actual.feasible == expected.feasible

    loaded = Experiment.from_artifact(experiment.commit())
    [sampling] = loaded.runs[0].samplings
    assert sampling.status == "finished"
    assert isinstance(sampling.input, Instance)
    assert sampling.input.to_v2_bytes() == source_bytes
    assert isinstance(sampling.output, SampleSet)
    assert sampling.output.to_v2_bytes() == sample_set.to_v2_bytes()
    assert sampling.adapter_options == {
        "preparation": True,
        "uniform_penalty_weight": 2.0,
        "num_reads": 4,
        "seed": 0,
    }
