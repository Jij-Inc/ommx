from __future__ import annotations

from collections import Counter
import math
from typing import Any

from ommx.v1 import Function, Instance

from .types import FactorKey, FactorNode, InstanceGraph, Monomial, Terms


def build_graph(
    instance: Instance, *, atol: float, color_atol: float, include_metadata: bool
) -> InstanceGraph:
    variables: dict[int, tuple[Any, ...]] = {}
    for attached in instance.decision_variables:
        var = attached.detach()
        variables[var.id] = _variable_attrs(
            var, atol=color_atol, include_metadata=include_metadata
        )

    factors: dict[FactorKey, FactorNode] = {}
    objective_terms = function_terms(instance.objective, atol=atol)
    factors[("objective", -1)] = _function_factor(
        key=("objective", -1),
        kind="objective",
        function_terms=objective_terms,
        equality=None,
        metadata=(),
        atol=color_atol,
        sense=str(instance.sense),
    )

    for constraint_id, attached in instance.constraints.items():
        metadata = _constraint_metadata(attached, include_metadata)
        factors[("constraint", int(constraint_id))] = _function_factor(
            key=("constraint", int(constraint_id)),
            kind="constraint",
            function_terms=function_terms(attached.function, atol=atol),
            equality=str(attached.equality),
            metadata=metadata,
            atol=color_atol,
        )

    for constraint_id, attached in instance.indicator_constraints.items():
        metadata = _constraint_metadata(attached, include_metadata)
        terms = function_terms(attached.function, atol=atol)
        neighbors = dict(function_neighbors(terms, atol=color_atol))
        indicator_id = int(attached.indicator_variable_id)
        labels = list(neighbors.get(indicator_id, ()))
        labels.append(("indicator",))
        neighbors[indicator_id] = tuple(sorted(labels, key=repr))
        attrs = (
            "indicator",
            str(attached.equality),
            function_shape_signature(terms, color_atol),
            metadata,
        )
        factors[("indicator", int(constraint_id))] = FactorNode(
            key=("indicator", int(constraint_id)),
            attrs=attrs,
            terms=terms,
            neighbors=neighbors,
            indicator_variable=indicator_id,
        )

    for constraint_id, attached in instance.one_hot_constraints.items():
        factors[("one_hot", int(constraint_id))] = _membership_factor(
            key=("one_hot", int(constraint_id)),
            kind="one_hot",
            variables=tuple(int(v) for v in attached.variables),
            metadata=_constraint_metadata(attached, include_metadata),
        )

    for constraint_id, attached in instance.sos1_constraints.items():
        factors[("sos1", int(constraint_id))] = _membership_factor(
            key=("sos1", int(constraint_id)),
            kind="sos1",
            variables=tuple(int(v) for v in attached.variables),
            metadata=_constraint_metadata(attached, include_metadata),
        )

    var_to_factors: dict[int, dict[FactorKey, tuple[Any, ...]]] = {
        variable_id: {} for variable_id in variables
    }
    for key, factor in factors.items():
        for variable_id, edge_label in factor.neighbors.items():
            var_to_factors.setdefault(variable_id, {})[key] = edge_label

    return InstanceGraph(
        variables=variables,
        factors=factors,
        var_to_factors=var_to_factors,
        sense=str(instance.sense),
    )


def resolve_color_atol(atol: float, color_atol: float | None) -> float:
    if atol < 0:
        raise ValueError("atol must be non-negative")
    if color_atol is None:
        return 10 * atol
    if color_atol < 0:
        raise ValueError("color_atol must be non-negative")
    if color_atol < atol:
        raise ValueError("color_atol must be greater than or equal to atol")
    return color_atol


def function_terms(function: Function, *, atol: float) -> Terms:
    terms: dict[Monomial, float] = {}
    for raw_monomial, raw_coefficient in function.terms.items():
        coefficient = float(raw_coefficient)
        if abs(coefficient) <= atol:
            continue
        monomial = tuple(sorted(int(v) for v in raw_monomial))
        terms[monomial] = terms.get(monomial, 0.0) + coefficient
    return {
        monomial: coefficient
        for monomial, coefficient in terms.items()
        if abs(coefficient) > atol
    }


def function_neighbors(terms: Terms, *, atol: float) -> dict[int, tuple[Any, ...]]:
    labels: dict[int, list[tuple[Any, ...]]] = {}
    for monomial, coefficient in terms.items():
        powers = Counter(monomial)
        power_shape = tuple(sorted(powers.values()))
        for variable_id, power in powers.items():
            labels.setdefault(variable_id, []).append(
                (
                    "term",
                    len(monomial),
                    int(power),
                    power_shape,
                    quantize_float(coefficient, atol),
                )
            )
    return {
        variable_id: tuple(sorted(edge_labels, key=repr))
        for variable_id, edge_labels in labels.items()
    }


def function_shape_signature(terms: Terms, atol: float) -> tuple[Any, ...]:
    items = []
    for monomial, coefficient in terms.items():
        power_shape = tuple(sorted(Counter(monomial).values()))
        items.append((len(monomial), power_shape, quantize_float(coefficient, atol)))
    return tuple(sorted(items, key=repr))


def quantize_float(value: float, atol: float) -> tuple[str, int]:
    if math.isnan(value):
        return ("nan", 0)
    if math.isinf(value):
        return ("inf", 1 if value > 0 else -1)
    if atol <= 0:
        return ("exact", hash(repr(value)))
    return ("q", int(round(value / atol)))


def _variable_attrs(
    var: Any, *, atol: float, include_metadata: bool
) -> tuple[Any, ...]:
    attrs: tuple[Any, ...] = (
        "variable",
        int(var.kind),
        quantize_float(float(var.bound.lower), atol),
        quantize_float(float(var.bound.upper), atol),
        _quantize_optional_float(var.substituted_value, atol),
    )
    if include_metadata:
        attrs += (
            str(var.name),
            tuple(int(s) for s in var.subscripts),
            tuple(sorted((str(k), str(v)) for k, v in var.parameters.items())),
            str(var.description),
        )
    return attrs


def _constraint_metadata(attached: Any, include_metadata: bool) -> tuple[Any, ...]:
    if not include_metadata:
        return ()
    return (
        str(getattr(attached, "name", None)),
        tuple(int(s) for s in getattr(attached, "subscripts", ())),
        tuple(
            sorted(
                (str(k), str(v)) for k, v in getattr(attached, "parameters", {}).items()
            )
        ),
        str(getattr(attached, "description", None)),
    )


def _function_factor(
    *,
    key: FactorKey,
    kind: str,
    function_terms: Terms,
    equality: str | None,
    metadata: tuple[Any, ...],
    atol: float,
    sense: str | None = None,
) -> FactorNode:
    attrs = (
        kind,
        sense,
        equality,
        function_shape_signature(function_terms, atol),
        metadata,
    )
    return FactorNode(
        key=key,
        attrs=attrs,
        terms=function_terms,
        neighbors=function_neighbors(function_terms, atol=atol),
    )


def _membership_factor(
    *,
    key: FactorKey,
    kind: str,
    variables: tuple[int, ...],
    metadata: tuple[Any, ...],
) -> FactorNode:
    neighbors = {variable_id: (("member",),) for variable_id in variables}
    attrs = (kind, len(variables), metadata)
    return FactorNode(
        key=key,
        attrs=attrs,
        terms={},
        neighbors=neighbors,
        variables=tuple(sorted(variables)),
    )


def _quantize_optional_float(
    value: float | None, atol: float
) -> tuple[str, int] | None:
    if value is None:
        return None
    return quantize_float(float(value), atol)
