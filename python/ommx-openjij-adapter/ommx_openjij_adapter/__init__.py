from __future__ import annotations

from ommx.v1 import Instance, State, Samples, SampleSet
from ommx.adapter import SamplerAdapter
import openjij as oj
from typing_extensions import deprecated


class OMMXOpenJijSAAdapter(SamplerAdapter):
    """
    Sampling QUBO with Simulated Annealing (SA) by `openjij.SASampler <https://openjij.github.io/OpenJij/reference/openjij/index.html#openjij.SASampler>`_
    """

    ommx_instance: Instance
    """
    ommx.v1.Instance representing a QUBO problem

    The input `instance` must be a QUBO (Quadratic Unconstrained Binary Optimization) problem, i.e.

    - Every decision variables are binary
    - No constraint
    - Objective function is quadratic
    - Minimization problem

    You can convert an instance to QUBO via :meth:`ommx.v1.Instance.penalty_method` or other corresponding method.
    """

    beta_min: float | None = None
    """ minimal value of inverse temperature """
    beta_max: float | None = None
    """ maximum value of inverse temperature """
    num_sweeps: int | None = None
    """ number of sweeps """
    num_reads: int | None = None
    """ number of reads """
    schedule: list | None = None
    """ list of inverse temperature """
    initial_state: list | dict | None = None
    """ initial state """
    updater: str | None = None
    """ updater algorithm """
    sparse: bool | None = None
    """ use sparse matrix or not """
    reinitialize_state: bool | None = None
    """ if true reinitialize state for each run """
    seed: int | None = None
    """ seed for Monte Carlo algorithm """

    @classmethod
    def sample(
        cls,
        ommx_instance: Instance,
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
    ) -> SampleSet:
        sampler = cls(
            ommx_instance,
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
        response = sampler._sample()
        return sampler.decode_to_sampleset(response)

    def __init__(
        self,
        ommx_instance: Instance,
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
    ):
        self.ommx_instance = ommx_instance
        self.beta_min = beta_min
        self.beta_max = beta_max
        self.num_sweeps = num_sweeps
        self.num_reads = num_reads
        self.schedule = schedule
        self.initial_state = initial_state
        self.updater = updater
        self.sparse = sparse
        self.reinitialize_state = reinitialize_state
        self.seed = seed

    def decode_to_sampleset(self, data: oj.Response) -> SampleSet:
        samples = decode_to_samples(data)
        return self.ommx_instance.evaluate_samples(samples)

    def decode_to_samples(self, data: oj.Response) -> Samples:
        """
        Convert `openjij.Response <https://openjij.github.io/OpenJij/reference/openjij/index.html#openjij.Response>`_ to :class:`Samples`

        There is a static method :meth:`decode_to_samples` that does the same thing.
        """
        return decode_to_samples(data)

    @property
    def sampler_input(self) -> dict[tuple[int, int], float]:
        qubo, _offset = self.ommx_instance.as_qubo_format()
        return qubo

    def _sample(self) -> oj.Response:
        sampler = oj.SASampler()
        qubo, _offset = self.ommx_instance.as_qubo_format()
        return sampler.sample_qubo(
            qubo,  # type: ignore
            beta_min=self.beta_min,
            beta_max=self.beta_max,
            num_sweeps=self.num_sweeps,
            num_reads=self.num_reads,
            schedule=self.schedule,
            initial_state=self.initial_state,
            updater=self.updater,
            sparse=self.sparse,
            reinitialize_state=self.reinitialize_state,
            seed=self.seed,
        )


@deprecated("Renamed to `decode_to_samples`")
def response_to_samples(response: oj.Response) -> Samples:
    """
    Deprecated: renamed to :meth:`decode_to_samples`
    """
    return decode_to_samples(response)


def decode_to_samples(response: oj.Response) -> Samples:
    """
    Convert `openjij.Response <https://openjij.github.io/OpenJij/reference/openjij/index.html#openjij.Response>`_ to :class:`Samples`
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


@deprecated("Use `OMMXOpenJijSAAdapter.sample` instead")
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
    Deprecated: Use :meth:`OMMXOpenJijSAAdapter.sample` instead
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
    return decode_to_samples(response)
