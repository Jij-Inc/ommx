from __future__ import annotations

from ommx.v1 import Instance, State, Samples, SampleSet
from ommx.adapter import SamplerAdapter
import openjij as oj
from typing_extensions import deprecated
from typing import Optional
import copy


class OMMXOpenJijSAAdapter(SamplerAdapter):
    """
    Sampling QUBO or HUBO with Simulated Annealing (SA) by `openjij.SASampler <https://openjij.github.io/OpenJij/reference/openjij/index.html#openjij.SASampler>`_
    """

    ommx_instance: Instance
    """
    ommx.v1.Instance representing a QUBO or HUBO problem

    The input `instance` must be a QUBO (Quadratic Unconstrained Binary Optimization) or HUBO (Higher-order Unconstrained Binary Optimization) problem, i.e.

    - All decision variables are binary
    - No constraints
    - Objective function is quadratic (QUBO) or higher (HUBO).
    - Minimization problem

    You can convert an instance to QUBO or HUBO via :meth:`ommx.v1.Instance.penalty_method` or other corresponding method.
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
    """ list of inverse temperature (parameter only used if problem is QUBO)"""
    initial_state: list | dict | None = None
    """ initial state (parameter only used if problem is QUBO)"""
    updater: str | None = None
    """ updater algorithm """
    sparse: bool | None = None
    """ use sparse matrix or not (parameter only used if problem is QUBO)"""
    reinitialize_state: bool | None = None
    """ if true reinitialize state for each run (parameter only used if problem is QUBO)"""
    seed: int | None = None
    """ seed for Monte Carlo algorithm """

    uniform_penalty_weight: Optional[float] = None
    """ Weight for uniform penalty, passed to ``Instance.to_qubo`` or ``Instance.to_hubo`` """
    penalty_weights: dict[int, float] = {}
    """ Penalty weights for each constraint, passed to ``Instance.to_qubo`` or ``Instance.to_hubo`` """
    inequality_integer_slack_max_range: int = 32
    """ Max range for integer slack variables in inequality constraints, passed to ``Instance.to_qubo`` or ``Instance.to_hubo`` """

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
        uniform_penalty_weight: Optional[float] = None,
        penalty_weights: dict[int, float] = {},
        inequality_integer_slack_max_range: int = 32,
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
            uniform_penalty_weight=uniform_penalty_weight,
            penalty_weights=penalty_weights,
            inequality_integer_slack_max_range=inequality_integer_slack_max_range,
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
        uniform_penalty_weight: Optional[float] = None,
        penalty_weights: dict[int, float] = {},
        inequality_integer_slack_max_range: int = 32,
    ):
        self.ommx_instance = copy.deepcopy(ommx_instance)
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
        self.uniform_penalty_weight = uniform_penalty_weight
        self.penalty_weights = penalty_weights
        self.inequality_integer_slack_max_range = inequality_integer_slack_max_range

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
    def sampler_input(self) -> dict[tuple[int, ...], float]:
        if self.ommx_instance.objective.degree() > 2:
            hubo, _offset = self.ommx_instance.to_hubo(
                uniform_penalty_weight=self.uniform_penalty_weight,
                penalty_weights=self.penalty_weights,
                inequality_integer_slack_max_range=self.inequality_integer_slack_max_range,
            )
            return hubo
        else:
            qubo, _offset = self.ommx_instance.to_qubo(
                uniform_penalty_weight=self.uniform_penalty_weight,
                penalty_weights=self.penalty_weights,
                inequality_integer_slack_max_range=self.inequality_integer_slack_max_range,
            )
            return qubo

    def _sample(self) -> oj.Response:
        sampler = oj.SASampler()
        degree = self.ommx_instance.objective.degree()
        input = self.sampler_input
        if degree > 2:
            return sampler.sample_hubo(
                input,  # type: ignore
                vartype="BINARY",
                beta_min=self.beta_min,
                beta_max=self.beta_max,
                # maintaining default parameters in openjij impl if None passed
                num_sweeps=self.num_sweeps or 1000,
                num_reads=self.num_reads or 1,
                updater=self.updater or "METROPOLIS",
                seed=self.seed,
            )

        else:
            return sampler.sample_qubo(
                input,  # type: ignore
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
    sampler = OMMXOpenJijSAAdapter(
        instance,
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
    return decode_to_samples(response)
