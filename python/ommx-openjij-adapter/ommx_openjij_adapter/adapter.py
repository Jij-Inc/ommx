"""Direct OpenJij Adapter implementation."""

from __future__ import annotations

from collections.abc import Iterable, Mapping
import copy
from math import isfinite
from typing import ClassVar

import openjij as oj
from ommx import (
    DegreeBound,
    Instance,
    InstanceClass,
    InstanceClassClause,
    InstanceClassMembershipReport,
    Kind,
    Sense,
    Samples,
    SampleSet,
    Solution,
)
from ommx.adapter import (
    AdapterApplicabilityReport,
    AdapterPreconditionViolation,
    DiagnosticsSink,
    SamplerAdapter,
)
from opentelemetry import trace

from ._decode import _decode_to_samples, decode_to_samples
from ._preparation import (
    OpenJijPreparation,
    OpenJijPreparationReport,
    _OpenJijPreparationSupport,
)

_tracer = trace.get_tracer("ommx.adapter.openjij")


class OMMXOpenJijSAAdapter(_OpenJijPreparationSupport, SamplerAdapter):
    """
    Sample an applicable Binary polynomial input with OpenJij simulated annealing.

    The direct Adapter input must use only Binary decision variables, have
    no active regular or special constraints, and be a minimization problem.
    Arbitrary polynomial objective degree is supported through OpenJij's QUBO
    and Binary-HUBO paths.

    Integer encoding, sense reversal, slack introduction, and finite constraint
    penalties are explicit preparation operations, not part of the declared
    input class. Pass :attr:`OpenJijPreparation.input` back to this Adapter
    as a separate :class:`ommx.Instance` value.
    """

    INPUT_CLASS: ClassVar[InstanceClass | None] = InstanceClass(
        [
            InstanceClassClause(
                label="openjij-binary-hubo",
                allowed_variable_kinds={Kind.Binary},
                objective_degree_bound=DegreeBound.unbounded(),
                allowed_senses={Sense.Minimize},
            )
        ]
    )

    MAX_OPENJIJ_VARIABLE_ID: ClassVar[int] = 2**63 - 1

    ommx_instance: Instance
    """
    Isolated copy of the exact Adapter input used to evaluate returned samples.
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

    _solver_instance: Instance
    _sampler_input_prepared: bool
    _is_hubo: bool
    _hubo: dict[tuple[int, ...], float]
    _qubo: dict[tuple[int, ...], float]

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
        if not isinstance(ommx_instance, Instance):
            raise TypeError("ommx_instance must be an Instance")
        self.require_applicable(ommx_instance)
        self._solver_instance = copy.deepcopy(ommx_instance)
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
        self._sampler_input_prepared = False
        self._is_hubo = False
        self._hubo = {}
        self._qubo = {}

    @classmethod
    def _check_preconditions(
        cls,
        ommx_instance: Instance,
        input_membership: InstanceClassMembershipReport,
    ) -> Iterable[AdapterPreconditionViolation]:
        _ = input_membership
        out_of_range_ids = frozenset(
            variable.id
            for variable in ommx_instance.used_decision_variables
            if variable.id > cls.MAX_OPENJIJ_VARIABLE_ID
        )
        if out_of_range_ids:
            return (
                AdapterPreconditionViolation(
                    condition="openjij.variable_id.signed_64_bit",
                    description=(
                        "OpenJij/cimod variable labels must fit a signed 64-bit "
                        f"integer: {sorted(out_of_range_ids)}."
                    ),
                    variable_ids=out_of_range_ids,
                    actual=max(out_of_range_ids),
                    limit=cls.MAX_OPENJIJ_VARIABLE_ID,
                ),
            )
        try:
            hubo, _ = ommx_instance.as_hubo_format()
            if any(len(key) > 2 for key in hubo):
                interactions = hubo
            else:
                interactions, _ = ommx_instance.as_qubo_format()
        except Exception as error:
            return (
                AdapterPreconditionViolation(
                    condition="openjij.interactions.format",
                    description=f"OpenJij interaction conversion failed: {error}",
                    actual=str(error),
                    limit="valid Binary QUBO or HUBO interactions",
                ),
            )

        nonfinite = {
            key: coefficient
            for key, coefficient in interactions.items()
            if not isfinite(coefficient)
        }
        if not nonfinite:
            return ()
        return (
            AdapterPreconditionViolation(
                condition="openjij.interactions.coefficient_finite",
                description=(
                    "OpenJij does not reliably reject non-finite interaction "
                    f"coefficients: {nonfinite}."
                ),
                variable_ids=frozenset(
                    variable_id for key in nonfinite for variable_id in key
                ),
                actual=len(nonfinite),
                limit="all interaction coefficients finite",
            ),
        )

    @classmethod
    def _check_prepared_input_applicability(
        cls, ommx_instance: Instance
    ) -> AdapterApplicabilityReport:
        return cls.check_applicability(ommx_instance)

    @classmethod
    def check_preparation(
        cls,
        ommx_instance: Instance,
        *,
        uniform_penalty_weight: float | None = None,
        penalty_weights: Mapping[int, float] | None = None,
        inequality_integer_slack_max_range: int = 32,
        allow_approximate_integer_slack: bool = False,
    ) -> OpenJijPreparationReport:
        """Dry-run the complete explicit preparation without mutating the input.

        This is intentionally separate from :meth:`check_applicability`, which
        checks only the Binary, unconstrained minimization Adapter input. The
        53-bit log-encoding limit is a preparation precondition, not an OpenJij
        input-class condition and not an ``ommx.v2.Feature``. A model proven
        infeasible while planning integer slack raises
        :class:`~ommx.adapter.InfeasibleDetected`. Approximate integer slack is
        disabled unless ``allow_approximate_integer_slack=True`` is supplied.
        """
        return super().check_preparation(
            ommx_instance,
            uniform_penalty_weight=uniform_penalty_weight,
            penalty_weights=penalty_weights,
            inequality_integer_slack_max_range=inequality_integer_slack_max_range,
            allow_approximate_integer_slack=allow_approximate_integer_slack,
        )

    @classmethod
    def prepare(
        cls,
        ommx_instance: Instance,
        *,
        uniform_penalty_weight: float | None = None,
        penalty_weights: Mapping[int, float] | None = None,
        inequality_integer_slack_max_range: int = 32,
        allow_approximate_integer_slack: bool = False,
    ) -> OpenJijPreparation:
        """Produce a separate Adapter input and an auditable preparation report.

        Raises :class:`~ommx.adapter.InfeasibleDetected` when variable bounds
        prove an inequality infeasible. Other preparation failures raise
        :class:`OpenJijPreparationError`. Approximate integer slack is used only
        when ``allow_approximate_integer_slack=True`` is supplied.
        """
        return super().prepare(
            ommx_instance,
            uniform_penalty_weight=uniform_penalty_weight,
            penalty_weights=penalty_weights,
            inequality_integer_slack_max_range=inequality_integer_slack_max_range,
            allow_approximate_integer_slack=allow_approximate_integer_slack,
        )

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
        diagnostics: DiagnosticsSink | None = None,
    ) -> SampleSet:
        """Sample the exact applicable ``ommx_instance`` passed to the Adapter."""
        _ = diagnostics
        with _tracer.start_as_current_span("sample") as span:
            span.set_attribute("adapter", f"{cls.__module__}.{cls.__qualname__}")
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
        diagnostics: DiagnosticsSink | None = None,
    ) -> Solution:
        """Return the best feasible sample from :meth:`sample`."""
        _ = diagnostics
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
            diagnostics=diagnostics,
        )
        return sample_set.best_feasible

    def decode_to_sampleset(self, data: oj.Response) -> SampleSet:
        with _tracer.start_as_current_span("decode"):
            variable_ids = {
                variable.id for variable in self.ommx_instance.used_decision_variables
            }
            samples = _decode_to_samples(
                data,
                variable_ids=variable_ids,
                default_values={id: 0.0 for id in variable_ids},
            )
            return self.ommx_instance.evaluate_samples(samples)

    def decode_to_samples(self, data: oj.Response) -> Samples:
        """
        Convert `openjij.Response <https://openjij.github.io/OpenJij/reference/openjij/index.html#openjij.Response>`_ to :class:`Samples`

        There is a static method :meth:`decode_to_samples` that does the same thing.
        """
        return decode_to_samples(data)

    @property
    def sampler_input(self) -> dict[tuple[int, ...], float]:
        self._prepare_sampler_input()
        if self._is_hubo:
            return self._hubo
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
        with _tracer.start_as_current_span("call"):
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

    def _prepare_sampler_input(self) -> None:
        if self._sampler_input_prepared:
            return

        with _tracer.start_as_current_span("convert"):
            hubo, _ = self._solver_instance.as_hubo_format()
            if any(len(k) > 2 for k in hubo):
                self._is_hubo = True
                self._hubo = hubo
            else:
                self._is_hubo = False
                qubo, _ = self._solver_instance.as_qubo_format()
                self._qubo = qubo

            self._sampler_input_prepared = True
