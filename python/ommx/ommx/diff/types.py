from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
from typing import Any

FactorKey = tuple[str, int]
Monomial = tuple[int, ...]
Terms = dict[Monomial, float]


@dataclass(frozen=True)
class FactorNode:
    key: FactorKey
    attrs: tuple[Any, ...]
    terms: Terms
    neighbors: Mapping[int, tuple[Any, ...]]
    variables: tuple[int, ...] = ()
    indicator_variable: int | None = None


@dataclass(frozen=True)
class InstanceGraph:
    variables: Mapping[int, tuple[Any, ...]]
    factors: Mapping[FactorKey, FactorNode]
    var_to_factors: Mapping[int, Mapping[FactorKey, tuple[Any, ...]]]
    sense: str


@dataclass
class Colors:
    source_variables: dict[int, int]
    target_variables: dict[int, int]
    source_factors: dict[FactorKey, int]
    target_factors: dict[FactorKey, int]


@dataclass
class SearchState:
    variable_map: dict[int, int]
    inverse_variable_map: dict[int, int]
    factor_map: dict[FactorKey, FactorKey]
    inverse_factor_map: dict[FactorKey, FactorKey]

    def copy(self) -> "SearchState":
        return SearchState(
            variable_map=dict(self.variable_map),
            inverse_variable_map=dict(self.inverse_variable_map),
            factor_map=dict(self.factor_map),
            inverse_factor_map=dict(self.inverse_factor_map),
        )
