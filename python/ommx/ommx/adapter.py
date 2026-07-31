from __future__ import annotations

import copy
from abc import ABC, abstractmethod
from collections.abc import Iterable
from dataclasses import dataclass, field
from typing import (
    Any,
    ClassVar,
    Generic,
    Protocol,
    TypeVar,
    cast,
    overload,
    runtime_checkable,
)

from ommx._ommx_rust import DiagnosticCollector as DiagnosticCollector
from ommx import (
    InfeasibleDetected as InfeasibleDetected,
    Instance,
    InstanceClass,
    InstanceClassMembershipReport,
    Samples,
    SampleSet,
    Solution,
    SpecialConstraintKind,
    State,
)


SolverInput = Any
SolverOutput = Any
SamplerInput = Any
SamplerOutput = Any


@runtime_checkable
class DiagnosticReport(Protocol):
    """Adapter diagnostic report convertible with ``dataclasses.asdict``."""

    __dataclass_fields__: ClassVar[dict[str, Any]]


class DiagnosticsSink(Protocol):
    """Receiver for adapter-defined diagnostics emitted during a solve.

    Adapters may call ``record`` while the backend solver is still running,
    including from backend callbacks. Sink implementations should keep
    ``record`` append-only, defer validation or serialization until after the
    solve, and preserve the order in which diagnostics are received.

    A conforming sink must not raise from ``record``. If recording fails, the
    sink should log the failure and return normally. If ``record`` does raise,
    that is a sink contract violation; adapters may let the exception propagate
    and do not need to recover from it.
    """

    def record(self, diagnostic: DiagnosticReport) -> None:
        """Record one adapter-defined dataclass diagnostic report or event.

        This method must not raise under normal sink failures. Custom sinks
        should log failures and return instead.
        """


@dataclass(frozen=True, slots=True)
class ConstraintRef:
    """Constraint identity qualified by its independently scoped family."""

    family: str
    id: int


PreconditionValue = str | int | float | bool | None


@dataclass(frozen=True, slots=True)
class AdapterPreconditionViolation:
    """One adapter-owned condition that an OMMX input class cannot express."""

    condition: str
    description: str
    variable_ids: frozenset[int] = field(default_factory=frozenset)
    constraint_refs: frozenset[ConstraintRef] = field(default_factory=frozenset)
    actual: PreconditionValue = None
    limit: PreconditionValue = None


@dataclass(frozen=True, slots=True)
class AdapterApplicabilityReport:
    """Combined input-class and adapter-specific applicability result."""

    adapter: str
    input_membership: InstanceClassMembershipReport
    preconditions_checked: bool
    precondition_violations: tuple[AdapterPreconditionViolation, ...]

    def __post_init__(self) -> None:
        if self.preconditions_checked != self.input_membership.is_member:
            raise ValueError(
                "preconditions_checked must be true exactly when input membership holds"
            )
        if not self.preconditions_checked and self.precondition_violations:
            raise ValueError(
                "precondition violations require adapter preconditions to be checked"
            )

    @property
    def is_applicable(self) -> bool:
        return (
            self.input_membership.is_member
            and self.preconditions_checked
            and not self.precondition_violations
        )

    def __str__(self) -> str:
        if not self.input_membership.is_member:
            return f"{self.adapter} is not applicable:\n{self.input_membership}"
        if self.precondition_violations:
            details = "\n".join(
                f"- {violation.condition}: {violation.description}"
                for violation in self.precondition_violations
            )
            return f"{self.adapter} preconditions failed:\n{details}"
        return f"{self.adapter} is applicable"


class AdapterNotApplicableError(ValueError):
    """Raised when an instance is not applicable to an adapter."""

    report: AdapterApplicabilityReport

    def __init__(self, report: AdapterApplicabilityReport):
        self.report = report
        super().__init__(str(report))


PreparationDiagnosticValue = str | int | float | bool | None


@dataclass(frozen=True, slots=True)
class PreparationPolicy:
    """Caller-owned restrictions on an Adapter's preparation candidates.

    ``None`` leaves the Adapter's declared special-constraint lowering
    candidates unrestricted. A concrete set can only remove candidates; an
    empty set therefore forbids every automatic special-constraint lowering.
    """

    allowed_special_constraint_lowerings: frozenset[SpecialConstraintKind] | None = None

    def __post_init__(self) -> None:
        allowed = self.allowed_special_constraint_lowerings
        if allowed is None:
            return
        try:
            snapshot = frozenset(allowed)
        except TypeError as error:
            raise TypeError(
                "allowed_special_constraint_lowerings must be an iterable of "
                "SpecialConstraintKind values or None"
            ) from error
        if not all(isinstance(kind, SpecialConstraintKind) for kind in snapshot):
            raise TypeError(
                "allowed_special_constraint_lowerings must contain only "
                "SpecialConstraintKind values"
            )
        object.__setattr__(self, "allowed_special_constraint_lowerings", snapshot)


@dataclass(frozen=True, slots=True)
class PreparationTransform:
    """Audit receipt for one applied SDK Transform.

    This is a record of a Transform that was applied while producing an
    Adapter input. It is not an executable Transform or a formal proof object.
    In particular, it does not provide a public ``encode`` operation.
    """

    name: str
    description: str
    variable_ids: frozenset[int] = field(default_factory=frozenset)
    constraint_refs: frozenset[ConstraintRef] = field(default_factory=frozenset)
    special_constraint_kinds: frozenset[SpecialConstraintKind] = field(
        default_factory=frozenset
    )

    def __post_init__(self) -> None:
        if not self.name:
            raise ValueError("preparation transform name must not be empty")
        if not all(
            isinstance(kind, SpecialConstraintKind)
            for kind in self.special_constraint_kinds
        ):
            raise TypeError(
                "special_constraint_kinds must contain only "
                "SpecialConstraintKind values"
            )
        object.__setattr__(self, "variable_ids", frozenset(self.variable_ids))
        object.__setattr__(self, "constraint_refs", frozenset(self.constraint_refs))
        object.__setattr__(
            self,
            "special_constraint_kinds",
            frozenset(self.special_constraint_kinds),
        )


@dataclass(frozen=True, slots=True)
class PreparationFailure:
    """One structured failure discovered while preparing an Adapter input."""

    name: str
    reason: str
    description: str
    variable_ids: frozenset[int] = field(default_factory=frozenset)
    constraint_refs: frozenset[ConstraintRef] = field(default_factory=frozenset)
    observed: PreparationDiagnosticValue = None
    expected: PreparationDiagnosticValue = None

    def __post_init__(self) -> None:
        if not self.name:
            raise ValueError("preparation failure name must not be empty")
        if not self.reason:
            raise ValueError("preparation failure reason must not be empty")
        object.__setattr__(self, "variable_ids", frozenset(self.variable_ids))
        object.__setattr__(self, "constraint_refs", frozenset(self.constraint_refs))


@dataclass(frozen=True, slots=True)
class PreparationReport:
    """Audit of one common SolverAdapter preparation attempt."""

    policy: PreparationPolicy
    source_applicability: AdapterApplicabilityReport
    transforms: tuple[PreparationTransform, ...]
    preparation_failures: tuple[PreparationFailure, ...] = ()
    input_applicability: AdapterApplicabilityReport | None = None

    def __post_init__(self) -> None:
        object.__setattr__(self, "transforms", tuple(self.transforms))
        object.__setattr__(
            self,
            "preparation_failures",
            tuple(self.preparation_failures),
        )
        if self.preparation_failures and self.input_applicability is not None:
            raise ValueError(
                "a preparation report cannot contain both preparation failures "
                "and an input applicability result"
            )

    @property
    def is_successful(self) -> bool:
        return (
            not self.preparation_failures
            and self.input_applicability is not None
            and self.input_applicability.is_applicable
        )


@runtime_checkable
class PreparationReportProtocol(Protocol):
    """Common read-only surface exposed by Adapter preparation reports."""

    @property
    def policy(self) -> PreparationPolicy: ...

    @property
    def transforms(self) -> tuple[PreparationTransform, ...]: ...

    @property
    def preparation_failures(self) -> tuple[PreparationFailure, ...]: ...

    @property
    def input_applicability(self) -> AdapterApplicabilityReport | None: ...

    @property
    def is_successful(self) -> bool: ...


PreparationReportT = TypeVar("PreparationReportT", bound=PreparationReportProtocol)
PreparationReportT_co = TypeVar(
    "PreparationReportT_co", bound=PreparationReportProtocol, covariant=True
)


class PreparationError(ValueError, Generic[PreparationReportT]):
    """Raised when preparation cannot produce an applicable Adapter input."""

    report: PreparationReportT

    def __init__(self, report: PreparationReportT):
        self.report = report
        preparation_failures = getattr(report, "preparation_failures", ())
        input_applicability = getattr(report, "input_applicability", None)
        if preparation_failures:
            details = "\n".join(
                f"- {failure.name}/{failure.reason}: {failure.description}"
                for failure in preparation_failures
            )
            message = f"Adapter input preparation failed:\n{details}"
        elif input_applicability is not None:
            message = (
                "Preparation did not produce an applicable Adapter input:\n"
                f"{input_applicability}"
            )
        else:
            message = "Preparation did not produce an Adapter input"
        super().__init__(message)


@dataclass(frozen=True, slots=True, init=False)
class Preparation(Generic[PreparationReportT_co]):
    """An auditable source-to-Adapter-input relation with output decoding.

    Values are created by :meth:`SolverAdapter.prepare` or an Adapter-specific
    preparation pipeline. ``source`` and ``input`` are isolated snapshots.
    Direct Adapter outputs still belong to ``input``; :meth:`decode` returns a
    separately evaluated source-side output.
    """

    _source: Instance = field(repr=False)
    _input: Instance = field(repr=False)
    policy: PreparationPolicy
    _report: PreparationReportT_co = field(repr=False)
    _input_applicability: AdapterApplicabilityReport = field(repr=False)
    _preserves_optimality: bool = field(repr=False)
    _is_identity: bool = field(repr=False)

    def __init__(self) -> None:
        raise TypeError("Preparation is created only by SolverAdapter.prepare()")

    @classmethod
    def _create(
        cls,
        *,
        source: Instance,
        input: Instance,
        report: PreparationReportT,
        preserves_optimality: bool = False,
    ) -> Preparation[PreparationReportT]:
        """Create a value after an Adapter has checked the concrete input."""
        if not isinstance(preserves_optimality, bool):
            raise TypeError("preserves_optimality must be a bool")
        if not isinstance(report, PreparationReportProtocol):
            raise TypeError("report must implement PreparationReportProtocol")
        if not isinstance(report.policy, PreparationPolicy):
            raise TypeError("Preparation report policy must be a PreparationPolicy")
        if report.is_successful is not True:
            raise ValueError("Preparation requires a successful report")
        input_applicability = report.input_applicability
        if input_applicability is None or not input_applicability.is_applicable:
            raise ValueError("Preparation requires an applicable Adapter input")

        source_snapshot = copy.deepcopy(source)
        input_snapshot = copy.deepcopy(input)
        preparation = object.__new__(cls)
        object.__setattr__(preparation, "_source", source_snapshot)
        object.__setattr__(preparation, "_input", input_snapshot)
        object.__setattr__(preparation, "policy", report.policy)
        object.__setattr__(preparation, "_report", report)
        object.__setattr__(
            preparation,
            "_input_applicability",
            input_applicability,
        )
        object.__setattr__(
            preparation,
            "_preserves_optimality",
            preserves_optimality,
        )
        object.__setattr__(
            preparation,
            "_is_identity",
            source_snapshot.to_v2_bytes() == input_snapshot.to_v2_bytes(),
        )
        return cast(Preparation[PreparationReportT], preparation)

    @property
    def report(self) -> PreparationReportT_co:
        """Return the Adapter-specific audit report for this preparation."""
        return self._report

    @property
    def source(self) -> Instance:
        """Return an isolated copy of the exact caller-supplied source."""
        return copy.deepcopy(self._source)

    @property
    def input(self) -> Instance:
        """Return an isolated copy of the applicable Adapter input."""
        return copy.deepcopy(self._input)

    @overload
    def decode(self, output: Solution) -> Solution: ...

    @overload
    def decode(self, output: SampleSet) -> SampleSet: ...

    def decode(self, output: Solution | SampleSet) -> Solution | SampleSet:
        """Decode an input-side output and reevaluate it against ``source``.

        For non-identity preparation, auxiliary input variable IDs are removed
        before source evaluation. Every sample is decoded independently before
        source feasibility or best-sample selection is considered.
        """
        if not isinstance(output, (Solution, SampleSet)):
            raise TypeError("output must be a Solution or SampleSet")
        if self._is_identity:
            return copy.deepcopy(output)

        source_variable_ids = {
            variable.id for variable in self._source.decision_variables
        }
        if isinstance(output, Solution):
            state = self._decode_state(output.state, source_variable_ids)
            decoded = self._source.evaluate(state)
            if self._preserves_optimality:
                decoded.optimality = output.optimality
            return decoded

        source_samples = Samples({})
        for sample_id in sorted(output.sample_ids()):
            state = self._decode_state(
                output.get(sample_id).state,
                source_variable_ids,
            )
            source_samples.append([sample_id], state)
        return self._source.evaluate_samples(source_samples)

    @staticmethod
    def _decode_state(state: State, source_variable_ids: set[int]) -> State:
        entries: list[tuple[int, float]] = []
        for variable_id in source_variable_ids:
            value = state.get(variable_id)
            if value is None:
                raise RuntimeError(
                    "Preparation decode could not reconstruct source variable "
                    f"ID {variable_id}"
                )
            entries.append((variable_id, value))
        return State(entries=entries)


class SolverAdapter(ABC):
    """
    An abstract interface for OMMX Solver Adapters, defining how solvers should be used with OMMX.

    See the `implementation guide <https://jij-inc-ommx.readthedocs-hosted.com/en/latest/tutorial/implement_adapter.html>`_ for more details.

    Subclasses declare ``INPUT_CLASS`` as the OMMX-defined structural class used
    by the first applicability condition. ``check_applicability`` does not mutate
    the input and combines class membership with the adapter's
    ``_check_preconditions`` hook.

    ``INPUT_CLASS`` describes only which exact inputs an adapter accepts; it does
    not prescribe how the subclass processes them. Direct constructors,
    :meth:`solve`, and :meth:`sample` never prepare their input. The separate
    :meth:`prepare` workflow may lower only the canonical special-constraint
    families declared by the Adapter and permitted by the caller's policy.
    """

    INPUT_CLASS: ClassVar[InstanceClass | None] = None
    PREPARATION_SPECIAL_CONSTRAINT_LOWERINGS: ClassVar[
        tuple[SpecialConstraintKind, ...]
    ] = ()

    @classmethod
    def prepare(
        cls,
        source: Instance,
        *,
        policy: PreparationPolicy | None = None,
    ) -> Preparation[PreparationReportProtocol]:
        """Prepare an isolated ``source`` for this Adapter.

        Identity is preferred whenever ``source`` is already applicable. For a
        non-applicable source, this base implementation lowers the active
        special-constraint kinds in the Adapter's declared candidate tuple,
        intersected with the caller's policy. The concrete produced input is
        always checked again before a :class:`Preparation` is returned.

        This method does not call :meth:`solve` and never mutates ``source``.
        Adapter packages may override it for a target-specific pipeline while
        retaining identity preference, caller isolation, policy restriction,
        and final-applicability checking.
        """
        if not isinstance(source, Instance):
            raise TypeError("source must be an Instance")
        normalized_policy = PreparationPolicy() if policy is None else policy
        if not isinstance(normalized_policy, PreparationPolicy):
            raise TypeError("policy must be a PreparationPolicy")

        declared_candidates = cls._validated_preparation_lowering_candidates()
        source_applicability = cls.check_applicability(source)
        if source_applicability.is_applicable:
            report = PreparationReport(
                policy=normalized_policy,
                source_applicability=source_applicability,
                transforms=(),
                input_applicability=source_applicability,
            )
            return Preparation._create(
                source=source,
                input=source,
                report=report,
                preserves_optimality=True,
            )

        working = copy.deepcopy(source)
        active_kinds = working.active_special_constraint_kinds
        allowed_kinds = normalized_policy.allowed_special_constraint_lowerings
        selected_candidates = tuple(
            kind
            for kind in declared_candidates
            if kind in active_kinds and (allowed_kinds is None or kind in allowed_kinds)
        )
        selected_set = set(selected_candidates)
        blocked_by_policy = tuple(
            kind
            for kind in declared_candidates
            if kind in active_kinds
            and allowed_kinds is not None
            and kind not in allowed_kinds
        )
        if blocked_by_policy:
            kind_names = {
                SpecialConstraintKind.Indicator: "Indicator",
                SpecialConstraintKind.OneHot: "OneHot",
                SpecialConstraintKind.Sos1: "Sos1",
            }
            blocked_refs = frozenset(
                ref
                for kind in blocked_by_policy
                for ref in cls._special_constraint_refs(working, kind)
            )
            failure = PreparationFailure(
                name="special_constraint_lowering",
                reason="preparation.policy.special_constraint_lowering_not_allowed",
                description=(
                    "The caller policy forbids an active special-constraint "
                    "lowering declared by this Adapter."
                ),
                constraint_refs=blocked_refs,
                observed=", ".join(kind_names[kind] for kind in blocked_by_policy),
                expected="included in allowed_special_constraint_lowerings",
            )
            report = PreparationReport(
                policy=normalized_policy,
                source_applicability=source_applicability,
                transforms=(),
                preparation_failures=(failure,),
            )
            raise PreparationError(report)
        selected_refs = frozenset(
            ref
            for kind in selected_candidates
            for ref in cls._special_constraint_refs(working, kind)
        )

        try:
            lowered = working.lower_special_constraints(selected_set)
        except (RuntimeError, ValueError) as error:
            failure = PreparationFailure(
                name="special_constraint_lowering",
                reason="materialization",
                description=str(error),
                constraint_refs=selected_refs,
                observed=str(error),
                expected="all selected special constraints are exactly lowerable",
            )
            report = PreparationReport(
                policy=normalized_policy,
                source_applicability=source_applicability,
                transforms=(),
                preparation_failures=(failure,),
            )
            raise PreparationError(report) from error

        transforms = tuple(
            cls._special_constraint_transform(kind, selected_refs)
            for kind in selected_candidates
            if kind in lowered
        )
        input_applicability = cls.check_applicability(working)
        report = PreparationReport(
            policy=normalized_policy,
            source_applicability=source_applicability,
            transforms=transforms,
            input_applicability=input_applicability,
        )
        if not report.is_successful:
            raise PreparationError(report)
        return Preparation._create(
            source=source,
            input=working,
            report=report,
            preserves_optimality=True,
        )

    @classmethod
    def _validated_preparation_lowering_candidates(
        cls,
    ) -> tuple[SpecialConstraintKind, ...]:
        candidates = cls.PREPARATION_SPECIAL_CONSTRAINT_LOWERINGS
        if not isinstance(candidates, tuple):
            raise TypeError("PREPARATION_SPECIAL_CONSTRAINT_LOWERINGS must be a tuple")
        if not all(isinstance(kind, SpecialConstraintKind) for kind in candidates):
            raise TypeError(
                "PREPARATION_SPECIAL_CONSTRAINT_LOWERINGS must contain only "
                "SpecialConstraintKind values"
            )
        if len(set(candidates)) != len(candidates):
            raise ValueError(
                "PREPARATION_SPECIAL_CONSTRAINT_LOWERINGS must not contain "
                "duplicate kinds"
            )
        sdk_order = (
            SpecialConstraintKind.Indicator,
            SpecialConstraintKind.OneHot,
            SpecialConstraintKind.Sos1,
        )
        declared_in_sdk_order = tuple(kind for kind in sdk_order if kind in candidates)
        if candidates != declared_in_sdk_order:
            raise ValueError(
                "PREPARATION_SPECIAL_CONSTRAINT_LOWERINGS must follow SDK order: "
                "Indicator, OneHot, Sos1"
            )
        return candidates

    @staticmethod
    def _special_constraint_refs(
        instance: Instance,
        kind: SpecialConstraintKind,
    ) -> frozenset[ConstraintRef]:
        if kind == SpecialConstraintKind.Indicator:
            return frozenset(
                ConstraintRef("indicator", constraint_id)
                for constraint_id in instance.indicator_constraints
            )
        if kind == SpecialConstraintKind.OneHot:
            return frozenset(
                ConstraintRef("one_hot", constraint_id)
                for constraint_id in instance.one_hot_constraints
            )
        return frozenset(
            ConstraintRef("sos1", constraint_id)
            for constraint_id in instance.sos1_constraints
        )

    @classmethod
    def _special_constraint_transform(
        cls,
        kind: SpecialConstraintKind,
        selected_refs: frozenset[ConstraintRef],
    ) -> PreparationTransform:
        details = {
            SpecialConstraintKind.Indicator: (
                "indicator_lowering",
                "Lowered Indicator constraints exactly with validated Big-M bounds.",
                "indicator",
            ),
            SpecialConstraintKind.OneHot: (
                "one_hot_lowering",
                "Lowered OneHot constraints exactly to regular equalities.",
                "one_hot",
            ),
            SpecialConstraintKind.Sos1: (
                "sos1_lowering",
                "Lowered SOS1 constraints exactly with validated Big-M bounds.",
                "sos1",
            ),
        }
        name, description, family = details[kind]
        return PreparationTransform(
            name=name,
            description=description,
            constraint_refs=frozenset(
                ref for ref in selected_refs if ref.family == family
            ),
            special_constraint_kinds=frozenset({kind}),
        )

    @classmethod
    def check_applicability(cls, ommx_instance: Instance) -> AdapterApplicabilityReport:
        """Inspect applicability without mutating or preparing ``ommx_instance``.

        Adapter-specific preconditions run only after at least one complete
        input-class clause contains the instance. The hook receives an isolated
        copy so it cannot mutate the caller's instance. Any explicitly
        transformed value is a different input and must be checked separately.
        """
        input_class = cls.INPUT_CLASS
        if input_class is None:
            raise TypeError(
                f"{cls.__module__}.{cls.__qualname__} must declare INPUT_CLASS"
            )

        input_membership = input_class.check_membership(ommx_instance)
        adapter = f"{cls.__module__}.{cls.__qualname__}"
        if not input_membership.is_member:
            return AdapterApplicabilityReport(
                adapter=adapter,
                input_membership=input_membership,
                preconditions_checked=False,
                precondition_violations=(),
            )

        violations = tuple(
            cls._check_preconditions(copy.copy(ommx_instance), input_membership)
        )
        if not all(
            isinstance(violation, AdapterPreconditionViolation)
            for violation in violations
        ):
            raise TypeError(
                f"{adapter}._check_preconditions() must return "
                "AdapterPreconditionViolation values"
            )
        return AdapterApplicabilityReport(
            adapter=adapter,
            input_membership=input_membership,
            preconditions_checked=True,
            precondition_violations=violations,
        )

    @classmethod
    def require_applicable(cls, ommx_instance: Instance) -> AdapterApplicabilityReport:
        """Return the report or raise :class:`AdapterNotApplicableError`."""
        report = cls.check_applicability(ommx_instance)
        if not report.is_applicable:
            raise AdapterNotApplicableError(report)
        return report

    @classmethod
    def _check_preconditions(
        cls,
        ommx_instance: Instance,
        input_membership: InstanceClassMembershipReport,
    ) -> Iterable[AdapterPreconditionViolation]:
        """Return adapter-owned violations after input-class membership holds."""
        return ()

    @classmethod
    @abstractmethod
    def solve(
        cls,
        ommx_instance: Instance,
        *,
        diagnostics: DiagnosticsSink | None = None,
    ) -> Solution:
        """Solve an OMMX instance.

        ``Run.log_solve`` owns the reserved ``diagnostics`` keyword. When
        called with ``store_diagnostics=True``, it passes a sink to the adapter
        and stores recorded diagnostics with the Solve entry. Adapters may
        record adapter-defined dataclass diagnostics into the sink during the
        solve; ``None`` means diagnostics are disabled. Adapters do not need to
        catch exceptions raised by a non-conforming diagnostics sink.
        """
        pass

    @property
    @abstractmethod
    def solver_input(self) -> SolverInput:
        pass

    @abstractmethod
    def decode(self, data: SolverOutput) -> Solution:
        pass


class SamplerAdapter(SolverAdapter):
    """
    An abstract interface for OMMX Sampler Adapters, defining how samplers should be used with OMMX.

    See the `implementation guide <https://jij-inc-ommx.readthedocs-hosted.com/en/latest/tutorial/implement_adapter.html>`_ for more details.
    """

    @classmethod
    @abstractmethod
    def sample(
        cls,
        ommx_instance: Instance,
        *,
        diagnostics: DiagnosticsSink | None = None,
    ) -> SampleSet:
        """Sample an OMMX instance.

        ``Run.log_sample`` owns the reserved ``diagnostics`` keyword and uses
        it the same way as ``Run.log_solve``. ``None`` means diagnostics are
        disabled.
        """
        pass

    @property
    @abstractmethod
    def sampler_input(self) -> SamplerInput:
        pass

    @abstractmethod
    def decode_to_sampleset(self, data: SamplerOutput) -> SampleSet:
        pass


class UnboundedDetected(Exception):
    """
    Raised when the problem is proven to be unbounded.

    This corresponds to ``Optimality.OPTIMALITY_UNBOUNDED`` and indicates that
    the mathematical model itself is unbounded.
    Should not be used when unboundedness cannot be proven (e.g., heuristic solvers).
    """

    pass


class NoSolutionReturned(Exception):
    """
    Raised when no solution was returned.

    This indicates that the solver did not return any solution (whether feasible
    or not) (e.g., due to time limits).
    This does not prove that the mathematical model itself is infeasible.
    """

    pass
