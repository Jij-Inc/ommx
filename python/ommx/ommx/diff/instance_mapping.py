from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass

from ommx.v1 import Instance

from .graph import build_graph, resolve_color_atol
from .isomorphism import (
    ambiguous_factors,
    ambiguous_variables,
    refine_colors,
    search_mapping,
)
from .types import InstanceGraph, SearchState
from .verify import objective_matches, output_factor_keys, verify_mapping


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

    :param source: Instance whose IDs are mapped from.
    :param target: Instance whose IDs are mapped to.
    :param atol: Absolute tolerance used when grouping coefficients and bounds.
    :param color_atol: Coarser absolute tolerance used only for
        color/fingerprint bucketing. The final verification still uses
        ``atol``.
    :param max_refinement_iterations: Maximum color-refinement rounds.
    :param max_backtracking_steps: Search budget for ambiguous color classes.
    :param include_metadata: Include names, subscripts, and parameters in
        fingerprints. The default ignores metadata and matches math shape.
    """

    effective_color_atol = resolve_color_atol(atol, color_atol)
    source_graph = build_graph(
        source,
        atol=atol,
        color_atol=effective_color_atol,
        include_metadata=include_metadata,
    )
    target_graph = build_graph(
        target,
        atol=atol,
        color_atol=effective_color_atol,
        include_metadata=include_metadata,
    )
    colors = refine_colors(
        source_graph,
        target_graph,
        atol=effective_color_atol,
        max_iterations=max_refinement_iterations,
    )
    result, steps = search_mapping(
        source_graph,
        target_graph,
        colors,
        max_steps=max_backtracking_steps,
        atol=atol,
    )
    verified = verify_mapping(source_graph, target_graph, result, atol=atol)
    matches_objective = objective_matches(source_graph, target_graph, result, atol=atol)
    diagnostics = _diagnostics(source_graph, target_graph, result, verified, steps)

    return InstanceMapping(
        decision_variables=dict(sorted(result.variable_map.items())),
        constraints=_extract_factor_mapping(result, "constraint"),
        indicator_constraints=_extract_factor_mapping(result, "indicator"),
        one_hot_constraints=_extract_factor_mapping(result, "one_hot"),
        sos1_constraints=_extract_factor_mapping(result, "sos1"),
        objective_matches=matches_objective,
        verified=verified,
        score=_score(source_graph, result),
        backtracking_steps=steps,
        ambiguous_decision_variables=ambiguous_variables(
            source_graph, target_graph, colors, result
        ),
        ambiguous_constraints=ambiguous_factors(
            source_graph, target_graph, colors, result, "constraint"
        ),
        diagnostics=diagnostics,
    )


def _extract_factor_mapping(state: SearchState, kind: str) -> dict[int, int]:
    out = {}
    for source_factor, target_factor in state.factor_map.items():
        if source_factor[0] == kind:
            out[source_factor[1]] = target_factor[1]
    return dict(sorted(out.items()))


def _score(source: InstanceGraph, state: SearchState) -> float:
    total = len(source.variables) + len(output_factor_keys(source))
    if total == 0:
        return 1.0
    mapped = len(state.variable_map) + sum(
        1 for factor_key in state.factor_map if factor_key[0] != "objective"
    )
    return mapped / total


def _diagnostics(
    source: InstanceGraph,
    target: InstanceGraph,
    state: SearchState,
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
