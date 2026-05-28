from __future__ import annotations

from collections import Counter
from collections.abc import Mapping
from dataclasses import dataclass
import math
from typing import Any

from ommx.v1 import Function, Instance

_FactorKey = tuple[str, int]
_Monomial = tuple[int, ...]
_Terms = dict[_Monomial, float]


@dataclass(frozen=True)
class InstanceMapping:
    """Heuristic ID mapping between two structurally similar instances.

    ``decision_variables`` maps source decision variable IDs to target IDs.
    Constraint mappings are split by OMMX constraint collection.
    """

    decision_variables: Mapping[int, int]
    constraints: Mapping[int, int]
    indicator_constraints: Mapping[int, int]
    one_hot_constraints: Mapping[int, int]
    sos1_constraints: Mapping[int, int]
    objective_matches: bool
    verified: bool
    score: float
    backtracking_steps: int
    ambiguous_decision_variables: Mapping[int, tuple[int, ...]]
    ambiguous_constraints: Mapping[int, tuple[int, ...]]
    diagnostics: tuple[str, ...] = ()

    @property
    def is_complete(self) -> bool:
        return self.verified


@dataclass(frozen=True)
class _FactorNode:
    key: _FactorKey
    attrs: tuple[Any, ...]
    terms: _Terms
    neighbors: Mapping[int, tuple[Any, ...]]
    variables: tuple[int, ...] = ()
    indicator_variable: int | None = None


@dataclass(frozen=True)
class _Graph:
    variables: Mapping[int, tuple[Any, ...]]
    factors: Mapping[_FactorKey, _FactorNode]
    var_to_factors: Mapping[int, Mapping[_FactorKey, tuple[Any, ...]]]
    sense: str


@dataclass
class _Colors:
    source_variables: dict[int, int]
    target_variables: dict[int, int]
    source_factors: dict[_FactorKey, int]
    target_factors: dict[_FactorKey, int]


@dataclass
class _SearchState:
    variable_map: dict[int, int]
    inverse_variable_map: dict[int, int]
    factor_map: dict[_FactorKey, _FactorKey]
    inverse_factor_map: dict[_FactorKey, _FactorKey]

    def copy(self) -> "_SearchState":
        return _SearchState(
            variable_map=dict(self.variable_map),
            inverse_variable_map=dict(self.inverse_variable_map),
            factor_map=dict(self.factor_map),
            inverse_factor_map=dict(self.inverse_factor_map),
        )


def match_instance_ids(
    source: Instance,
    target: Instance,
    *,
    atol: float = 1e-12,
    color_atol: float | None = None,
    max_refinement_iterations: int = 12,
    max_backtracking_steps: int = 100_000,
    include_metadata: bool = False,
) -> InstanceMapping:
    """Infer a source-to-target ID mapping between two OMMX instances.

    This is an experimental heuristic. It builds a factor graph whose nodes are
    decision variables and objective/constraint factors, refines node colors by
    neighborhood signatures, then runs a bounded backtracking search over the
    remaining candidates. The returned mapping is marked ``verified`` only when
    all active factors and the objective match after applying the inferred
    decision-variable mapping.

    Args:
        source: Instance whose IDs are mapped from.
        target: Instance whose IDs are mapped to.
        atol: Absolute tolerance used when grouping coefficients and bounds.
        color_atol: Coarser absolute tolerance used only for color/fingerprint
            bucketing. The final verification still uses ``atol``.
        max_refinement_iterations: Maximum color-refinement rounds.
        max_backtracking_steps: Search budget for ambiguous color classes.
        include_metadata: Include names, subscripts, and parameters in
            fingerprints. The default ignores metadata and matches math shape.
    """

    effective_color_atol = _resolve_color_atol(atol, color_atol)
    source_graph = _build_graph(
        source,
        atol=atol,
        color_atol=effective_color_atol,
        include_metadata=include_metadata,
    )
    target_graph = _build_graph(
        target,
        atol=atol,
        color_atol=effective_color_atol,
        include_metadata=include_metadata,
    )
    colors = _refine_colors(
        source_graph,
        target_graph,
        atol=effective_color_atol,
        max_iterations=max_refinement_iterations,
    )
    result, steps = _search_mapping(
        source_graph,
        target_graph,
        colors,
        max_steps=max_backtracking_steps,
        atol=atol,
    )
    verified = _verify_mapping(source_graph, target_graph, result, atol=atol)
    objective_matches = _objective_matches(
        source_graph, target_graph, result, atol=atol
    )
    diagnostics = _diagnostics(source_graph, target_graph, result, verified, steps)

    return InstanceMapping(
        decision_variables=dict(sorted(result.variable_map.items())),
        constraints=_extract_factor_mapping(result, "constraint"),
        indicator_constraints=_extract_factor_mapping(result, "indicator"),
        one_hot_constraints=_extract_factor_mapping(result, "one_hot"),
        sos1_constraints=_extract_factor_mapping(result, "sos1"),
        objective_matches=objective_matches,
        verified=verified,
        score=_score(source_graph, result),
        backtracking_steps=steps,
        ambiguous_decision_variables=_ambiguous_variables(
            source_graph, target_graph, colors, result
        ),
        ambiguous_constraints=_ambiguous_factors(
            source_graph, target_graph, colors, result, "constraint"
        ),
        diagnostics=diagnostics,
    )


def _build_graph(
    instance: Instance, *, atol: float, color_atol: float, include_metadata: bool
) -> _Graph:
    variables: dict[int, tuple[Any, ...]] = {}
    for attached in instance.decision_variables:
        var = attached.detach()
        variables[var.id] = _variable_attrs(
            var, atol=color_atol, include_metadata=include_metadata
        )

    factors: dict[_FactorKey, _FactorNode] = {}
    objective_terms = _function_terms(instance.objective, atol=atol)
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
            function_terms=_function_terms(attached.function, atol=atol),
            equality=str(attached.equality),
            metadata=metadata,
            atol=color_atol,
        )

    for constraint_id, attached in instance.indicator_constraints.items():
        metadata = _constraint_metadata(attached, include_metadata)
        terms = _function_terms(attached.function, atol=atol)
        neighbors = dict(_function_neighbors(terms, atol=color_atol))
        indicator_id = int(attached.indicator_variable_id)
        labels = list(neighbors.get(indicator_id, ()))
        labels.append(("indicator",))
        neighbors[indicator_id] = tuple(sorted(labels, key=repr))
        attrs = (
            "indicator",
            str(attached.equality),
            _function_shape_signature(terms, color_atol),
            metadata,
        )
        factors[("indicator", int(constraint_id))] = _FactorNode(
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

    var_to_factors: dict[int, dict[_FactorKey, tuple[Any, ...]]] = {
        variable_id: {} for variable_id in variables
    }
    for key, factor in factors.items():
        for variable_id, edge_label in factor.neighbors.items():
            var_to_factors.setdefault(variable_id, {})[key] = edge_label

    return _Graph(
        variables=variables,
        factors=factors,
        var_to_factors=var_to_factors,
        sense=str(instance.sense),
    )


def _resolve_color_atol(atol: float, color_atol: float | None) -> float:
    if atol < 0:
        raise ValueError("atol must be non-negative")
    if color_atol is None:
        return 10 * atol
    if color_atol < 0:
        raise ValueError("color_atol must be non-negative")
    if color_atol < atol:
        raise ValueError("color_atol must be greater than or equal to atol")
    return color_atol


def _variable_attrs(
    var: Any, *, atol: float, include_metadata: bool
) -> tuple[Any, ...]:
    attrs: tuple[Any, ...] = (
        "variable",
        int(var.kind),
        _quantize_float(float(var.bound.lower), atol),
        _quantize_float(float(var.bound.upper), atol),
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
    key: _FactorKey,
    kind: str,
    function_terms: _Terms,
    equality: str | None,
    metadata: tuple[Any, ...],
    atol: float,
    sense: str | None = None,
) -> _FactorNode:
    attrs = (
        kind,
        sense,
        equality,
        _function_shape_signature(function_terms, atol),
        metadata,
    )
    return _FactorNode(
        key=key,
        attrs=attrs,
        terms=function_terms,
        neighbors=_function_neighbors(function_terms, atol=atol),
    )


def _membership_factor(
    *,
    key: _FactorKey,
    kind: str,
    variables: tuple[int, ...],
    metadata: tuple[Any, ...],
) -> _FactorNode:
    neighbors = {variable_id: (("member",),) for variable_id in variables}
    attrs = (kind, len(variables), metadata)
    return _FactorNode(
        key=key,
        attrs=attrs,
        terms={},
        neighbors=neighbors,
        variables=tuple(sorted(variables)),
    )


def _function_terms(function: Function, *, atol: float) -> _Terms:
    terms: dict[_Monomial, float] = {}
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


def _function_neighbors(terms: _Terms, *, atol: float) -> dict[int, tuple[Any, ...]]:
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
                    _quantize_float(coefficient, atol),
                )
            )
    return {
        variable_id: tuple(sorted(edge_labels, key=repr))
        for variable_id, edge_labels in labels.items()
    }


def _function_shape_signature(terms: _Terms, atol: float) -> tuple[Any, ...]:
    items = []
    for monomial, coefficient in terms.items():
        power_shape = tuple(sorted(Counter(monomial).values()))
        items.append((len(monomial), power_shape, _quantize_float(coefficient, atol)))
    return tuple(sorted(items, key=repr))


def _colored_terms(
    terms: _Terms,
    variable_colors: Mapping[int, int],
    atol: float,
) -> tuple[Any, ...]:
    items = []
    for monomial, coefficient in terms.items():
        color_counts = tuple(
            sorted(Counter(variable_colors[v] for v in monomial).items())
        )
        items.append((color_counts, _quantize_float(coefficient, atol)))
    return tuple(sorted(items, key=repr))


def _refine_colors(
    source: _Graph,
    target: _Graph,
    *,
    atol: float,
    max_iterations: int,
) -> _Colors:
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

    return _Colors(
        source_variables=source_variable_colors,
        target_variables=target_variable_colors,
        source_factors=source_factor_colors,
        target_factors=target_factor_colors,
    )


def _factor_color_inputs(
    graph: _Graph,
    variable_colors: Mapping[int, int],
    *,
    atol: float,
) -> dict[_FactorKey, tuple[Any, ...]]:
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
    graph: _Graph,
    factor_colors: Mapping[_FactorKey, int],
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


def _search_mapping(
    source: _Graph,
    target: _Graph,
    colors: _Colors,
    *,
    max_steps: int,
    atol: float,
) -> tuple[_SearchState, int]:
    state = _SearchState(
        variable_map={},
        inverse_variable_map={},
        factor_map={("objective", -1): ("objective", -1)},
        inverse_factor_map={("objective", -1): ("objective", -1)},
    )
    best = state.copy()
    steps = 0

    variable_candidates = _variable_candidates_by_color(source, target, colors)
    factor_candidates = _factor_candidates_by_color(source, target, colors)

    def remember(candidate: _SearchState) -> None:
        nonlocal best
        if _mapped_size(candidate) > _mapped_size(best):
            best = candidate.copy()

    def recurse(current: _SearchState) -> _SearchState | None:
        nonlocal steps
        steps += 1
        remember(current)
        if steps > max_steps:
            return None
        if _source_is_fully_mapped(source, current):
            if _verify_mapping(source, target, current, atol=atol):
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


def _variable_candidates_by_color(
    source: _Graph, target: _Graph, colors: _Colors
) -> dict[int, tuple[int, ...]]:
    by_color: dict[int, list[int]] = {}
    for target_variable, color in colors.target_variables.items():
        by_color.setdefault(color, []).append(target_variable)
    return {
        source_variable: tuple(sorted(by_color.get(color, ())))
        for source_variable, color in colors.source_variables.items()
    }


def _factor_candidates_by_color(
    source: _Graph, target: _Graph, colors: _Colors
) -> dict[_FactorKey, tuple[_FactorKey, ...]]:
    by_color: dict[tuple[str, int], list[_FactorKey]] = {}
    for target_key, color in colors.target_factors.items():
        by_color.setdefault((target_key[0], color), []).append(target_key)
    return {
        source_key: tuple(sorted(by_color.get((source_key[0], color), ())))
        for source_key, color in colors.source_factors.items()
        if source_key[0] != "objective"
    }


def _choose_next_item(
    source: _Graph,
    target: _Graph,
    state: _SearchState,
    variable_candidates: Mapping[int, tuple[int, ...]],
    factor_candidates: Mapping[_FactorKey, tuple[_FactorKey, ...]],
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

    for source_factor in _output_factor_keys(source):
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
    source: _Graph,
    target: _Graph,
    state: _SearchState,
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
    source: _Graph,
    target: _Graph,
    state: _SearchState,
    source_factor: _FactorKey,
    target_factor: _FactorKey,
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
    source: _Graph,
    target: _Graph,
    state: _SearchState,
    source_factor: _FactorKey,
    target_factor: _FactorKey,
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
    source_terms: _Terms,
    target_terms: _Terms,
    variable_map: Mapping[int, int],
    inverse_variable_map: Mapping[int, int],
    *,
    atol: float,
) -> bool:
    source_known = {
        _map_monomial(monomial, variable_map): coefficient
        for monomial, coefficient in source_terms.items()
        if all(variable_id in variable_map for variable_id in monomial)
    }
    target_known = {
        monomial: coefficient
        for monomial, coefficient in target_terms.items()
        if all(variable_id in inverse_variable_map for variable_id in monomial)
    }
    return _terms_close(source_known, target_known, atol=atol)


def _verify_mapping(
    source: _Graph,
    target: _Graph,
    state: _SearchState,
    *,
    atol: float,
) -> bool:
    if len(source.variables) != len(target.variables):
        return False
    if len(state.variable_map) != len(source.variables):
        return False
    if source.sense != target.sense:
        return False
    if len(_output_factor_keys(source)) != len(_output_factor_keys(target)):
        return False
    if len(state.factor_map) != len(source.factors):
        return False
    if not _objective_matches(source, target, state, atol=atol):
        return False

    for source_factor, target_factor in state.factor_map.items():
        if source_factor[0] == "objective":
            continue
        if source_factor[0] != target_factor[0]:
            return False
        if not _factor_matches(
            source.factors[source_factor],
            target.factors[target_factor],
            state.variable_map,
            atol=atol,
        ):
            return False
    return True


def _objective_matches(
    source: _Graph,
    target: _Graph,
    state: _SearchState,
    *,
    atol: float,
) -> bool:
    if len(state.variable_map) != len(source.variables):
        return False
    if source.sense != target.sense:
        return False
    return _function_matches(
        source.factors[("objective", -1)].terms,
        target.factors[("objective", -1)].terms,
        state.variable_map,
        atol=atol,
    )


def _factor_matches(
    source: _FactorNode,
    target: _FactorNode,
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
    return _function_matches(source.terms, target.terms, variable_map, atol=atol)


def _function_matches(
    source_terms: _Terms,
    target_terms: _Terms,
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
    mapped_terms: dict[_Monomial, float] = {}
    for monomial, coefficient in source_terms.items():
        mapped_monomial = _map_monomial(monomial, variable_map)
        mapped_terms[mapped_monomial] = (
            mapped_terms.get(mapped_monomial, 0.0) + coefficient
        )
    return _terms_close(mapped_terms, target_terms, atol=atol)


def _map_monomial(monomial: _Monomial, variable_map: Mapping[int, int]) -> _Monomial:
    return tuple(sorted(variable_map[variable_id] for variable_id in monomial))


def _terms_close(source_terms: _Terms, target_terms: _Terms, *, atol: float) -> bool:
    keys = set(source_terms) | set(target_terms)
    for key in keys:
        if abs(source_terms.get(key, 0.0) - target_terms.get(key, 0.0)) > atol:
            return False
    return True


def _source_is_fully_mapped(source: _Graph, state: _SearchState) -> bool:
    return len(state.variable_map) == len(source.variables) and len(
        state.factor_map
    ) == len(source.factors)


def _output_factor_keys(graph: _Graph) -> tuple[_FactorKey, ...]:
    return tuple(
        sorted((key for key in graph.factors if key[0] != "objective"), key=repr)
    )


def _extract_factor_mapping(state: _SearchState, kind: str) -> dict[int, int]:
    out = {}
    for source_factor, target_factor in state.factor_map.items():
        if source_factor[0] == kind:
            out[source_factor[1]] = target_factor[1]
    return dict(sorted(out.items()))


def _score(source: _Graph, state: _SearchState) -> float:
    total = len(source.variables) + len(_output_factor_keys(source))
    if total == 0:
        return 1.0
    mapped = len(state.variable_map) + sum(
        1 for factor_key in state.factor_map if factor_key[0] != "objective"
    )
    return mapped / total


def _mapped_size(state: _SearchState) -> int:
    return len(state.variable_map) + sum(
        1 for factor_key in state.factor_map if factor_key[0] != "objective"
    )


def _ambiguous_variables(
    source: _Graph,
    target: _Graph,
    colors: _Colors,
    state: _SearchState,
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


def _ambiguous_factors(
    source: _Graph,
    target: _Graph,
    colors: _Colors,
    state: _SearchState,
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


def _diagnostics(
    source: _Graph,
    target: _Graph,
    state: _SearchState,
    verified: bool,
    steps: int,
) -> tuple[str, ...]:
    messages = []
    if verified:
        return ()
    if len(source.variables) != len(target.variables):
        messages.append(
            f"decision variable count differs: {len(source.variables)} != {len(target.variables)}"
        )
    for kind in ("constraint", "indicator", "one_hot", "sos1"):
        source_count = sum(1 for key in source.factors if key[0] == kind)
        target_count = sum(1 for key in target.factors if key[0] == kind)
        if source_count != target_count:
            messages.append(f"{kind} count differs: {source_count} != {target_count}")
    if source.sense != target.sense:
        messages.append(f"sense differs: {source.sense} != {target.sense}")
    if len(state.variable_map) < len(source.variables):
        messages.append("some source decision variables remain unmapped")
    if len(state.factor_map) < len(source.factors):
        messages.append("some source constraints remain unmapped")
    messages.append(f"search stopped after {steps} steps")
    return tuple(messages)


def _quantize_optional_float(
    value: float | None, atol: float
) -> tuple[str, int] | None:
    if value is None:
        return None
    return _quantize_float(float(value), atol)


def _quantize_float(value: float, atol: float) -> tuple[str, int]:
    if math.isnan(value):
        return ("nan", 0)
    if math.isinf(value):
        return ("inf", 1 if value > 0 else -1)
    if atol <= 0:
        return ("exact", hash(repr(value)))
    return ("q", int(round(value / atol)))
