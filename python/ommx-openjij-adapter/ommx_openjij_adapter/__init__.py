from __future__ import annotations

from ommx import (
    AdditionalCapability,
    AdapterCapabilities,
    CapabilityProfile,
    DegreeLimit,
    Equality,
    Instance,
    Kind,
    Sense,
    State,
    Samples,
    SampleSet,
    Solution,
)
from ommx.adapter import (
    AdapterCompatibilityError,
    AdapterCompatibilityReport,
    AdapterPreconditionViolation,
    ConstraintRef,
    DiagnosticsSink,
    InfeasibleDetected,
    SamplerAdapter,
)
import openjij as oj
from opentelemetry import trace
from typing_extensions import deprecated
from collections.abc import Callable, Iterable, Mapping
from dataclasses import dataclass, field
from enum import Enum
from math import isfinite
from typing import ClassVar
import copy

_tracer = trace.get_tracer("ommx.adapter.openjij")


class OpenJijPreparationSemantics(str, Enum):
    """Semantic effect of one explicit OpenJij preparation step."""

    Exact = "exact"
    Approximate = "approximate"
    FinitePenalty = "finite_penalty"


@dataclass(frozen=True, slots=True)
class OpenJijPreparationStep:
    """One auditable transformation applied before native OpenJij translation."""

    operation: str
    semantics: OpenJijPreparationSemantics
    description: str
    variable_ids: frozenset[int] = field(default_factory=frozenset)
    constraint_refs: frozenset[ConstraintRef] = field(default_factory=frozenset)


@dataclass(frozen=True, slots=True)
class OpenJijPreparationReport:
    """Compatibility checks and semantic steps for an explicit preparation."""

    source_compatibility: AdapterCompatibilityReport
    encoding_compatibility: AdapterCompatibilityReport
    steps: tuple[OpenJijPreparationStep, ...]
    final_compatibility: AdapterCompatibilityReport


@dataclass(frozen=True, slots=True)
class OpenJijPreparedModel:
    """Prepared native solver input plus the model used to evaluate samples."""

    _solver_instance: Instance = field(repr=False)
    _decoder_instance: Instance = field(repr=False)
    _evaluation_instance: Instance = field(repr=False)
    report: OpenJijPreparationReport

    @property
    def solver_instance(self) -> Instance:
        """Return an isolated copy of the Binary, unconstrained minimization input."""
        return copy.deepcopy(self._solver_instance)

    @property
    def evaluation_instance(self) -> Instance:
        """Return an isolated copy retaining the source optimization sense."""
        return copy.deepcopy(self._evaluation_instance)


class OMMXOpenJijSAAdapter(SamplerAdapter):
    """
    Sample a native Binary polynomial model with OpenJij simulated annealing.

    The direct translator input must use only Binary decision variables, have
    no active regular or special constraints, and be a minimization problem.
    Arbitrary polynomial objective degree is supported through OpenJij's QUBO
    and Binary-HUBO paths.

    Integer encoding, sense reversal, slack introduction, and finite constraint
    penalties are not native capabilities. Use :meth:`prepare` explicitly for
    inspection or direct adapter construction. The common :meth:`sample` and
    :meth:`solve` entry points keep an :class:`ommx.Instance` input and require
    ``preparation=True`` before applying those transformations.
    """

    CAPABILITIES: ClassVar[AdapterCapabilities | None] = AdapterCapabilities(
        [
            CapabilityProfile(
                name="openjij-binary-hubo",
                variable_kinds={Kind.Binary},
                objective_degree=DegreeLimit.any(),
                senses={Sense.Minimize},
            )
        ]
    )

    # This describes inputs accepted by the explicit preparation operation,
    # not inputs accepted directly by the OpenJij translator.
    _PREPARATION_INPUT_CAPABILITIES: ClassVar[AdapterCapabilities] = (
        AdapterCapabilities(
            [
                CapabilityProfile(
                    name="openjij-explicit-preparation-input",
                    variable_kinds={Kind.Binary, Kind.Integer},
                    objective_degree=DegreeLimit.any(),
                    regular_constraints={
                        Equality.EqualToZero: DegreeLimit.any(),
                        Equality.LessThanOrEqualToZero: DegreeLimit.any(),
                    },
                    indicator_constraints={
                        Equality.EqualToZero: DegreeLimit.any(),
                        Equality.LessThanOrEqualToZero: DegreeLimit.any(),
                    },
                    supports_one_hot=True,
                    supports_sos1=True,
                    senses={Sense.Minimize, Sense.Maximize},
                )
            ]
        )
    )
    _ENCODING_INPUT_CAPABILITIES: ClassVar[AdapterCapabilities] = AdapterCapabilities(
        [
            CapabilityProfile(
                name="openjij-log-encoding-input",
                variable_kinds={Kind.Binary, Kind.Integer},
                objective_degree=DegreeLimit.any(),
                senses={Sense.Minimize},
            )
        ]
    )
    MAX_LOG_ENCODING_BITS: ClassVar[int] = 53
    MAX_OPENJIJ_VARIABLE_ID: ClassVar[int] = 2**63 - 1
    MAX_SLACK_RANGE: ClassVar[int] = 2**64 - 1

    ommx_instance: Instance
    """
    Isolated instance used to evaluate and decode returned samples.

    For a prepared source model this retains the source optimization sense,
    while the separate private solver instance satisfies ``CAPABILITIES``.
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

    preparation_report: OpenJijPreparationReport | None
    """Audit report for explicit preparation, or ``None`` for native input."""

    _sampler_input_prepared: bool = False

    def __init__(
        self,
        ommx_instance: Instance | OpenJijPreparedModel,
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
        if isinstance(ommx_instance, OpenJijPreparedModel):
            self.require_compatible(ommx_instance._solver_instance)
            self._solver_instance = copy.deepcopy(ommx_instance._solver_instance)
            self._decoder_instance = copy.deepcopy(ommx_instance._decoder_instance)
            self.ommx_instance = copy.deepcopy(ommx_instance._evaluation_instance)
            self.preparation_report = ommx_instance.report
        elif isinstance(ommx_instance, Instance):
            self.require_compatible(ommx_instance)
            self._solver_instance = copy.deepcopy(ommx_instance)
            self._decoder_instance = copy.deepcopy(ommx_instance)
            self.ommx_instance = copy.deepcopy(ommx_instance)
            self.preparation_report = None
        else:
            raise TypeError("ommx_instance must be an Instance or OpenJijPreparedModel")
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

    @staticmethod
    def _active_constraint_refs(ommx_instance: Instance) -> frozenset[ConstraintRef]:
        return frozenset(
            [
                *(ConstraintRef("regular", id) for id in ommx_instance.constraints),
                *(
                    ConstraintRef("indicator", id)
                    for id in ommx_instance.indicator_constraints
                ),
                *(
                    ConstraintRef("one_hot", id)
                    for id in ommx_instance.one_hot_constraints
                ),
                *(ConstraintRef("sos1", id) for id in ommx_instance.sos1_constraints),
            ]
        )

    @classmethod
    def _compatibility_report_against(
        cls,
        ommx_instance: Instance,
        capabilities: AdapterCapabilities,
        context: str,
        check_preconditions: Callable[[], Iterable[AdapterPreconditionViolation]],
    ) -> AdapterCompatibilityReport:
        portable_report = capabilities.check_compatibility(
            ommx_instance.solver_requirements()
        )
        adapter = f"{cls.__module__}.{cls.__qualname__}.{context}"
        if not portable_report.compatible:
            return AdapterCompatibilityReport(
                adapter=adapter,
                portable_report=portable_report,
                preconditions_checked=False,
                precondition_violations=(),
            )
        return AdapterCompatibilityReport(
            adapter=adapter,
            portable_report=portable_report,
            preconditions_checked=True,
            precondition_violations=tuple(check_preconditions()),
        )

    @classmethod
    def _log_encoding_precondition_violations(
        cls, ommx_instance: Instance
    ) -> tuple[AdapterPreconditionViolation, ...]:
        encoding_input = copy.deepcopy(ommx_instance)
        try:
            encoding_input.reduce_capabilities(set())
        except Exception:
            # Full preparation materialization reports special-constraint
            # lowering failures with their source constraint references.
            return ()

        integer_variables = {
            variable.id: variable
            for variable in encoding_input.used_decision_variables
            if variable.kind == Kind.Integer
        }
        if not integer_variables:
            return ()

        violations: list[AdapterPreconditionViolation] = []
        max_exact_integer = float(2**cls.MAX_LOG_ENCODING_BITS)
        max_range_width = float(2**cls.MAX_LOG_ENCODING_BITS - 1)
        for variable_id, variable in integer_variables.items():
            candidate = copy.deepcopy(encoding_input)
            try:
                candidate.log_encode({variable_id})
            except Exception as error:
                lower = variable.bound.lower
                upper = variable.bound.upper
                width = upper - lower
                description = (
                    f"Integer variable {variable_id} cannot be log-encoded for "
                    f"OpenJij preparation: {error}"
                )
                if not isfinite(lower) or not isfinite(upper):
                    condition = "openjij.log_encoding.bound_finite"
                    actual: str | int | float = f"[{lower}, {upper}]"
                    limit: str | int | float = "finite integer range"
                elif width > max_range_width:
                    condition = "openjij.log_encoding.max_bits"
                    actual = int(width).bit_length()
                    limit = cls.MAX_LOG_ENCODING_BITS
                elif width != 0 and (
                    lower < -max_exact_integer or upper > max_exact_integer
                ):
                    condition = "openjij.log_encoding.exact_integer_range"
                    actual = lower if lower < -max_exact_integer else upper
                    limit = max_exact_integer
                else:
                    condition = "openjij.log_encoding.failed"
                    actual = f"[{lower}, {upper}]"
                    limit = (
                        "finite unit-spaced range encodable with at most "
                        f"{cls.MAX_LOG_ENCODING_BITS} bits"
                    )
                violations.append(
                    AdapterPreconditionViolation(
                        condition=condition,
                        description=description,
                        variable_ids=frozenset({variable_id}),
                        actual=actual,
                        limit=limit,
                    )
                )

        if violations:
            return tuple(violations)

        candidate = copy.deepcopy(encoding_input)
        try:
            candidate.log_encode(set(integer_variables))
        except Exception as error:
            return (
                AdapterPreconditionViolation(
                    condition="openjij.log_encoding.combined_rewrite",
                    description=(
                        "The complete Integer-to-Binary rewrite cannot be applied "
                        f"for OpenJij preparation: {error}"
                    ),
                    variable_ids=frozenset(integer_variables),
                    actual=str(error),
                    limit="a finite atomic log-encoding rewrite",
                ),
            )
        return ()

    @classmethod
    def _penalty_precondition_violations(
        cls,
        ommx_instance: Instance,
        uniform_penalty_weight: float | None,
        penalty_weights: Mapping[int, float] | None,
        inequality_integer_slack_max_range: int,
    ) -> tuple[AdapterPreconditionViolation, ...]:
        constraint_ids = frozenset(ommx_instance.constraints)
        constraint_refs = cls._active_constraint_refs(ommx_instance)
        special_constraint_refs = frozenset(
            ref for ref in constraint_refs if ref.family != "regular"
        )
        violations: list[AdapterPreconditionViolation] = []

        if uniform_penalty_weight is not None and penalty_weights is not None:
            violations.append(
                AdapterPreconditionViolation(
                    condition="openjij.penalty.options_exclusive",
                    description=(
                        "Choose either a uniform penalty weight or per-constraint "
                        "penalty weights, not both."
                    ),
                    constraint_refs=constraint_refs,
                    actual="both selected",
                    limit="one penalty mode",
                )
            )

        if (
            constraint_refs
            and uniform_penalty_weight is None
            and penalty_weights is None
        ):
            violations.append(
                AdapterPreconditionViolation(
                    condition="openjij.penalty.explicit_selection",
                    description=(
                        "Active regular constraints require an explicit finite-penalty "
                        "preparation; they are not a native OpenJij capability."
                    ),
                    constraint_refs=constraint_refs,
                    actual="not selected",
                    limit="uniform_penalty_weight or penalty_weights",
                )
            )

        if not constraint_refs and (
            uniform_penalty_weight is not None or penalty_weights is not None
        ):
            violations.append(
                AdapterPreconditionViolation(
                    condition="openjij.penalty.unused",
                    description="Penalty weights were supplied for an unconstrained model.",
                    actual="penalty weights supplied",
                    limit="no penalty configuration",
                )
            )

        if uniform_penalty_weight is not None and (
            not isfinite(uniform_penalty_weight) or uniform_penalty_weight <= 0
        ):
            violations.append(
                AdapterPreconditionViolation(
                    condition="openjij.penalty.weight_positive_finite",
                    description="The uniform penalty weight must be finite and positive.",
                    constraint_refs=constraint_refs,
                    actual=uniform_penalty_weight,
                    limit="finite value > 0",
                )
            )

        if penalty_weights is not None:
            if special_constraint_refs:
                violations.append(
                    AdapterPreconditionViolation(
                        condition="openjij.penalty.special_requires_uniform",
                        description=(
                            "Per-constraint weights are keyed by regular constraint ID "
                            "and cannot identify constraints introduced by exact special-"
                            "constraint lowering. Use a uniform penalty weight."
                        ),
                        constraint_refs=special_constraint_refs,
                        actual="per-constraint penalty weights",
                        limit="uniform_penalty_weight",
                    )
                )
            missing = constraint_ids.difference(penalty_weights)
            if missing:
                violations.append(
                    AdapterPreconditionViolation(
                        condition="openjij.penalty.weight_coverage",
                        description=(
                            "Per-constraint penalty weights do not cover every "
                            f"active regular constraint: missing {sorted(missing)}."
                        ),
                        constraint_refs=frozenset(
                            ConstraintRef("regular", constraint_id)
                            for constraint_id in missing
                        ),
                        actual=len(penalty_weights),
                        limit=len(constraint_ids),
                    )
                )
            unexpected = frozenset(penalty_weights).difference(constraint_ids)
            if unexpected:
                violations.append(
                    AdapterPreconditionViolation(
                        condition="openjij.penalty.weight_coverage",
                        description=(
                            "Per-constraint penalty weights contain unknown regular "
                            f"constraint IDs: {sorted(unexpected)}."
                        ),
                        constraint_refs=frozenset(
                            ConstraintRef("regular", constraint_id)
                            for constraint_id in unexpected
                        ),
                        actual=len(penalty_weights),
                        limit=len(constraint_ids),
                    )
                )
            for constraint_id, weight in penalty_weights.items():
                if not isfinite(weight) or weight <= 0:
                    violations.append(
                        AdapterPreconditionViolation(
                            condition="openjij.penalty.weight_positive_finite",
                            description=(
                                f"Penalty weight for regular constraint {constraint_id} "
                                "must be finite and positive."
                            ),
                            constraint_refs=frozenset(
                                {ConstraintRef("regular", constraint_id)}
                            ),
                            actual=weight,
                            limit="finite value > 0",
                        )
                    )

        inequality_refs = frozenset(
            ConstraintRef("regular", constraint_id)
            for constraint_id, constraint in ommx_instance.constraints.items()
            if constraint.equality == Equality.LessThanOrEqualToZero
        )
        slack_relevant_refs = inequality_refs.union(
            ref
            for ref in special_constraint_refs
            if ref.family in {"indicator", "sos1"}
        )
        valid_slack_range = (
            isinstance(inequality_integer_slack_max_range, int)
            and not isinstance(inequality_integer_slack_max_range, bool)
            and 0 < inequality_integer_slack_max_range <= cls.MAX_SLACK_RANGE
        )
        if slack_relevant_refs and not valid_slack_range:
            violations.append(
                AdapterPreconditionViolation(
                    condition="openjij.slack.range_unsigned_64_bit",
                    description=(
                        "The integer slack range must fit the positive unsigned "
                        "64-bit range when preparing inequality constraints."
                    ),
                    constraint_refs=slack_relevant_refs,
                    actual=inequality_integer_slack_max_range,
                    limit=f"integer in [1, {cls.MAX_SLACK_RANGE}]",
                )
            )
        return tuple(violations)

    @classmethod
    def _check_preparation_input(
        cls,
        ommx_instance: Instance,
        *,
        uniform_penalty_weight: float | None = None,
        penalty_weights: Mapping[int, float] | None = None,
        inequality_integer_slack_max_range: int = 32,
    ) -> AdapterCompatibilityReport:
        return cls._compatibility_report_against(
            ommx_instance,
            cls._PREPARATION_INPUT_CAPABILITIES,
            "prepare",
            lambda: (
                *cls._log_encoding_precondition_violations(ommx_instance),
                *cls._penalty_precondition_violations(
                    ommx_instance,
                    uniform_penalty_weight,
                    penalty_weights,
                    inequality_integer_slack_max_range,
                ),
            ),
        )

    @classmethod
    def _plan_preparation(
        cls,
        ommx_instance: Instance,
        *,
        uniform_penalty_weight: float | None,
        penalty_weights: Mapping[int, float] | None,
        inequality_integer_slack_max_range: int,
    ) -> tuple[AdapterCompatibilityReport, OpenJijPreparedModel | None]:
        source_report = cls._check_preparation_input(
            ommx_instance,
            uniform_penalty_weight=uniform_penalty_weight,
            penalty_weights=penalty_weights,
            inequality_integer_slack_max_range=inequality_integer_slack_max_range,
        )
        if not source_report.compatible:
            return source_report, None

        try:
            prepared = cls._materialize_prepared_model(
                ommx_instance,
                source_report=source_report,
                uniform_penalty_weight=uniform_penalty_weight,
                penalty_weights=penalty_weights,
                inequality_integer_slack_max_range=inequality_integer_slack_max_range,
            )
        except InfeasibleDetected:
            raise
        except AdapterCompatibilityError as error:
            return error.report, None
        except Exception as error:
            violation = AdapterPreconditionViolation(
                condition="openjij.preparation.materialization",
                description=(
                    "The explicit OpenJij preparation transformations could not "
                    f"be materialized on an isolated copy: {error}"
                ),
                variable_ids=frozenset(
                    variable.id for variable in ommx_instance.used_decision_variables
                ),
                constraint_refs=cls._active_constraint_refs(ommx_instance),
                actual=str(error),
                limit="a successfully materialized prepared model",
            )
            return (
                AdapterCompatibilityReport(
                    adapter=source_report.adapter,
                    portable_report=source_report.portable_report,
                    preconditions_checked=True,
                    precondition_violations=(
                        *source_report.precondition_violations,
                        violation,
                    ),
                ),
                None,
            )
        return source_report, prepared

    @classmethod
    def check_preparation(
        cls,
        ommx_instance: Instance,
        *,
        uniform_penalty_weight: float | None = None,
        penalty_weights: Mapping[int, float] | None = None,
        inequality_integer_slack_max_range: int = 32,
    ) -> AdapterCompatibilityReport:
        """Dry-run the complete explicit preparation without mutating the input.

        This is intentionally separate from :meth:`check_compatibility`, which
        checks only the native Binary, unconstrained minimization translator
        input. The 53-bit log-encoding limit is a preparation precondition, not
        an OpenJij backend capability and not an ``ommx.v2.Feature``. A model
        proven infeasible while planning integer slack raises
        :class:`~ommx.adapter.InfeasibleDetected`.
        """

        report, _ = cls._plan_preparation(
            ommx_instance,
            uniform_penalty_weight=uniform_penalty_weight,
            penalty_weights=penalty_weights,
            inequality_integer_slack_max_range=inequality_integer_slack_max_range,
        )
        return report

    @classmethod
    def _check_encoding_compatibility(
        cls, ommx_instance: Instance
    ) -> AdapterCompatibilityReport:
        return cls._compatibility_report_against(
            ommx_instance,
            cls._ENCODING_INPUT_CAPABILITIES,
            "prepare.log_encode",
            lambda: cls._log_encoding_precondition_violations(ommx_instance),
        )

    @classmethod
    def _check_preconditions(
        cls,
        ommx_instance: Instance,
        portable_report,
    ) -> Iterable[AdapterPreconditionViolation]:
        _ = portable_report
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
    def prepare(
        cls,
        ommx_instance: Instance,
        *,
        uniform_penalty_weight: float | None = None,
        penalty_weights: Mapping[int, float] | None = None,
        inequality_integer_slack_max_range: int = 32,
    ) -> OpenJijPreparedModel:
        """Explicitly prepare a source model and return an auditable result.

        Raises :class:`~ommx.adapter.InfeasibleDetected` when variable bounds
        prove an inequality infeasible. Other preparation incompatibilities
        raise :class:`~ommx.adapter.AdapterCompatibilityError`.
        """

        with _tracer.start_as_current_span("prepare") as span:
            span.set_attribute("adapter", f"{cls.__module__}.{cls.__qualname__}")
            report, prepared = cls._plan_preparation(
                ommx_instance,
                uniform_penalty_weight=uniform_penalty_weight,
                penalty_weights=penalty_weights,
                inequality_integer_slack_max_range=inequality_integer_slack_max_range,
            )
            if prepared is None:
                raise AdapterCompatibilityError(report)
            return prepared

    @classmethod
    def _materialize_prepared_model(
        cls,
        ommx_instance: Instance,
        *,
        source_report: AdapterCompatibilityReport,
        uniform_penalty_weight: float | None,
        penalty_weights: Mapping[int, float] | None,
        inequality_integer_slack_max_range: int,
    ) -> OpenJijPreparedModel:
        """Explicitly prepare a source model and return an auditable result.

        Sense reversal and Integer log encoding are exact. Integer slack may be
        exact or approximate, and every constraint penalty is finite rather
        than an assertion of native or exact constrained support.
        """

        evaluation_instance = copy.deepcopy(ommx_instance)
        working = copy.deepcopy(ommx_instance)
        steps: list[OpenJijPreparationStep] = []

        special_refs = {
            AdditionalCapability.Indicator: frozenset(
                ConstraintRef("indicator", id) for id in working.indicator_constraints
            ),
            AdditionalCapability.OneHot: frozenset(
                ConstraintRef("one_hot", id) for id in working.one_hot_constraints
            ),
            AdditionalCapability.Sos1: frozenset(
                ConstraintRef("sos1", id) for id in working.sos1_constraints
            ),
        }
        converted_specials = working.reduce_capabilities(set())
        special_step_details = {
            AdditionalCapability.Indicator: (
                "indicator_lowering",
                "Lowered Indicator constraints exactly with validated Big-M bounds.",
            ),
            AdditionalCapability.OneHot: (
                "one_hot_lowering",
                "Lowered OneHot constraints exactly to regular equalities.",
            ),
            AdditionalCapability.Sos1: (
                "sos1_lowering",
                "Lowered SOS1 constraints exactly with validated Big-M bounds.",
            ),
        }
        for capability in (
            AdditionalCapability.Indicator,
            AdditionalCapability.OneHot,
            AdditionalCapability.Sos1,
        ):
            if capability not in converted_specials:
                continue
            operation, description = special_step_details[capability]
            steps.append(
                OpenJijPreparationStep(
                    operation=operation,
                    semantics=OpenJijPreparationSemantics.Exact,
                    description=description,
                    constraint_refs=special_refs[capability],
                )
            )

        source_integer_ids = frozenset(
            variable.id
            for variable in evaluation_instance.used_decision_variables
            if variable.kind == Kind.Integer
        )
        if source_integer_ids:
            working.log_encode(set(source_integer_ids))
            steps.append(
                OpenJijPreparationStep(
                    operation="integer_log_encoding",
                    semantics=OpenJijPreparationSemantics.Exact,
                    description=(
                        "Log-encoded source Integer variables after validating "
                        f"the {cls.MAX_LOG_ENCODING_BITS}-bit preparation limit."
                    ),
                    variable_ids=source_integer_ids,
                )
            )

        decoder_instance = copy.deepcopy(working)

        sense_reversed = working.as_minimization_problem()
        if sense_reversed:
            steps.append(
                OpenJijPreparationStep(
                    operation="sense_reversal",
                    semantics=OpenJijPreparationSemantics.Exact,
                    description=(
                        "Negated the objective for native minimization; sample "
                        "evaluation retains the source maximization sense."
                    ),
                )
            )

        inequality_ids = [
            constraint_id
            for constraint_id, constraint in working.constraints.items()
            if constraint.equality == Equality.LessThanOrEqualToZero
        ]
        for constraint_id in inequality_ids:
            constraint_ref = frozenset({ConstraintRef("regular", constraint_id)})
            try:
                working.convert_inequality_to_equality_with_integer_slack(
                    constraint_id, inequality_integer_slack_max_range
                )
            except RuntimeError as exact_error:
                try:
                    residual_step = working.add_integer_slack_to_inequality(
                        constraint_id, inequality_integer_slack_max_range
                    )
                except RuntimeError as approximate_error:
                    message = str(approximate_error)
                    if (
                        "The bound of `f(x)` in inequality constraint" in message
                        and "is positive" in message
                    ):
                        raise InfeasibleDetected(message) from None
                    raise
                steps.append(
                    OpenJijPreparationStep(
                        operation="approximate_integer_slack",
                        semantics=OpenJijPreparationSemantics.Approximate,
                        description=(
                            "Exact integer slack was unavailable "
                            f"({exact_error}); used a discrete slack with residual "
                            f"step {residual_step}."
                        ),
                        constraint_refs=constraint_ref,
                    )
                )
            else:
                if constraint_id in working.constraints:
                    operation = "exact_integer_slack"
                    description = "Converted the inequality with exact integer slack."
                else:
                    operation = "trivial_inequality_removal"
                    description = (
                        "Removed an inequality proven satisfied by the variable bounds."
                    )
                steps.append(
                    OpenJijPreparationStep(
                        operation=operation,
                        semantics=OpenJijPreparationSemantics.Exact,
                        description=description,
                        constraint_refs=constraint_ref,
                    )
                )

        remaining_constraint_ids = frozenset(working.constraints)
        if remaining_constraint_ids:
            if penalty_weights is not None:
                parametric = working.penalty_method()
                weights: dict[int, float] = {}
                for constraint_id in remaining_constraint_ids:
                    removed = parametric.removed_constraints[constraint_id]
                    parameter_id = int(
                        removed.removed_reason_parameters["parameter_id"]
                    )
                    weights[parameter_id] = penalty_weights[constraint_id]
                working = parametric.with_parameters(weights)
                penalty_description = (
                    "Applied positive per-constraint finite penalties."
                )
            else:
                assert uniform_penalty_weight is not None
                parametric = working.uniform_penalty_method()
                parameter = parametric.parameters[0]
                working = parametric.with_parameters(
                    {parameter.id: uniform_penalty_weight}
                )
                penalty_description = (
                    f"Applied finite uniform penalty weight {uniform_penalty_weight}."
                )
            steps.append(
                OpenJijPreparationStep(
                    operation="finite_penalty",
                    semantics=OpenJijPreparationSemantics.FinitePenalty,
                    description=penalty_description,
                    constraint_refs=frozenset(
                        [
                            *(
                                ConstraintRef("regular", constraint_id)
                                for constraint_id in remaining_constraint_ids
                                if constraint_id in ommx_instance.constraints
                            ),
                            *(
                                ref
                                for ref in cls._active_constraint_refs(ommx_instance)
                                if ref.family != "regular"
                            ),
                        ]
                    ),
                )
            )

        encoding_report = cls._check_encoding_compatibility(working)
        if not encoding_report.compatible:
            raise AdapterCompatibilityError(encoding_report)

        slack_integer_ids = frozenset(
            variable.id
            for variable in working.used_decision_variables
            if variable.kind == Kind.Integer
        )
        if slack_integer_ids:
            working.log_encode(set(slack_integer_ids))
            steps.append(
                OpenJijPreparationStep(
                    operation="integer_slack_log_encoding",
                    semantics=OpenJijPreparationSemantics.Exact,
                    description="Log-encoded Integer variables introduced by slack preparation.",
                    variable_ids=slack_integer_ids,
                )
            )

        final_report = cls.require_compatible(working)
        return OpenJijPreparedModel(
            _solver_instance=working,
            _decoder_instance=decoder_instance,
            _evaluation_instance=evaluation_instance,
            report=OpenJijPreparationReport(
                source_compatibility=source_report,
                encoding_compatibility=encoding_report,
                steps=tuple(steps),
                final_compatibility=final_report,
            ),
        )

    @classmethod
    def sample(
        cls,
        ommx_instance: Instance,
        *,
        preparation: bool = False,
        uniform_penalty_weight: float | None = None,
        penalty_weights: Mapping[int, float] | None = None,
        inequality_integer_slack_max_range: int = 32,
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
        """Sample a native model, or explicitly prepare a source Instance first.

        Set ``preparation=True`` to enable preparation options. If preparation
        proves an inequality infeasible, this raises
        :class:`~ommx.adapter.InfeasibleDetected`.
        """
        _ = diagnostics
        with _tracer.start_as_current_span("sample") as span:
            span.set_attribute("adapter", f"{cls.__module__}.{cls.__qualname__}")
            if preparation:
                solver_input: Instance | OpenJijPreparedModel = cls.prepare(
                    ommx_instance,
                    uniform_penalty_weight=uniform_penalty_weight,
                    penalty_weights=penalty_weights,
                    inequality_integer_slack_max_range=inequality_integer_slack_max_range,
                )
            else:
                if (
                    uniform_penalty_weight is not None
                    or penalty_weights is not None
                    or inequality_integer_slack_max_range != 32
                ):
                    raise ValueError(
                        "OpenJij preparation options require preparation=True"
                    )
                solver_input = ommx_instance
            sampler = cls(
                solver_input,
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
        preparation: bool = False,
        uniform_penalty_weight: float | None = None,
        penalty_weights: Mapping[int, float] | None = None,
        inequality_integer_slack_max_range: int = 32,
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
        """Return the best feasible sample from :meth:`sample`.

        When ``preparation=True``, a bound-proven infeasible inequality raises
        :class:`~ommx.adapter.InfeasibleDetected`.
        """
        _ = diagnostics
        sample_set = cls.sample(
            ommx_instance,
            preparation=preparation,
            uniform_penalty_weight=uniform_penalty_weight,
            penalty_weights=penalty_weights,
            inequality_integer_slack_max_range=inequality_integer_slack_max_range,
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
            decoder_variable_ids = {
                variable.id
                for variable in self._decoder_instance.used_decision_variables
            }
            samples = _decode_to_samples(
                data,
                variable_ids=decoder_variable_ids,
                default_values={id: 0.0 for id in decoder_variable_ids},
            )
            if self.preparation_report is None:
                return self.ommx_instance.evaluate_samples(samples)

            decoded = self._decoder_instance.evaluate_samples(samples)
            source_variable_ids = {
                variable.id for variable in self.ommx_instance.used_decision_variables
            }
            source_samples = Samples({})
            for sample_id in sorted(decoded.sample_ids()):
                decoded_state = decoded.get(sample_id).state
                entries: list[tuple[int, float]] = []
                for variable_id in source_variable_ids:
                    value = decoded_state.get(variable_id)
                    if value is None:
                        raise RuntimeError(
                            "OpenJij preparation decoder did not reconstruct "
                            f"source variable ID {variable_id}"
                        )
                    entries.append((variable_id, value))
                source_samples.append([sample_id], State(entries=entries))
            return self.ommx_instance.evaluate_samples(source_samples)

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

    def _prepare_sampler_input(self) -> None:
        if self._sampler_input_prepared:
            return

        with _tracer.start_as_current_span("convert"):
            hubo, _ = self._solver_instance.as_hubo_format()
            if any(len(k) > 2 for k in hubo.keys()):
                self._is_hubo = True
                self._hubo = hubo
            else:
                self._is_hubo = False
                qubo, _ = self._solver_instance.as_qubo_format()
                self._qubo = qubo

            self._sampler_input_prepared = True


@deprecated("Renamed to `decode_to_samples`")
def response_to_samples(response: oj.Response) -> Samples:
    """
    Deprecated: renamed to :meth:`decode_to_samples`
    """
    return decode_to_samples(response)


def _decode_to_samples(
    response: oj.Response,
    *,
    variable_ids: set[int] | None = None,
    default_values: Mapping[int, float] | None = None,
) -> Samples:
    # Create empty samples and append each state with its sample IDs
    # Since OpenJij does not issue the sample ID, we need to generate it in the responsibility of this OMMX Adapter
    samples = Samples({})  # Create empty samples
    sample_id = 0

    num_reads = len(response.record.num_occurrences)
    for i in range(num_reads):
        sample = response.record.sample[i]
        entries = dict(default_values or {})
        for variable, value in zip(response.variables, sample):
            variable_id = int(variable)  # type: ignore[arg-type]
            if variable_ids is None or variable_id in variable_ids:
                entries[variable_id] = value
        state = State(entries=entries.items())
        # `num_occurrences` is encoded into sample ID list.
        # For example, if `num_occurrences` is 2, there are two samples with the same state, thus two sample IDs are generated.
        ids = []
        for _ in range(response.record.num_occurrences[i]):
            ids.append(sample_id)
            sample_id += 1
        samples.append(ids, state)

    return samples


def decode_to_samples(response: oj.Response) -> Samples:
    """
    Convert `openjij.Response <https://openjij.github.io/OpenJij/reference/openjij/index.html#openjij.Response>`_ to :class:`Samples`
    """
    return _decode_to_samples(response)


@deprecated(
    "Use `OMMXOpenJijSAAdapter.sample`; pass preparation=True for explicit transformations"
)
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
    Deprecated: Use :meth:`OMMXOpenJijSAAdapter.sample` instead. This legacy
    helper accepts only the adapter's native Binary unconstrained minimization
    input; use ``preparation=True`` on :meth:`OMMXOpenJijSAAdapter.sample` for
    explicit transformations.
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
