"""OpenJij preparation source classes and Adapter-owned preconditions."""

from __future__ import annotations

import copy
from collections.abc import Callable, Iterable
from math import isfinite

from ommx import (
    DegreeBound,
    Equality,
    Instance,
    InstanceClass,
    InstanceClassClause,
    InstanceClassMembershipReport,
    Kind,
    Sense,
)
from ommx.adapter import AdapterPreconditionViolation, ConstraintRef

from ._preparation import OpenJijPreparationConfig, OpenJijPreparationSourceCheck


# This describes sources accepted by the explicit preparation operation,
# not inputs accepted directly by the OpenJij Adapter.
PREPARATION_SOURCE_CLASS = InstanceClass(
    [
        InstanceClassClause(
            label="openjij-preparation-source",
            allowed_variable_kinds={Kind.Binary, Kind.Integer},
            objective_degree_bound=DegreeBound.unbounded(),
            regular_constraint_degree_bounds={
                Equality.EqualToZero: DegreeBound.unbounded(),
                Equality.LessThanOrEqualToZero: DegreeBound.unbounded(),
            },
            indicator_constraint_degree_bounds={
                Equality.EqualToZero: DegreeBound.unbounded(),
                Equality.LessThanOrEqualToZero: DegreeBound.unbounded(),
            },
            allows_one_hot=True,
            allows_sos1=True,
            allowed_senses={Sense.Minimize, Sense.Maximize},
        )
    ]
)

ENCODING_INPUT_CLASS = InstanceClass(
    [
        InstanceClassClause(
            label="openjij-log-encoding-input",
            allowed_variable_kinds={Kind.Binary, Kind.Integer},
            objective_degree_bound=DegreeBound.unbounded(),
            allowed_senses={Sense.Minimize},
        )
    ]
)

MAX_LOG_ENCODING_BITS = 53


def active_constraint_refs(ommx_instance: Instance) -> frozenset[ConstraintRef]:
    return frozenset(
        [
            *(ConstraintRef("regular", id) for id in ommx_instance.constraints),
            *(
                ConstraintRef("indicator", id)
                for id in ommx_instance.indicator_constraints
            ),
            *(ConstraintRef("one_hot", id) for id in ommx_instance.one_hot_constraints),
            *(ConstraintRef("sos1", id) for id in ommx_instance.sos1_constraints),
        ]
    )


def _check_class_preconditions(
    ommx_instance: Instance,
    input_class: InstanceClass,
    check_preconditions: Callable[[], Iterable[AdapterPreconditionViolation]],
) -> tuple[
    InstanceClassMembershipReport,
    bool,
    tuple[AdapterPreconditionViolation, ...],
]:
    membership = input_class.check_membership(ommx_instance)
    if not membership.is_member:
        return membership, False, ()
    return membership, True, tuple(check_preconditions())


def _log_encoding_precondition_violations(
    ommx_instance: Instance,
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
    max_exact_integer = float(2**MAX_LOG_ENCODING_BITS)
    max_range_width = float(2**MAX_LOG_ENCODING_BITS - 1)
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
                limit = MAX_LOG_ENCODING_BITS
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
                    f"{MAX_LOG_ENCODING_BITS} bits"
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


def _penalty_precondition_violations(
    ommx_instance: Instance,
    config: OpenJijPreparationConfig,
) -> tuple[AdapterPreconditionViolation, ...]:
    constraint_ids = frozenset(ommx_instance.constraints)
    constraint_refs = active_constraint_refs(ommx_instance)
    special_constraint_refs = frozenset(
        ref for ref in constraint_refs if ref.family != "regular"
    )
    violations: list[AdapterPreconditionViolation] = []
    uniform_penalty_weight = config.uniform_penalty_weight
    penalty_weights = config.penalty_weights

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
    return tuple(violations)


def check_preparation_source(
    ommx_instance: Instance,
    *,
    config: OpenJijPreparationConfig,
) -> OpenJijPreparationSourceCheck:
    membership, preconditions_checked, violations = _check_class_preconditions(
        ommx_instance,
        PREPARATION_SOURCE_CLASS,
        lambda: (
            *_log_encoding_precondition_violations(ommx_instance),
            *_penalty_precondition_violations(ommx_instance, config),
        ),
    )
    return OpenJijPreparationSourceCheck(
        source_membership=membership,
        preconditions_checked=preconditions_checked,
        precondition_violations=violations,
    )


def check_encoding_input(
    ommx_instance: Instance,
) -> tuple[
    InstanceClassMembershipReport,
    bool,
    tuple[AdapterPreconditionViolation, ...],
]:
    return _check_class_preconditions(
        ommx_instance,
        ENCODING_INPUT_CLASS,
        lambda: _log_encoding_precondition_violations(ommx_instance),
    )
