from ommx.v1 import Instance, State, Samples
import openjij as oj
from typing import Optional


def response_to_samples(response: oj.Response) -> Samples:
    """
    Convert OpenJij's `Response` to `ommx.v1.Samples`
    """
    # Filling into ommx.v1.Samples
    # Since OpenJij does not issue the sample ID, we need to generate it in the responsibility of this OMMX Adapter
    sample_id = 0
    entries = []

    num_reads = len(response.record.num_occurrences)
    for i in range(num_reads):
        sample = response.record.sample[i]
        state = State(entries=zip(response.variables, sample))  # type: ignore
        # `num_occurrences` is encoded into sample ID list.
        # For example, if `num_occurrences` is 2, there are two samples with the same state, thus two sample IDs are generated.
        ids = []
        for _ in range(response.record.num_occurrences[i]):
            ids.append(sample_id)
            sample_id += 1
        entries.append(Samples.SamplesEntry(state=state, ids=ids))
    return Samples(entries=entries)


def sample_qubo_sa(
    instance: Instance, *, num_reads: int = 1, seed: Optional[int] = None
) -> Samples:
    """
    Sampling QUBO with Simulated Annealing (SA) by [`openjij.SASampler`](https://openjij.github.io/OpenJij/reference/openjij/index.html#openjij.SASampler)

    Note that input `instance` must be a QUBO (Quadratic Unconstrained Binary Optimization) problem, i.e.

    - Every decision variables are binary
    - No constraint
    - Objective function is quadratic
    - Minimization problem

    You can convert a problem to QUBO via [`ommx.v1.Instance.penalty_method`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.penalty_method) or other corresponding method.
    """
    q = instance.as_qubo_format()
    sampler = oj.SASampler()
    response = sampler.sample_qubo(q, num_reads=num_reads, seed=seed)  # type: ignore
    return response_to_samples(response)
