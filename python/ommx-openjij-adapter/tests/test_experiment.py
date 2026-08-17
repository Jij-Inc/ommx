from ommx import DecisionVariable, FixedPenaltyPreparation, Instance, SampleSet, Sense
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
    input_class = OMMXOpenJijSAAdapter.INPUT_CLASS
    policy = OMMXOpenJijSAAdapter.recommended_preparation_policy()
    policy.fixed_penalty = (
        FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight(weight=2.0)
    )
    source.prepare(input_class, policy)
    adapter_input = source
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
    assert prepared_samples.sense == Sense.Minimize
    for sample_id in prepared_samples.sample_ids():
        actual = prepared_samples.get(sample_id)
        expected = adapter_input.evaluate(actual.state)
        assert actual.objective == expected.objective
        assert actual.sense == expected.sense
        assert len(actual.constraints) == len(expected.constraints)

    loaded = Experiment.from_artifact(experiment.commit())
    [sampling] = loaded.runs[0].samplings
    assert sampling.status == "finished"
    assert isinstance(sampling.input, Instance)
    assert sampling.input.to_v2_bytes() == input_bytes
    assert isinstance(sampling.output, SampleSet)
    assert sampling.output.to_v2_bytes() == prepared_samples.to_v2_bytes()
    assert sampling.adapter == "ommx_openjij_adapter.OMMXOpenJijSAAdapter"
    assert sampling.adapter_options == {
        "num_reads": 4,
        "seed": 0,
    }
