"""OpenJij preparation planning and materialization."""

from __future__ import annotations

import copy
from collections.abc import Callable, Mapping
from typing import NoReturn

from ommx import AdditionalCapability, Equality, Instance, Kind
from ommx.adapter import (
    AdapterApplicabilityReport,
    AdapterPreconditionViolation,
    ConstraintRef,
    InfeasibleDetected,
)

from ._preparation import (
    OpenJijPreparation,
    OpenJijPreparationError,
    OpenJijPreparationFailure,
    OpenJijPreparationReport,
    OpenJijPreparationSourceCheck,
    OpenJijPreparationStep,
)
from ._preparation_checks import (
    MAX_LOG_ENCODING_BITS,
    active_constraint_refs,
    check_encoding_input,
    check_preparation_source,
)


def check_preparation(
    ommx_instance: Instance,
    *,
    check_input_applicability: Callable[[Instance], AdapterApplicabilityReport],
    uniform_penalty_weight: float | None = None,
    penalty_weights: Mapping[int, float] | None = None,
    inequality_integer_slack_max_range: int = 32,
    allow_approximate_integer_slack: bool = False,
) -> OpenJijPreparationReport:
    """Dry-run the complete explicit preparation without mutating the input."""
    report, _ = _plan_preparation(
        ommx_instance,
        check_input_applicability=check_input_applicability,
        uniform_penalty_weight=uniform_penalty_weight,
        penalty_weights=penalty_weights,
        inequality_integer_slack_max_range=inequality_integer_slack_max_range,
        allow_approximate_integer_slack=allow_approximate_integer_slack,
    )
    return report


def prepare(
    ommx_instance: Instance,
    *,
    check_input_applicability: Callable[[Instance], AdapterApplicabilityReport],
    uniform_penalty_weight: float | None = None,
    penalty_weights: Mapping[int, float] | None = None,
    inequality_integer_slack_max_range: int = 32,
    allow_approximate_integer_slack: bool = False,
) -> OpenJijPreparation:
    """Produce a separate Adapter input and an auditable preparation report."""
    report, prepared = _plan_preparation(
        ommx_instance,
        check_input_applicability=check_input_applicability,
        uniform_penalty_weight=uniform_penalty_weight,
        penalty_weights=penalty_weights,
        inequality_integer_slack_max_range=inequality_integer_slack_max_range,
        allow_approximate_integer_slack=allow_approximate_integer_slack,
    )
    if prepared is None:
        raise OpenJijPreparationError(report)
    return prepared


def _plan_preparation(
    ommx_instance: Instance,
    *,
    check_input_applicability: Callable[[Instance], AdapterApplicabilityReport],
    uniform_penalty_weight: float | None,
    penalty_weights: Mapping[int, float] | None,
    inequality_integer_slack_max_range: int,
    allow_approximate_integer_slack: bool,
) -> tuple[OpenJijPreparationReport, OpenJijPreparation | None]:
    source_check = check_preparation_source(
        ommx_instance,
        uniform_penalty_weight=uniform_penalty_weight,
        penalty_weights=penalty_weights,
        inequality_integer_slack_max_range=inequality_integer_slack_max_range,
    )
    if not source_check.conditions_hold:
        return _report(source_check), None

    steps: list[OpenJijPreparationStep] = []
    try:
        prepared = _materialize_preparation(
            ommx_instance,
            source_check=source_check,
            steps=steps,
            check_input_applicability=check_input_applicability,
            uniform_penalty_weight=uniform_penalty_weight,
            penalty_weights=penalty_weights,
            inequality_integer_slack_max_range=inequality_integer_slack_max_range,
            allow_approximate_integer_slack=allow_approximate_integer_slack,
        )
    except InfeasibleDetected:
        raise
    except OpenJijPreparationError as error:
        return error.report, None
    except Exception as error:
        failure = OpenJijPreparationFailure(
            reason="openjij.preparation.materialization",
            description=(
                "The explicit OpenJij preparation transformations could not "
                f"be materialized on an isolated copy: {error}"
            ),
            variable_ids=frozenset(
                variable.id for variable in ommx_instance.used_decision_variables
            ),
            constraint_refs=active_constraint_refs(ommx_instance),
            observed=str(error),
            expected="a successfully materialized prepared input",
        )
        return (
            _report(
                source_check,
                preparation_failures=(failure,),
                steps=steps,
            ),
            None,
        )
    return prepared.report, prepared


def _materialize_preparation(
    ommx_instance: Instance,
    *,
    source_check: OpenJijPreparationSourceCheck,
    steps: list[OpenJijPreparationStep],
    check_input_applicability: Callable[[Instance], AdapterApplicabilityReport],
    uniform_penalty_weight: float | None,
    penalty_weights: Mapping[int, float] | None,
    inequality_integer_slack_max_range: int,
    allow_approximate_integer_slack: bool,
) -> OpenJijPreparation:
    """Apply the ordered OpenJij preparation operations to an isolated copy."""
    source_instance = copy.deepcopy(ommx_instance)
    working = copy.deepcopy(ommx_instance)

    _lower_special_constraints(working, steps)
    _encode_source_integers(source_instance, working, steps)
    _reverse_maximization(working, steps)
    _convert_inequalities(
        working,
        source_check=source_check,
        steps=steps,
        inequality_integer_slack_max_range=inequality_integer_slack_max_range,
        allow_approximate_integer_slack=allow_approximate_integer_slack,
    )
    working = _apply_penalties(
        source_instance=source_instance,
        working=working,
        source_check=source_check,
        steps=steps,
        uniform_penalty_weight=uniform_penalty_weight,
        penalty_weights=penalty_weights,
    )
    _validate_encoding_input(
        working,
        source_check=source_check,
        steps=steps,
    )
    _encode_slack_integers(working, steps)

    input_applicability = check_input_applicability(working)
    report = _report(
        source_check,
        steps=steps,
        input_applicability=input_applicability,
    )
    if not report.is_successful:
        raise OpenJijPreparationError(report)
    return OpenJijPreparation(
        _input=working,
        _source_instance=source_instance,
        report=report,
    )


def _lower_special_constraints(
    working: Instance,
    steps: list[OpenJijPreparationStep],
) -> None:
    special_refs = {
        AdditionalCapability.Indicator: frozenset(
            ConstraintRef("indicator", constraint_id)
            for constraint_id in working.indicator_constraints
        ),
        AdditionalCapability.OneHot: frozenset(
            ConstraintRef("one_hot", constraint_id)
            for constraint_id in working.one_hot_constraints
        ),
        AdditionalCapability.Sos1: frozenset(
            ConstraintRef("sos1", constraint_id)
            for constraint_id in working.sos1_constraints
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
                description=description,
                constraint_refs=special_refs[capability],
            )
        )


def _encode_source_integers(
    source_instance: Instance,
    working: Instance,
    steps: list[OpenJijPreparationStep],
) -> None:
    source_integer_ids = frozenset(
        variable.id
        for variable in source_instance.used_decision_variables
        if variable.kind == Kind.Integer
    )
    if not source_integer_ids:
        return
    working.log_encode(set(source_integer_ids))
    steps.append(
        OpenJijPreparationStep(
            operation="integer_log_encoding",
            description=(
                "Log-encoded source Integer variables after validating "
                f"the {MAX_LOG_ENCODING_BITS}-bit preparation limit."
            ),
            variable_ids=source_integer_ids,
        )
    )


def _reverse_maximization(
    working: Instance,
    steps: list[OpenJijPreparationStep],
) -> None:
    if not working.as_minimization_problem():
        return
    steps.append(
        OpenJijPreparationStep(
            operation="sense_reversal",
            description=(
                "Negated the objective for the Adapter minimization input; sample "
                "evaluation retains the source maximization sense."
            ),
        )
    )


def _convert_inequalities(
    working: Instance,
    *,
    source_check: OpenJijPreparationSourceCheck,
    steps: list[OpenJijPreparationStep],
    inequality_integer_slack_max_range: int,
    allow_approximate_integer_slack: bool,
) -> None:
    inequality_ids = [
        constraint_id
        for constraint_id, constraint in working.constraints.items()
        if constraint.equality == Equality.LessThanOrEqualToZero
    ]
    for constraint_id in inequality_ids:
        constraint_refs = frozenset({ConstraintRef("regular", constraint_id)})
        try:
            working.convert_inequality_to_equality_with_integer_slack(
                constraint_id,
                inequality_integer_slack_max_range,
            )
        except RuntimeError as exact_error:
            exact_message = str(exact_error)
            if _reports_proven_infeasibility(exact_message):
                raise InfeasibleDetected(exact_message) from None
            if not allow_approximate_integer_slack:
                failure = OpenJijPreparationFailure(
                    reason="openjij.slack.approximation_explicit_selection",
                    description=(
                        "Exact integer slack was unavailable "
                        f"({exact_error}). Set "
                        "allow_approximate_integer_slack=True to permit "
                        "discrete slack approximation."
                    ),
                    constraint_refs=constraint_refs,
                    observed="not selected",
                    expected="allow_approximate_integer_slack=True",
                )
                _raise_preparation_failure(
                    source_check,
                    steps=steps,
                    failures=(failure,),
                )
            try:
                residual_step = working.add_integer_slack_to_inequality(
                    constraint_id,
                    inequality_integer_slack_max_range,
                )
            except RuntimeError as approximate_error:
                message = str(approximate_error)
                if _reports_proven_infeasibility(message):
                    raise InfeasibleDetected(message) from None
                raise
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
                    description=description,
                    constraint_refs=constraint_refs,
                )
            )


def _apply_penalties(
    *,
    source_instance: Instance,
    working: Instance,
    source_check: OpenJijPreparationSourceCheck,
    steps: list[OpenJijPreparationStep],
    uniform_penalty_weight: float | None,
    penalty_weights: Mapping[int, float] | None,
) -> Instance:
    remaining_constraint_ids = frozenset(working.constraints)
    if not remaining_constraint_ids:
        return working

    penalty_constraint_refs = frozenset(
        [
            *(
                ConstraintRef("regular", constraint_id)
                for constraint_id in remaining_constraint_ids
                if constraint_id in source_instance.constraints
            ),
            *(
                ref
                for ref in active_constraint_refs(source_instance)
                if ref.family != "regular"
            ),
        ]
    )
    if uniform_penalty_weight is None and penalty_weights is None:
        failure = OpenJijPreparationFailure(
            reason="openjij.penalty.explicit_selection",
            description=(
                "Constraints remaining after exact preparation require an "
                "explicitly selected finite penalty; constrained models are "
                "not part of the OpenJij input class."
            ),
            constraint_refs=penalty_constraint_refs,
            observed="not selected",
            expected="uniform_penalty_weight or penalty_weights",
        )
        _raise_preparation_failure(
            source_check,
            steps=steps,
            failures=(failure,),
        )

    if penalty_weights is not None:
        parametric = working.penalty_method()
        weights: dict[int, float] = {}
        for constraint_id in remaining_constraint_ids:
            removed = parametric.removed_constraints[constraint_id]
            parameter_id = int(removed.removed_reason_parameters["parameter_id"])
            weights[parameter_id] = penalty_weights[constraint_id]
        penalized = parametric.with_parameters(weights)
        penalty_description = "Applied positive per-constraint finite penalties."
    else:
        assert uniform_penalty_weight is not None
        parametric = working.uniform_penalty_method()
        parameter = parametric.parameters[0]
        penalized = parametric.with_parameters({parameter.id: uniform_penalty_weight})
        penalty_description = (
            f"Applied finite uniform penalty weight {uniform_penalty_weight}."
        )

    steps.append(
        OpenJijPreparationStep(
            operation="finite_penalty",
            description=penalty_description,
            constraint_refs=penalty_constraint_refs,
        )
    )
    return penalized


def _validate_encoding_input(
    working: Instance,
    *,
    source_check: OpenJijPreparationSourceCheck,
    steps: list[OpenJijPreparationStep],
) -> None:
    membership, preconditions_checked, violations = check_encoding_input(working)
    if not membership.is_member:
        violations = (
            AdapterPreconditionViolation(
                condition="openjij.preparation.encoding_input_class",
                description=(
                    "The prepared intermediate value is outside the class "
                    "supported by Integer log encoding."
                ),
                actual=str(membership),
                limit="Binary or Integer unconstrained minimization input",
            ),
        )
    if not preconditions_checked or violations:
        _raise_preparation_failure(
            source_check,
            steps=steps,
            failures=tuple(_as_preparation_failure(item) for item in violations),
        )


def _encode_slack_integers(
    working: Instance,
    steps: list[OpenJijPreparationStep],
) -> None:
    slack_integer_ids = frozenset(
        variable.id
        for variable in working.used_decision_variables
        if variable.kind == Kind.Integer
    )
    if not slack_integer_ids:
        return
    working.log_encode(set(slack_integer_ids))
    steps.append(
        OpenJijPreparationStep(
            operation="integer_slack_log_encoding",
            description=(
                "Log-encoded Integer variables introduced by slack preparation."
            ),
            variable_ids=slack_integer_ids,
        )
    )


def _report(
    source_check: OpenJijPreparationSourceCheck,
    *,
    preparation_failures: tuple[OpenJijPreparationFailure, ...] = (),
    steps: list[OpenJijPreparationStep] | tuple[OpenJijPreparationStep, ...] = (),
    input_applicability: AdapterApplicabilityReport | None = None,
) -> OpenJijPreparationReport:
    return OpenJijPreparationReport(
        source_check=source_check,
        preparation_failures=preparation_failures,
        steps=tuple(steps),
        input_applicability=input_applicability,
    )


def _raise_preparation_failure(
    source_check: OpenJijPreparationSourceCheck,
    *,
    steps: list[OpenJijPreparationStep],
    failures: tuple[OpenJijPreparationFailure, ...],
) -> NoReturn:
    raise OpenJijPreparationError(
        _report(
            source_check,
            preparation_failures=failures,
            steps=steps,
        )
    )


def _as_preparation_failure(
    violation: AdapterPreconditionViolation,
) -> OpenJijPreparationFailure:
    return OpenJijPreparationFailure(
        reason=violation.condition,
        description=violation.description,
        variable_ids=violation.variable_ids,
        constraint_refs=violation.constraint_refs,
        observed=violation.actual,
        expected=violation.limit,
    )


def _reports_proven_infeasibility(message: str) -> bool:
    return (
        "The bound of `f(x)` in inequality constraint" in message
        and "is positive" in message
    )
