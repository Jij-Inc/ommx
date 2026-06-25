from __future__ import annotations

from collections import Counter
from collections.abc import Mapping
from typing import Any

from .graph import quantize_float
from .types import Colors, FactorKey, InstanceGraph, SearchState, Terms
from .verify import (
    map_monomial,
    output_factor_keys,
    source_is_fully_mapped,
    terms_close,
    verify_mapping,
)


def refine_colors(
    source: InstanceGraph,
    target: InstanceGraph,
    *,
    atol: float,
    max_iterations: int,
) -> Colors:
    source_variable_colors, target_variable_colors = _intern_pair(
        source.variables, target.variables
    )
    source_factor_colors, target_factor_colors = _intern_pair(
        {k: f.attrs for k, f in source.factors.items()},
        {k: f.attrs for k, f in target.factors.items()},
    )

    for _ in range(max_iterations):
        next_source_factor_raw = _factor_color_inputs(
            source, source_variable_colors, atol=atol
        )
        next_target_factor_raw = _factor_color_inputs(
            target, target_variable_colors, atol=atol
        )
        next_source_factor_colors, next_target_factor_colors = _intern_pair(
            next_source_factor_raw, next_target_factor_raw
        )

        next_source_variable_raw = _variable_color_inputs(
            source, next_source_factor_colors
        )
        next_target_variable_raw = _variable_color_inputs(
            target, next_target_factor_colors
        )
        next_source_variable_colors, next_target_variable_colors = _intern_pair(
            next_source_variable_raw, next_target_variable_raw
        )

        if (
            next_source_variable_colors == source_variable_colors
            and next_target_variable_colors == target_variable_colors
            and next_source_factor_colors == source_factor_colors
            and next_target_factor_colors == target_factor_colors
        ):
            break

        source_variable_colors = next_source_variable_colors
        target_variable_colors = next_target_variable_colors
        source_factor_colors = next_source_factor_colors
        target_factor_colors = next_target_factor_colors

    return Colors(
        source_variables=source_variable_colors,
        target_variables=target_variable_colors,
        source_factors=source_factor_colors,
        target_factors=target_factor_colors,
    )


def search_mapping(
    source: InstanceGraph,
    target: InstanceGraph,
    colors: Colors,
    *,
    max_steps: int,
    atol: float,
) -> tuple[SearchState, int]:
    state = SearchState(
        variable_map={},
        inverse_variable_map={},
        factor_map={("objective", -1): ("objective", -1)},
        inverse_factor_map={("objective", -1): ("objective", -1)},
    )
    best = state.copy()
    steps = 0

    variable_candidates = _variable_candidates_by_color(source, target, colors)
    factor_candidates = _factor_candidates_by_color(source, target, colors)

    def remember(candidate: SearchState) -> None:
        nonlocal best
        if mapped_size(candidate) > mapped_size(best):
            best = candidate.copy()

    def recurse(current: SearchState) -> SearchState | None:
        nonlocal steps
        steps += 1
        remember(current)
        if steps > max_steps:
            return None
        if source_is_fully_mapped(source, current):
            if verify_mapping(source, target, current, atol=atol):
                return current
            return None

        item = _choose_next_item(
            source,
            target,
            current,
            variable_candidates,
            factor_candidates,
            atol=atol,
        )
        if item is None:
            return None
        item_kind, source_key, candidates = item
        if not candidates:
            return None

        for target_key in candidates:
            next_state = current.copy()
            if item_kind == "variable":
                source_variable = int(source_key)
                target_variable = int(target_key)
                next_state.variable_map[source_variable] = target_variable
                next_state.inverse_variable_map[target_variable] = source_variable
            else:
                source_factor = source_key
                target_factor = target_key
                next_state.factor_map[source_factor] = target_factor
                next_state.inverse_factor_map[target_factor] = source_factor
            result = recurse(next_state)
            if result is not None:
                return result
        return None

    result = recurse(state)
    return (result or best, min(steps, max_steps))


def ambiguous_variables(
    source: InstanceGraph,
    target: InstanceGraph,
    colors: Colors,
    state: SearchState,
) -> dict[int, tuple[int, ...]]:
    by_color: dict[int, list[int]] = {}
    for target_variable, color in colors.target_variables.items():
        by_color.setdefault(color, []).append(target_variable)
    out = {}
    for source_variable, color in colors.source_variables.items():
        if source_variable in state.variable_map:
            continue
        candidates = tuple(sorted(by_color.get(color, ())))
        if len(candidates) != 1:
            out[source_variable] = candidates
    return out


def ambiguous_factors(
    source: InstanceGraph,
    target: InstanceGraph,
    colors: Colors,
    state: SearchState,
    kind: str,
) -> dict[int, tuple[int, ...]]:
    by_color: dict[int, list[int]] = {}
    for target_key, color in colors.target_factors.items():
        if target_key[0] == kind:
            by_color.setdefault(color, []).append(target_key[1])
    out = {}
    for source_key, color in colors.source_factors.items():
        if source_key[0] != kind or source_key in state.factor_map:
            continue
        candidates = tuple(sorted(by_color.get(color, ())))
        if len(candidates) != 1:
            out[source_key[1]] = candidates
    return out


def mapped_size(state: SearchState) -> int:
    return len(state.variable_map) + sum(
        1 for factor_key in state.factor_map if factor_key[0] != "objective"
    )


def _colored_terms(
    terms: Terms,
    variable_colors: Mapping[int, int],
    atol: float,
) -> tuple[Any, ...]:
    items = []
    for monomial, coefficient in terms.items():
        color_counts = tuple(
            sorted(Counter(variable_colors[v] for v in monomial).items())
        )
        items.append((color_counts, quantize_float(coefficient, atol)))
    return tuple(sorted(items, key=repr))


def _factor_color_inputs(
    graph: InstanceGraph,
    variable_colors: Mapping[int, int],
    *,
    atol: float,
) -> dict[FactorKey, tuple[Any, ...]]:
    out = {}
    for key, factor in graph.factors.items():
        neighborhood = tuple(
            sorted(
                (
                    factor.neighbors[variable_id],
                    variable_colors.get(variable_id),
                )
                for variable_id in factor.neighbors
            )
        )
        out[key] = (
            factor.attrs,
            _colored_terms(factor.terms, variable_colors, atol),
            neighborhood,
        )
    return out


def _variable_color_inputs(
    graph: InstanceGraph,
    factor_colors: Mapping[FactorKey, int],
) -> dict[int, tuple[Any, ...]]:
    out = {}
    for variable_id, attrs in graph.variables.items():
        neighborhood = tuple(
            sorted(
                (edge_label, factor_colors[factor_key])
                for factor_key, edge_label in graph.var_to_factors.get(
                    variable_id, {}
                ).items()
            )
        )
        out[variable_id] = (attrs, neighborhood)
    return out


def _intern_pair(
    source_values: Mapping[Any, Any],
    target_values: Mapping[Any, Any],
) -> tuple[dict[Any, int], dict[Any, int]]:
    unique_values = {_freeze(value) for value in source_values.values()} | {
        _freeze(value) for value in target_values.values()
    }
    interned = {
        value: index for index, value in enumerate(sorted(unique_values, key=repr))
    }
    return (
        {key: interned[_freeze(value)] for key, value in source_values.items()},
        {key: interned[_freeze(value)] for key, value in target_values.items()},
    )


def _freeze(value: Any) -> Any:
    if isinstance(value, dict):
        return tuple(sorted((_freeze(k), _freeze(v)) for k, v in value.items()))
    if isinstance(value, (list, tuple)):
        return tuple(_freeze(v) for v in value)
    if isinstance(value, set):
        return tuple(sorted((_freeze(v) for v in value), key=repr))
    return value


def _variable_candidates_by_color(
    source: InstanceGraph, target: InstanceGraph, colors: Colors
) -> dict[int, tuple[int, ...]]:
    by_color: dict[int, list[int]] = {}
    for target_variable, color in colors.target_variables.items():
        by_color.setdefault(color, []).append(target_variable)
    return {
        source_variable: tuple(sorted(by_color.get(color, ())))
        for source_variable, color in colors.source_variables.items()
    }


def _factor_candidates_by_color(
    source: InstanceGraph, target: InstanceGraph, colors: Colors
) -> dict[FactorKey, tuple[FactorKey, ...]]:
    by_color: dict[tuple[str, int], list[FactorKey]] = {}
    for target_key, color in colors.target_factors.items():
        by_color.setdefault((target_key[0], color), []).append(target_key)
    return {
        source_key: tuple(sorted(by_color.get((source_key[0], color), ())))
        for source_key, color in colors.source_factors.items()
        if source_key[0] != "objective"
    }


def _choose_next_item(
    source: InstanceGraph,
    target: InstanceGraph,
    state: SearchState,
    variable_candidates: Mapping[int, tuple[int, ...]],
    factor_candidates: Mapping[FactorKey, tuple[FactorKey, ...]],
    *,
    atol: float,
) -> tuple[str, Any, tuple[Any, ...]] | None:
    choices: list[tuple[tuple[int, int, int], str, Any, tuple[Any, ...]]] = []

    for source_variable in source.variables:
        if source_variable in state.variable_map:
            continue
        candidates = tuple(
            target_variable
            for target_variable in variable_candidates.get(source_variable, ())
            if target_variable not in state.inverse_variable_map
            and _variable_pair_consistent(
                source, target, state, source_variable, target_variable, atol=atol
            )
        )
        degree = len(source.var_to_factors.get(source_variable, {}))
        choices.append(
            (
                (len(candidates), -degree, source_variable),
                "variable",
                source_variable,
                candidates,
            )
        )

    for source_factor in output_factor_keys(source):
        if source_factor in state.factor_map:
            continue
        candidates = tuple(
            target_factor
            for target_factor in factor_candidates.get(source_factor, ())
            if target_factor not in state.inverse_factor_map
            and _factor_pair_consistent(
                source, target, state, source_factor, target_factor, atol=atol
            )
        )
        edge_count = len(source.factors[source_factor].neighbors)
        choices.append(
            (
                (len(candidates), -edge_count, source_factor[1]),
                "factor",
                source_factor,
                candidates,
            )
        )

    if not choices:
        return None
    _, kind, source_key, candidates = min(choices, key=lambda item: item[0])
    return kind, source_key, candidates


def _variable_pair_consistent(
    source: InstanceGraph,
    target: InstanceGraph,
    state: SearchState,
    source_variable: int,
    target_variable: int,
    *,
    atol: float,
) -> bool:
    if source.variables[source_variable] != target.variables[target_variable]:
        return False
    trial = state.copy()
    trial.variable_map[source_variable] = target_variable
    trial.inverse_variable_map[target_variable] = source_variable
    for source_factor, target_factor in trial.factor_map.items():
        if not _mapped_factor_edges_match(
            source, target, trial, source_factor, target_factor, atol=atol
        ):
            return False
    return True


def _factor_pair_consistent(
    source: InstanceGraph,
    target: InstanceGraph,
    state: SearchState,
    source_factor: FactorKey,
    target_factor: FactorKey,
    *,
    atol: float,
) -> bool:
    if (
        source.factors[source_factor].attrs[:3]
        != target.factors[target_factor].attrs[:3]
    ):
        return False
    trial = state.copy()
    trial.factor_map[source_factor] = target_factor
    trial.inverse_factor_map[target_factor] = source_factor
    return _mapped_factor_edges_match(
        source, target, trial, source_factor, target_factor, atol=atol
    )


def _mapped_factor_edges_match(
    source: InstanceGraph,
    target: InstanceGraph,
    state: SearchState,
    source_factor: FactorKey,
    target_factor: FactorKey,
    *,
    atol: float,
) -> bool:
    source_node = source.factors[source_factor]
    target_node = target.factors[target_factor]
    for source_variable, target_variable in state.variable_map.items():
        source_edge = source_node.neighbors.get(source_variable)
        target_edge = target_node.neighbors.get(target_variable)
        if source_edge != target_edge:
            return False

    if source_node.indicator_variable is not None:
        mapped_indicator = state.variable_map.get(source_node.indicator_variable)
        if (
            mapped_indicator is not None
            and mapped_indicator != target_node.indicator_variable
        ):
            return False

    return _partial_terms_match(
        source_node.terms,
        target_node.terms,
        state.variable_map,
        state.inverse_variable_map,
        atol=atol,
    )


def _partial_terms_match(
    source_terms: Terms,
    target_terms: Terms,
    variable_map: Mapping[int, int],
    inverse_variable_map: Mapping[int, int],
    *,
    atol: float,
) -> bool:
    source_known = {
        map_monomial(monomial, variable_map): coefficient
        for monomial, coefficient in source_terms.items()
        if all(variable_id in variable_map for variable_id in monomial)
    }
    target_known = {
        monomial: coefficient
        for monomial, coefficient in target_terms.items()
        if all(variable_id in inverse_variable_map for variable_id in monomial)
    }
    return terms_close(source_known, target_known, atol=atol)
