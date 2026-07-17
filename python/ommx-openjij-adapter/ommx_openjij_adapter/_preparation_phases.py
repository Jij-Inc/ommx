"""Ordered phase operations for explicit OpenJij preparation."""

from __future__ import annotations

from ommx import (
    Equality,
    ExactIntegerSlackError,
    InfeasibleDetected,
    Instance,
    Kind,
    LogEncodingError,
    SpecialConstraintKind,
)
from ommx.adapter import ConstraintRef

from ._preparation import (
    OpenJijPreparationConfig,
    OpenJijPreparationFailure,
    OpenJijPreparationStep,
)
from ._preparation_checks import active_constraint_refs
from ._preparation_stages import (
    _AdapterInputCandidate,
    _Applied,
    _ApproximateIntegerSlack,
    _Blocked,
    _EncodingInput,
    _ExactIntegerSlack,
    _MinimizationSource,
    _PenaltyReady,
    _PhaseOutcome,
    _ProvenInfeasible,
    _RegularSource,
    _SourceEncoded,
    _SourceMember,
    _TrivialInequality,
)

MAX_LOG_ENCODING_BITS = 53


def _materialization_failure(
    operation: str,
    error: Exception,
    *,
    variable_ids: frozenset[int] = frozenset(),
    constraint_refs: frozenset[ConstraintRef] = frozenset(),
) -> OpenJijPreparationFailure:
    phase = operation.replace("_", " ")
    return OpenJijPreparationFailure(
        operation=operation,
        reason="openjij.preparation.materialization",
        description=f"The {phase} phase could not be materialized: {error}",
        variable_ids=variable_ids,
        constraint_refs=constraint_refs,
        observed=str(error),
        expected=f"the {phase} phase postcondition",
    )


def _log_encoding_unavailable(
    error: LogEncodingError,
    fallback_variable_ids: frozenset[int],
    *,
    operation: str,
) -> OpenJijPreparationFailure:
    kind = getattr(error, "kind", "unavailable")
    reason = {
        "non_finite_bound": "openjij.log_encoding.bound_finite",
        "outside_exact_integer_domain": ("openjij.log_encoding.exact_integer_range"),
        "range_too_large": "openjij.log_encoding.max_bits",
    }.get(kind, "openjij.log_encoding.unavailable")
    variable_id = getattr(error, "variable_id", None)
    variable_ids = (
        frozenset({variable_id})
        if isinstance(variable_id, int)
        else fallback_variable_ids
    )
    return OpenJijPreparationFailure(
        operation=operation,
        reason=reason,
        description=f"Exact Integer-to-Binary log encoding is unavailable: {error}",
        variable_ids=variable_ids,
        observed=getattr(error, "observed", str(error)),
        expected=getattr(
            error,
            "expected",
            (
                "finite unit-spaced Integer bounds encodable with at most "
                f"{MAX_LOG_ENCODING_BITS} bits"
            ),
        ),
    )


def lower_special_constraints(
    state: _SourceMember,
) -> _PhaseOutcome[_RegularSource]:
    working = state.take_instance()
    special_refs = {
        SpecialConstraintKind.Indicator: frozenset(
            ConstraintRef("indicator", constraint_id)
            for constraint_id in working.indicator_constraints
        ),
        SpecialConstraintKind.OneHot: frozenset(
            ConstraintRef("one_hot", constraint_id)
            for constraint_id in working.one_hot_constraints
        ),
        SpecialConstraintKind.Sos1: frozenset(
            ConstraintRef("sos1", constraint_id)
            for constraint_id in working.sos1_constraints
        ),
    }
    try:
        lowered_specials = working.lower_special_constraints(set(special_refs))
    except (RuntimeError, ValueError) as error:
        return _Blocked(
            failures=(
                _materialization_failure(
                    "special_constraint_lowering",
                    error,
                    constraint_refs=frozenset().union(*special_refs.values()),
                ),
            )
        )

    special_step_details = {
        SpecialConstraintKind.Indicator: (
            "indicator_lowering",
            "Lowered Indicator constraints exactly with validated Big-M bounds.",
        ),
        SpecialConstraintKind.OneHot: (
            "one_hot_lowering",
            "Lowered OneHot constraints exactly to regular equalities.",
        ),
        SpecialConstraintKind.Sos1: (
            "sos1_lowering",
            "Lowered SOS1 constraints exactly with validated Big-M bounds.",
        ),
    }
    steps = []
    for kind in (
        SpecialConstraintKind.Indicator,
        SpecialConstraintKind.OneHot,
        SpecialConstraintKind.Sos1,
    ):
        if kind not in lowered_specials:
            continue
        operation, description = special_step_details[kind]
        steps.append(
            OpenJijPreparationStep(
                operation=operation,
                description=description,
                constraint_refs=special_refs[kind],
            )
        )
    return _Applied(_RegularSource(working), tuple(steps))


def encode_source_integers(
    state: _RegularSource,
    source_integer_ids: frozenset[int],
) -> _PhaseOutcome[_SourceEncoded]:
    working = state.take_instance()
    if not source_integer_ids:
        return _Applied(_SourceEncoded(working, source_integer_ids))

    try:
        working.log_encode(set(source_integer_ids))
    except LogEncodingError as error:
        return _Blocked(
            failures=(
                _log_encoding_unavailable(
                    error,
                    source_integer_ids,
                    operation="integer_log_encoding",
                ),
            )
        )
    except (RuntimeError, ValueError) as error:
        return _Blocked(
            failures=(
                _materialization_failure(
                    "integer_log_encoding",
                    error,
                    variable_ids=source_integer_ids,
                ),
            )
        )

    return _Applied(
        _SourceEncoded(working, source_integer_ids),
        (
            OpenJijPreparationStep(
                operation="integer_log_encoding",
                description=(
                    "Log-encoded source Integer variables after validating "
                    f"the {MAX_LOG_ENCODING_BITS}-bit encoding limit."
                ),
                variable_ids=source_integer_ids,
            ),
        ),
    )


def normalize_sense(
    state: _SourceEncoded,
) -> _Applied[_MinimizationSource]:
    source_integer_ids = state.source_integer_ids
    working = state.take_instance()
    reversed_sense = working.as_minimization_problem()
    steps = ()
    if reversed_sense:
        steps = (
            OpenJijPreparationStep(
                operation="sense_reversal",
                description=(
                    "Negated the objective for the Adapter minimization input; "
                    "sample evaluation retains the source maximization sense."
                ),
            ),
        )
    return _Applied(
        _MinimizationSource(working, source_integer_ids),
        steps,
    )


def prepare_inequalities(
    state: _MinimizationSource,
    config: OpenJijPreparationConfig,
) -> _PhaseOutcome[_PenaltyReady]:
    working = state.take_instance()
    inequality_ids = frozenset(
        constraint_id
        for constraint_id, constraint in working.constraints.items()
        if constraint.equality == Equality.LessThanOrEqualToZero
    )
    slack_outcomes = []
    steps = []

    for constraint_id in sorted(inequality_ids):
        constraint_refs = frozenset({ConstraintRef("regular", constraint_id)})
        try:
            working.convert_inequality_to_equality_with_integer_slack(
                constraint_id,
                config.inequality_integer_slack_max_range,
            )
        except InfeasibleDetected as error:
            return _ProvenInfeasible(error)
        except ExactIntegerSlackError as exact_error:
            if not config.allow_approximate_integer_slack:
                return _Blocked(
                    failures=(
                        OpenJijPreparationFailure(
                            operation="integer_slack",
                            reason=("openjij.slack.approximation_explicit_selection"),
                            description=(
                                "Exact integer slack was unavailable "
                                f"({exact_error}). Set "
                                "allow_approximate_integer_slack=True to permit "
                                "discrete slack approximation."
                            ),
                            constraint_refs=constraint_refs,
                            observed="not selected",
                            expected="allow_approximate_integer_slack=True",
                        ),
                    ),
                    steps=tuple(steps),
                )
            try:
                residual_step = working.add_integer_slack_to_inequality(
                    constraint_id,
                    config.inequality_integer_slack_max_range,
                )
            except InfeasibleDetected as error:
                return _ProvenInfeasible(error)
            except (RuntimeError, ValueError) as error:
                return _Blocked(
                    failures=(
                        _materialization_failure(
                            "approximate_integer_slack",
                            error,
                            constraint_refs=constraint_refs,
                        ),
                    ),
                    steps=tuple(steps),
                )

            if residual_step is None:
                slack_outcomes.append(_TrivialInequality(constraint_id))
                steps.append(
                    OpenJijPreparationStep(
                        operation="trivial_inequality_removal",
                        description=(
                            "Removed an inequality proven satisfied by the variable "
                            "bounds."
                        ),
                        constraint_refs=constraint_refs,
                    )
                )
            else:
                slack_outcomes.append(
                    _ApproximateIntegerSlack(constraint_id, residual_step)
                )
                steps.append(
                    OpenJijPreparationStep(
                        operation="approximate_integer_slack",
                        description=(
                            "Exact integer slack was unavailable "
                            f"({exact_error}); used a discrete slack with residual "
                            f"step {residual_step}."
                        ),
                        constraint_refs=constraint_refs,
                    )
                )
        except (RuntimeError, ValueError) as error:
            return _Blocked(
                failures=(
                    _materialization_failure(
                        "exact_integer_slack",
                        error,
                        constraint_refs=constraint_refs,
                    ),
                ),
                steps=tuple(steps),
            )
        else:
            if constraint_id in working.constraints:
                slack_outcomes.append(_ExactIntegerSlack(constraint_id))
                operation = "exact_integer_slack"
                description = "Converted the inequality with exact integer slack."
            else:
                slack_outcomes.append(_TrivialInequality(constraint_id))
                operation = "trivial_inequality_removal"
                description = (
                    "Removed an inequality proven satisfied by the variable bounds."
                )
            steps.append(
                OpenJijPreparationStep(
                    operation=operation,
                    description=description,
                    constraint_refs=constraint_refs,
                )
            )

    return _Applied(
        _PenaltyReady(working, inequality_ids, tuple(slack_outcomes)),
        tuple(steps),
    )


def _penalty_policy_failures(
    working: Instance,
    source_instance: Instance,
    config: OpenJijPreparationConfig,
) -> tuple[OpenJijPreparationFailure, ...]:
    remaining_constraint_ids = frozenset(working.constraints)
    source_regular_ids = frozenset(source_instance.constraints)
    source_special_refs = frozenset(
        ref
        for ref in active_constraint_refs(source_instance)
        if ref.family != "regular"
    )
    source_has_constraints = bool(active_constraint_refs(source_instance))
    penalty_weights = config.penalty_weights
    penalty_selected = (
        config.uniform_penalty_weight is not None or penalty_weights is not None
    )
    failures = []

    if not source_has_constraints and penalty_selected:
        failures.append(
            OpenJijPreparationFailure(
                operation="finite_penalty",
                reason="openjij.penalty.unused",
                description="Penalty weights were supplied for an unconstrained model.",
                observed="penalty weights supplied",
                expected="no penalty configuration",
            )
        )

    if penalty_weights is not None:
        configured_ids = frozenset(penalty_weights)
        unexpected = configured_ids.difference(source_regular_ids)
        if unexpected:
            failures.append(
                OpenJijPreparationFailure(
                    operation="finite_penalty",
                    reason="openjij.penalty.weight_coverage",
                    description=(
                        "Per-constraint penalty weights contain unknown regular "
                        f"constraint IDs: {sorted(unexpected)}."
                    ),
                    constraint_refs=frozenset(
                        ConstraintRef("regular", constraint_id)
                        for constraint_id in unexpected
                    ),
                    observed=len(configured_ids),
                    expected=len(source_regular_ids),
                )
            )

        generated_ids = remaining_constraint_ids.difference(source_regular_ids)
        if generated_ids:
            failures.append(
                OpenJijPreparationFailure(
                    operation="finite_penalty",
                    reason="openjij.penalty.special_requires_uniform",
                    description=(
                        "Per-constraint weights cannot identify regular constraints "
                        "introduced by exact special-constraint lowering. Use a "
                        "uniform penalty weight."
                    ),
                    constraint_refs=source_special_refs,
                    observed="per-constraint penalty weights",
                    expected="uniform_penalty_weight",
                )
            )

        remaining_source_ids = remaining_constraint_ids.intersection(source_regular_ids)
        missing = remaining_source_ids.difference(configured_ids)
        if missing:
            failures.append(
                OpenJijPreparationFailure(
                    operation="finite_penalty",
                    reason="openjij.penalty.weight_coverage",
                    description=(
                        "Per-constraint penalty weights do not cover every regular "
                        f"constraint remaining at the penalty phase: {sorted(missing)}."
                    ),
                    constraint_refs=frozenset(
                        ConstraintRef("regular", constraint_id)
                        for constraint_id in missing
                    ),
                    observed=len(configured_ids),
                    expected=len(remaining_source_ids),
                )
            )

    if remaining_constraint_ids and not penalty_selected:
        failures.append(
            OpenJijPreparationFailure(
                operation="finite_penalty",
                reason="openjij.penalty.explicit_selection",
                description=(
                    "Constraints remaining after exact preparation require an "
                    "explicitly selected finite penalty; constrained models are "
                    "not part of the OpenJij input class."
                ),
                constraint_refs=frozenset(
                    [
                        *(
                            ConstraintRef("regular", constraint_id)
                            for constraint_id in remaining_constraint_ids
                            if constraint_id in source_regular_ids
                        ),
                        *source_special_refs,
                    ]
                ),
                observed="not selected",
                expected="uniform_penalty_weight or penalty_weights",
            )
        )

    return tuple(failures)


def apply_penalties(
    state: _PenaltyReady,
    source_instance: Instance,
    config: OpenJijPreparationConfig,
) -> _PhaseOutcome[_EncodingInput]:
    working = state.take_instance()
    failures = _penalty_policy_failures(working, source_instance, config)
    if failures:
        return _Blocked(failures=failures)

    remaining_constraint_ids = frozenset(working.constraints)
    if not remaining_constraint_ids:
        return _Applied(_EncodingInput(working))

    source_regular_ids = frozenset(source_instance.constraints)
    source_special_refs = frozenset(
        ref
        for ref in active_constraint_refs(source_instance)
        if ref.family != "regular"
    )
    penalty_constraint_refs = frozenset(
        [
            *(
                ConstraintRef("regular", constraint_id)
                for constraint_id in remaining_constraint_ids
                if constraint_id in source_regular_ids
            ),
            *source_special_refs,
        ]
    )

    try:
        if config.penalty_weights is not None:
            parametric = working.penalty_method()
            weights: dict[int, float] = {}
            for constraint_id in remaining_constraint_ids:
                removed = parametric.removed_constraints[constraint_id]
                parameter_id = int(removed.removed_reason_parameters["parameter_id"])
                weights[parameter_id] = config.penalty_weights[constraint_id]
            penalized = parametric.with_parameters(weights)
            penalty_description = "Applied positive per-constraint finite penalties."
        else:
            assert config.uniform_penalty_weight is not None
            parametric = working.uniform_penalty_method()
            parameter = parametric.parameters[0]
            penalized = parametric.with_parameters(
                {parameter.id: config.uniform_penalty_weight}
            )
            penalty_description = (
                "Applied finite uniform penalty weight "
                f"{config.uniform_penalty_weight}."
            )
    except (RuntimeError, ValueError) as error:
        return _Blocked(
            failures=(
                _materialization_failure(
                    "finite_penalty",
                    error,
                    constraint_refs=penalty_constraint_refs,
                ),
            )
        )

    return _Applied(
        _EncodingInput(penalized),
        (
            OpenJijPreparationStep(
                operation="finite_penalty",
                description=penalty_description,
                constraint_refs=penalty_constraint_refs,
            ),
        ),
    )


def encode_remaining_integers(
    state: _EncodingInput,
) -> _PhaseOutcome[_AdapterInputCandidate]:
    working = state.take_instance()
    integer_ids = frozenset(
        variable.id
        for variable in working.used_decision_variables
        if variable.kind == Kind.Integer
    )
    if not integer_ids:
        return _Applied(_AdapterInputCandidate(working))

    try:
        working.log_encode(set(integer_ids))
    except LogEncodingError as error:
        return _Blocked(
            failures=(
                _log_encoding_unavailable(
                    error,
                    integer_ids,
                    operation="integer_slack_log_encoding",
                ),
            )
        )
    except (RuntimeError, ValueError) as error:
        return _Blocked(
            failures=(
                _materialization_failure(
                    "integer_slack_log_encoding",
                    error,
                    variable_ids=integer_ids,
                ),
            )
        )

    return _Applied(
        _AdapterInputCandidate(working),
        (
            OpenJijPreparationStep(
                operation="integer_slack_log_encoding",
                description=(
                    "Log-encoded Integer variables introduced during preparation."
                ),
                variable_ids=integer_ids,
            ),
        ),
    )
