from ommx import DecisionVariable, Instance, SampleSet, Sense
from ommx.experiment import Experiment

from ommx_openjij_adapter import OMMXOpenJijSAAdapter


def test_log_sample_records_the_exact_prepared_adapter_input() -> None:
    x = DecisionVariable.binary(0, name="x", subscripts=[0])
    source = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: x == 1},
        sense=Sense.Maximize,
    )
    source_bytes = source.to_v2_bytes()
    preparation = OMMXOpenJijSAAdapter.prepare(
        source,
        uniform_penalty_weight=2.0,
    )
    adapter_input = preparation.input
    input_bytes = adapter_input.to_v2_bytes()
    experiment = Experiment.with_temp_local_registry()
    prepared_samples: SampleSet | None = None

    with experiment.run() as run:
        prepared_samples = run.log_sample(
            OMMXOpenJijSAAdapter,
            adapter_input,
            num_reads=4,
            seed=0,
        )

    assert prepared_samples is not None
    assert source.to_v2_bytes() == source_bytes
    assert prepared_samples.sense == Sense.Minimize
    for sample_id in prepared_samples.sample_ids():
        actual = prepared_samples.get(sample_id)
        expected = adapter_input.evaluate(actual.state)
        assert actual.objective == expected.objective
        assert actual.sense == expected.sense
        assert len(actual.constraints) == len(expected.constraints)

    source_samples = preparation.evaluate_source(prepared_samples)
    assert source_samples.sense == Sense.Maximize
    assert len(source_samples.constraints) == 1
    for sample_id in source_samples.sample_ids():
        value = source_samples.extract_decision_variables("x", sample_id)[(0,)]
        expected = source.evaluate({0: value})
        actual = source_samples.get(sample_id)
        assert actual.objective == expected.objective
        assert actual.feasible == expected.feasible

    loaded = Experiment.from_artifact(experiment.commit())
    [sampling] = loaded.runs[0].samplings
    assert sampling.status == "finished"
    assert isinstance(sampling.input, Instance)
    assert sampling.input.to_v2_bytes() == input_bytes
    assert isinstance(sampling.output, SampleSet)
    assert sampling.output.to_v2_bytes() == prepared_samples.to_v2_bytes()
    assert sampling.adapter_options == {
        "num_reads": 4,
        "seed": 0,
    }
