from ommx.v1 import Instance, State, Samples
import openjij as oj


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
    instance: Instance,
    *,
    beta_min: float | None = None,
    beta_max: float | None = None,
    num_sweeps: int | None = None,
    num_reads: int | None = None,
    schedule: list | None = None,
    initial_state: list | dict | None = None,
    updater: str | None = None,
    sparse: bool | None = None,
    reinitialize_state: bool | None = None,
    seed: int | None = None,
) -> Samples:
    """
    Sampling QUBO with Simulated Annealing (SA) by [`openjij.SASampler`](https://openjij.github.io/OpenJij/reference/openjij/index.html#openjij.SASampler)

    The input `instance` must be a QUBO (Quadratic Unconstrained Binary Optimization) problem, i.e.

    - Every decision variables are binary
    - No constraint
    - Objective function is quadratic
    - Minimization problem

    You can convert a problem to QUBO via [`ommx.v1.Instance.penalty_method`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.penalty_method) or other corresponding method.

    :param instance: ommx.v1.Instance representing a QUBO problem
    :param beta_min: minimal value of inverse temperature
    :param beta_max: maximum value of inverse temperature
    :param num_sweeps: number of sweeps
    :param num_reads: number of reads
    :param schedule: list of inverse temperature
    :param initial_state: initial state
    :param updater: updater algorithm
    :param sparse: use sparse matrix or not.
    :param reinitialize_state: if true reinitialize state for each run
    :param seed: seed for Monte Carlo algorithm
    """
    q, _offset = instance.as_qubo_format()
    sampler = oj.SASampler()
    response = sampler.sample_qubo(
        q,  # type: ignore
        beta_min=beta_min,
        beta_max=beta_max,
        num_sweeps=num_sweeps,
        num_reads=num_reads,
        schedule=schedule,
        initial_state=initial_state,
        updater=updater,
        sparse=sparse,
        reinitialize_state=reinitialize_state,
        seed=seed,
    )
    return response_to_samples(response)
