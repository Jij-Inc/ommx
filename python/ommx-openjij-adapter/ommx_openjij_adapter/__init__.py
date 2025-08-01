from __future__ import annotations

from ommx.v1 import (
    Instance,
    State,
    Samples,
    SampleSet,
    Solution,
    DecisionVariable,
    Constraint,
)
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

    _instance_prepared: bool = False

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

    @classmethod
    def solve(
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
    ) -> Solution:
        sample_set = cls.sample(
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
        return sample_set.best_feasible

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
        self._prepare_convert()
        if self._is_hubo:
            return self._hubo
        else:
            return self._qubo

    @property
    def solver_input(self) -> dict[tuple[int, ...], float]:
        return self.sampler_input

    def decode(self, data: oj.Response) -> Solution:
        sample_set = self.decode_to_sampleset(data)
        return sample_set.best_feasible

    def _sample(self) -> oj.Response:
        sampler = oj.SASampler()
        input = self.sampler_input
        if self._is_hubo:
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

    # Manually perform the conversion process to QUBO/HUBO, instead of using
    # `to_hubo` or `to_qubo`.
    #
    # This is so that we can manually call `as_hubo_format()`, check if the
    # finalized instance is higher-order, and if not, call
    # `as_qubo_format()`.
    #
    # We could do alternative methods like simply checking the degrees of
    # the objective function and all constraints. But some instances will
    # see to be higher-order but ultimately representable as QUBO after the
    # conversion (eg. due to simplifying binary `x * x`). So we chose to do
    # it this way.
    def _prepare_convert(self):
        if self._instance_prepared:
            return

        is_converted = self.ommx_instance.as_minimization_problem()

        continuous_variables = [
            var.id
            for var in self.ommx_instance.used_decision_variables
            if var.kind == DecisionVariable.CONTINUOUS
        ]
        if len(continuous_variables) > 0:
            raise ValueError(
                f"Continuous variables are not supported in HUBO conversion: IDs={continuous_variables}"
            )

        # Prepare inequality constraints
        ineq_ids = [
            c.id
            for c in self.ommx_instance.constraints
            if c.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO
        ]
        for ineq_id in ineq_ids:
            try:
                self.ommx_instance.convert_inequality_to_equality_with_integer_slack(
                    ineq_id, self.inequality_integer_slack_max_range
                )
            except RuntimeError:
                self.ommx_instance.add_integer_slack_to_inequality(
                    ineq_id, self.inequality_integer_slack_max_range
                )

        # Penalty method
        if self.ommx_instance.constraints:
            if self.uniform_penalty_weight is not None and self.penalty_weights:
                raise ValueError(
                    "Both uniform_penalty_weight and penalty_weights are specified. Please choose one."
                )
            if self.penalty_weights:
                pi = self.ommx_instance.penalty_method()
                weights = {
                    p.id: self.penalty_weights[p.subscripts[0]] for p in pi.parameters
                }
                unconstrained = pi.with_parameters(weights)
            else:
                if self.uniform_penalty_weight is None:
                    # If both are None, defaults to uniform_penalty_weight = 1.0
                    self.uniform_penalty_weight = 1.0
                pi = self.ommx_instance.uniform_penalty_method()
                weight = pi.parameters[0]
                unconstrained = pi.with_parameters(
                    {weight.id: self.uniform_penalty_weight}
                )
            self.ommx_instance.raw = unconstrained.raw

        self.ommx_instance.log_encode()

        hubo, _ = self.ommx_instance.as_hubo_format()
        if any(len(k) > 2 for k in hubo.keys()):
            self._is_hubo = True
            self._hubo = hubo
        else:
            self._is_hubo = False
            qubo, _ = self.ommx_instance.as_qubo_format()
            self._qubo = qubo

        self._instance_prepared = True

        if is_converted:
            self.ommx_instance.as_maximization_problem()


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
    # Create empty samples and append each state with its sample IDs
    # Since OpenJij does not issue the sample ID, we need to generate it in the responsibility of this OMMX Adapter
    samples = Samples({})  # Create empty samples
    sample_id = 0

    num_reads = len(response.record.num_occurrences)
    for i in range(num_reads):
        sample = response.record.sample[i]
        state = State(entries=zip(response.variables, sample))
        # `num_occurrences` is encoded into sample ID list.
        # For example, if `num_occurrences` is 2, there are two samples with the same state, thus two sample IDs are generated.
        ids = []
        for _ in range(response.record.num_occurrences[i]):
            ids.append(sample_id)
            sample_id += 1
        samples.append(ids, state)

    return samples


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
