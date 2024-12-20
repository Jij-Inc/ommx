from ommx.v1 import Instance, State, Samples
import openjij as oj


def sample_qubo(instance: Instance, *, num_reads: int = 1) -> Samples:
    q, c = instance.as_qubo_format()
    sampler = oj.SASampler()
    response = sampler.sample_qubo(q, num_reads=num_reads)  # type: ignore

    sample_id = 0
    entries = []
    for i in range(num_reads):
        sample = response.record.sample[i]
        state = State(entries=zip(response.variables, sample))  # type: ignore
        ids = [sample_id + j for j in range(response.record.num_occurrences[i])]
        entries.append(Samples.SamplesEntry(state=state, ids=ids))
    return Samples(entries=entries)
