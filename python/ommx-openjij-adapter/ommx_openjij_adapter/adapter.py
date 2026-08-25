"""Direct OpenJij Adapter implementation."""

from __future__ import annotations

import copy
from math import isfinite
from typing import Any, ClassVar

import openjij as oj
from ommx import (
    DegreeBound,
    IntegerEncodingPreparation,
    IntegerSlackPreparation,
    Instance,
    InstanceClass,
    InstanceClassClause,
    Kind,
    ObjectivePreparation,
    PreparationPolicy,
    Sense,
    Samples,
    SampleSet,
    Solution,
    SpecialConstraintKind,
    SpecialConstraintPreparation,
)
from ommx.adapter import DiagnosticsSink, SamplerAdapter
from opentelemetry import trace

from ._decode import _decode_for_instance, decode_to_samples

_tracer = trace.get_tracer("ommx.adapter.openjij")


class OMMXOpenJijSAAdapter(SamplerAdapter):
    """
    Sample an applicable Binary polynomial input with OpenJij simulated annealing.

    The direct Adapter input must use only Binary decision variables, have
    no active regular or special constraints, and be a minimization problem.
    Arbitrary polynomial objective degree is supported through OpenJij's QUBO
    and Binary-HUBO paths.

    :meth:`sample` and :meth:`solve` prepare an isolated copy with
    :meth:`recommended_preparation_policy`. Use :meth:`sample_without_preparation` or
    :meth:`solve_without_preparation` after explicitly preparing an instance when
    caller-owned choices such as fixed penalty magnitudes are required.
    """

    INPUT_CLASS: ClassVar[InstanceClass] = InstanceClass(
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

    @classmethod
    def recommended_preparation_policy(cls) -> PreparationPolicy:
        """Recommend the model changes commonly needed by OpenJij.

        The recommendation lowers every special-constraint family, converts the
        active objective to minimization, adds Integer slack while permitting an
        inequality to remain when exact equality conversion is unavailable,
        and log-encodes every used Integer variable. Both Integer slack ranges
        use 32.

        Fixed penalty magnitudes remain explicit caller parameters because
        sufficient values depend on the application. The shared ``Instance``
        owner operation validates their nonnegative-with-tolerance domain. Set
        ``fixed_penalty`` on the fresh returned policy when active constraints
        must be removed.
        """
        return PreparationPolicy(
            special_constraints=SpecialConstraintPreparation.lower_special_constraints(
                kinds={
                    SpecialConstraintKind.Indicator,
                    SpecialConstraintKind.OneHot,
                    SpecialConstraintKind.Sos1,
                }
            ),
            objective=ObjectivePreparation(target=Sense.Minimize),
            integer_slack=IntegerSlackPreparation(
                max_integer_range=32,
                slack_upper_bound=32,
            ),
            integer_encoding=IntegerEncodingPreparation.log_encode_all_used_integers(),
        )

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
        **kwargs: Any,
    ) -> SampleSet:
        """Prepare and sample an isolated copy of ``ommx_instance``."""
        return super().sample(
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
            **kwargs,
        )

    @classmethod
    def sample_without_preparation(
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
        **kwargs: Any,
    ) -> SampleSet:
        """Sample an exact OpenJij Adapter input without preparing it."""
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
                **kwargs,
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
        **kwargs: Any,
    ) -> Solution:
        """Prepare, sample, and return the best feasible result."""
        return super().solve(
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
            **kwargs,
        )

    @classmethod
    def solve_without_preparation(
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
        **kwargs: Any,
    ) -> Solution:
        """Return the best feasible result from :meth:`sample_without_preparation`."""
        return cls.sample_without_preparation(
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
            **kwargs,
        ).best_feasible

    def decode_to_sampleset(self, data: oj.Response) -> SampleSet:
        with _tracer.start_as_current_span("decode"):
            return _decode_for_instance(data, self.ommx_instance)

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
            out_of_range_ids = sorted(
                variable.id
                for variable in self._solver_instance.used_decision_variables
                if variable.id > self.MAX_OPENJIJ_VARIABLE_ID
            )
            if out_of_range_ids:
                raise ValueError(
                    "OpenJij/cimod variable labels must fit a signed 64-bit "
                    f"integer: {out_of_range_ids}."
                )

            try:
                hubo, _ = self._solver_instance.as_hubo_format()
                is_hubo = any(len(key) > 2 for key in hubo)
                if is_hubo:
                    interactions = hubo
                    qubo = {}
                else:
                    qubo, _ = self._solver_instance.as_qubo_format()
                    interactions = qubo
            except Exception as error:
                raise ValueError(
                    f"OpenJij interaction conversion failed: {error}"
                ) from error

            nonfinite = {
                key: coefficient
                for key, coefficient in interactions.items()
                if not isfinite(coefficient)
            }
            if nonfinite:
                raise ValueError(
                    "OpenJij does not reliably reject non-finite interaction "
                    f"coefficients: {nonfinite}."
                )

            self._is_hubo = is_hubo
            self._hubo = hubo if is_hubo else {}
            self._qubo = qubo
            self._sampler_input_prepared = True
