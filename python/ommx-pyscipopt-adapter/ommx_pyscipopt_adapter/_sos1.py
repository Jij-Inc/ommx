"""Checked, adapter-local replacement of legacy SOS1 formulations."""

from __future__ import annotations

from collections import Counter
from dataclasses import dataclass
from fractions import Fraction
import math

from ommx import get_default_atol
from ommx.v1 import Constraint, DecisionVariable, Instance, Sos1

from .exception import OMMXPySCIPOptAdapterError


def _satisfied(row: Constraint, entries: dict[int, float], atol: float) -> bool:
    value = row.function.evaluate(entries)
    # V2's EvaluatedConstraint uses a strict comparison with ATol.
    return math.isfinite(value) and value < atol


@dataclass(frozen=True)
class _Selector:
    member: int
    lower: float
    links: tuple[Constraint, ...]


@dataclass(frozen=True)
class Sos1Plan:
    name: str
    members: tuple[int, ...]
    selectors: dict[int, _Selector]
    bounds: dict[int, tuple[float, float]]
    rows: tuple[Constraint, ...]

    def restore(self, entries: dict[int, float]) -> None:
        atol = get_default_atol()
        for selector, claim in self.selectors.items():
            value = entries[claim.member]
            if not math.isfinite(value):
                raise OMMXPySCIPOptAdapterError(
                    f"Cannot restore SOS1 selector {selector}: member {claim.member} is not finite."
                )
            # A solver may return several tiny SOS1 members. Test the original
            # link residuals, rather than applying an arbitrary epsilon to x:
            # row scaling matters to OMMX feasibility. Do not modify members.
            zero_state = {claim.member: value, selector: 0.0}
            can_be_zero = claim.lower <= 0 and all(
                _satisfied(row, zero_state, atol) for row in claim.links
            )
            entries[selector] = 0.0 if can_be_zero else 1.0

        if not all(_satisfied(row, entries, atol) for row in self.rows):
            raise OMMXPySCIPOptAdapterError(
                f"SCIP's solution cannot satisfy the original rows of {self.name} "
                "after selector restoration. Tighten SCIP's feasibility tolerance "
                "or solve with use_sos1='disabled'."
            )


def plan_sos1(instance: Instance, mode: str) -> list[Sos1Plan]:
    if mode == "disabled":
        return []
    hints = instance.constraint_hints.sos1_constraints
    if mode == "forced" and not hints:
        raise OMMXPySCIPOptAdapterError(
            "No SOS1 constraints were found, but `use_sos1` is set to `forced`."
        )

    variables = {var.id: var for var in instance.used_decision_variables}
    constraints = {row.id: row for row in instance.constraints}
    analysis = instance.decision_variable_analysis()
    objective_ids = analysis.used_in_objective()
    row_usage = analysis.used_in_constraints()
    claimed_rows = Counter(
        id
        for hint in hints
        for id in [hint.binary_constraint_id, *hint.big_m_constraint_ids]
    )
    plans = []
    for hint in hints:
        try:
            row_ids = {hint.binary_constraint_id, *hint.big_m_constraint_ids}
            if any(claimed_rows[id] != 1 for id in row_ids):
                raise ValueError("a backing row is claimed more than once")
            outside = set(objective_ids)
            for id, used in row_usage.items():
                if id not in row_ids:
                    outside.update(used)
            # Fixed and dependent variables are absent from `variables`.
            plans.append(_plan_hint(hint, variables, constraints, outside))
        except ValueError as error:
            if mode == "forced":
                raise OMMXPySCIPOptAdapterError(
                    f"Cannot replace SOS1 hint {hint.binary_constraint_id}: {error}."
                ) from error
            # Hints are advisory: retain every original row for this candidate.
    return plans


def _plan_hint(
    hint: Sos1,
    variables: dict[int, DecisionVariable],
    constraints: dict[int, Constraint],
    outside: set[int],
) -> Sos1Plan:
    members = set(hint.variables)
    row_ids = {hint.binary_constraint_id, *hint.big_m_constraint_ids}
    if not members or not members <= variables.keys():
        raise ValueError("members must be nonempty, active, independent variables")
    if not row_ids <= constraints.keys():
        raise ValueError("backing rows must be active constraints")
    cardinality = constraints[hint.binary_constraint_id]
    f = cardinality.function
    if (
        cardinality.equality != Constraint.LESS_THAN_OR_EQUAL_TO_ZERO
        or f.degree() != 1
        or f.constant_term != -1
        or any(coefficient != 1 for coefficient in f.linear_terms.values())
    ):
        raise ValueError("cardinality must be the sum of selectors <= 1")
    cardinality_ids = set(f.linear_terms)
    if not cardinality_ids <= variables.keys():
        raise ValueError("selectors must be active, independent variables")
    if any(variables[id].kind != DecisionVariable.BINARY for id in cardinality_ids):
        raise ValueError("cardinality selectors must be binary")

    # The wire hint has unordered row IDs, not positional member/selector pairs.
    links: dict[int, dict[str, tuple[int, Fraction]]] = {}
    member_selectors: dict[int, int] = {}
    for id in hint.big_m_constraint_ids:
        row = constraints[id]
        f = row.function
        terms = f.linear_terms
        linked_members = members & terms.keys()
        selectors = terms.keys() - members
        if (
            row.equality != Constraint.LESS_THAN_OR_EQUAL_TO_ZERO
            or f.degree() != 1
            or f.constant_term != 0
            or len(terms) != 2
            or len(linked_members) != 1
            or len(selectors) != 1
        ):
            raise ValueError(f"link {id} must contain one member and one selector")
        member = next(iter(linked_members))
        selector = next(iter(selectors))
        if selector not in cardinality_ids:
            raise ValueError(f"link {id} uses a selector absent from cardinality")
        if variables[member].kind == DecisionVariable.BINARY:
            raise ValueError("binary members must be reused as their own selectors")
        a, b = terms[member], terms[selector]
        if not math.isfinite(a) or not math.isfinite(b) or a == 0 or b >= 0:
            raise ValueError(f"link {id} has invalid coefficient signs or values")
        if member in member_selectors and member_selectors[member] != selector:
            raise ValueError(f"member {member} has conflicting selectors")
        member_selectors[member] = selector
        side = "upper" if a > 0 else "lower"
        sides = links.setdefault(member, {})
        if side in sides:
            raise ValueError(f"member {member} has duplicate {side} links")
        sides[side] = (id, -Fraction(b) / abs(Fraction(a)))

    reused = {id for id in members if variables[id].kind == DecisionVariable.BINARY}
    if not reused <= cardinality_ids:
        raise ValueError("binary members must appear in cardinality")
    unresolved_members = members - reused - member_selectors.keys()
    unresolved_selectors = cardinality_ids - reused - set(member_selectors.values())
    if len(unresolved_members) == len(unresolved_selectors) == 1:
        member_selectors[unresolved_members.pop()] = unresolved_selectors.pop()
    elif unresolved_members or unresolved_selectors:
        raise ValueError("member/selector correspondence is incomplete or ambiguous")
    if len(set(member_selectors.values())) != len(member_selectors):
        raise ValueError("a private selector is shared by multiple members")
    if set(member_selectors.values()) & outside:
        raise ValueError("a private selector is used outside its formulation")

    bounds: dict[int, tuple[float, float]] = {}
    restoration = {}
    for member, selector in member_selectors.items():
        var, binary = variables[member], variables[selector]
        if var.kind not in (DecisionVariable.CONTINUOUS, DecisionVariable.INTEGER):
            raise ValueError(f"member {member} has an unsupported variable kind")
        lower, upper = var.bound.lower, var.bound.upper
        sides = links.get(member, {})
        for side, (_, big_m) in sides.items():
            # Preserve restrictions imposed by removed links in SCIP bounds.
            # Outward rounding avoids silently shrinking the mathematical model;
            # the exact coverage check below rejects unrepresentable endpoints.
            if var.kind == DecisionVariable.INTEGER:
                big_m = Fraction(math.floor(big_m))
            try:
                endpoint = float(big_m)
            except OverflowError:
                raise ValueError(f"member {member} has an unrepresentable link bound")
            if not math.isfinite(endpoint):
                raise ValueError(f"member {member} has a non-finite link bound")
            if Fraction(endpoint) < big_m:
                endpoint = math.nextafter(endpoint, math.inf)
            if side == "upper":
                upper = min(upper, endpoint)
            else:
                lower = max(lower, -endpoint)
        if var.kind == DecisionVariable.INTEGER:
            if math.isfinite(lower):
                lower = float(math.ceil(lower))
            if math.isfinite(upper):
                upper = float(math.floor(upper))
        if not math.isfinite(lower) or not math.isfinite(upper) or lower > upper:
            raise ValueError(f"member {member} has an empty or unbounded linked domain")
        for side, endpoint in [("upper", upper), ("lower", -lower)]:
            if endpoint > 0 and side not in sides:
                raise ValueError(f"member {member} is missing its {side} link")
            if side in sides and Fraction(endpoint) > sides[side][1]:
                raise ValueError(
                    f"member {member}'s {side} link does not cover its bound"
                )
        for id, _ in sides.values():
            terms = constraints[id].function.linear_terms
            if any(
                not math.isfinite(terms[member] * value + terms[selector] * selected)
                for value in (lower, upper)
                for selected in (0, 1)
            ):
                raise ValueError(
                    f"link {id} has a non-finite residual over the member bound"
                )
        can_be_zero = lower <= 0 <= upper
        can_be_nonzero = lower < 0 or upper > 0
        if (can_be_zero and binary.bound.lower > 0) or (
            can_be_nonzero and binary.bound.upper < 1
        ):
            raise ValueError(
                f"selector {selector}'s bound excludes its reconstructed value"
            )
        bounds[member] = (lower, upper)
        restoration[selector] = _Selector(
            member,
            binary.bound.lower,
            tuple(constraints[id] for id, _ in sides.values()),
        )

    suffix = "_".join(map(str, sorted(hint.big_m_constraint_ids)))
    name = f"sos1_{hint.binary_constraint_id}" + (f"_{suffix}" if suffix else "")
    return Sos1Plan(
        name,
        tuple(sorted(members)),
        restoration,
        bounds,
        tuple(constraints[id] for id in sorted(row_ids)),
    )
