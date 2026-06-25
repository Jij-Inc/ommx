from __future__ import annotations

from collections.abc import Mapping

from .types import FactorKey, FactorNode, InstanceGraph, Monomial, SearchState, Terms


def verify_mapping(
    source: InstanceGraph,
    target: InstanceGraph,
    state: SearchState,
    *,
    atol: float,
) -> bool:
    if len(source.variables) != len(target.variables):
        return False
    if len(state.variable_map) != len(source.variables):
        return False
    if source.sense != target.sense:
        return False
    if len(output_factor_keys(source)) != len(output_factor_keys(target)):
        return False
    if len(state.factor_map) != len(source.factors):
        return False
    if not objective_matches(source, target, state, atol=atol):
        return False

    for source_factor, target_factor in state.factor_map.items():
        if source_factor[0] == "objective":
            continue
        if source_factor[0] != target_factor[0]:
            return False
        if not factor_matches(
            source.factors[source_factor],
            target.factors[target_factor],
            state.variable_map,
            atol=atol,
        ):
            return False
    return True


def objective_matches(
    source: InstanceGraph,
    target: InstanceGraph,
    state: SearchState,
    *,
    atol: float,
) -> bool:
    if len(state.variable_map) != len(source.variables):
        return False
    if source.sense != target.sense:
        return False
    return function_matches(
        source.factors[("objective", -1)].terms,
        target.factors[("objective", -1)].terms,
        state.variable_map,
        atol=atol,
    )


def factor_matches(
    source: FactorNode,
    target: FactorNode,
    variable_map: Mapping[int, int],
    *,
    atol: float,
) -> bool:
    if source.attrs[:3] != target.attrs[:3]:
        return False
    if source.indicator_variable is not None:
        if variable_map.get(source.indicator_variable) != target.indicator_variable:
            return False
    if source.variables:
        return (
            tuple(sorted(variable_map[v] for v in source.variables)) == target.variables
        )
    return function_matches(source.terms, target.terms, variable_map, atol=atol)


def function_matches(
    source_terms: Terms,
    target_terms: Terms,
    variable_map: Mapping[int, int],
    *,
    atol: float,
) -> bool:
    if any(
        variable_id not in variable_map
        for monomial in source_terms
        for variable_id in monomial
    ):
        return False
    mapped_terms: dict[Monomial, float] = {}
    for monomial, coefficient in source_terms.items():
        mapped_monomial = map_monomial(monomial, variable_map)
        mapped_terms[mapped_monomial] = (
            mapped_terms.get(mapped_monomial, 0.0) + coefficient
        )
    return terms_close(mapped_terms, target_terms, atol=atol)


def map_monomial(monomial: Monomial, variable_map: Mapping[int, int]) -> Monomial:
    return tuple(sorted(variable_map[variable_id] for variable_id in monomial))


def terms_close(source_terms: Terms, target_terms: Terms, *, atol: float) -> bool:
    keys = set(source_terms) | set(target_terms)
    for key in keys:
        if abs(source_terms.get(key, 0.0) - target_terms.get(key, 0.0)) > atol:
            return False
    return True


def source_is_fully_mapped(source: InstanceGraph, state: SearchState) -> bool:
    return len(state.variable_map) == len(source.variables) and len(
        state.factor_map
    ) == len(source.factors)


def output_factor_keys(graph: InstanceGraph) -> tuple[FactorKey, ...]:
    return tuple(
        sorted((key for key in graph.factors if key[0] != "objective"), key=repr)
    )
