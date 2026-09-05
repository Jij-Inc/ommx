from __future__ import annotations

import pytest

from ommx_column_generation import (
    Column,
    ColumnGenerationProblem,
    PricingContext,
    PricingResult,
    MasterRow,
    highs_master_solver,
    solve_column_generation,
)


def test_build_restricted_master_and_solve_with_highs():
    problem = ColumnGenerationProblem(
        rows=[
            MasterRow("cover_a", ">=", 1.0),
            MasterRow("cover_b", ">=", 1.0),
        ],
        columns=[
            Column("a", 1.0, {"cover_a": 1.0}),
            Column("b", 1.0, {"cover_b": 1.0}),
            Column("ab", 3.0, {"cover_a": 1.0, "cover_b": 1.0}),
        ],
    )

    rmp = problem.build_restricted_master()
    solution = highs_master_solver(rmp.instance)

    assert solution.objective == pytest.approx(2.0)
    assert rmp.column_values(solution) == pytest.approx({"a": 1.0, "b": 1.0, "ab": 0.0})
    assert rmp.raw_duals(solution) == pytest.approx({"cover_a": -1.0, "cover_b": -1.0})
    assert rmp.duals(solution) == pytest.approx({"cover_a": 1.0, "cover_b": 1.0})


def test_solve_column_generation_accepts_columns_from_pricing_oracle():
    problem = ColumnGenerationProblem(
        rows=[
            MasterRow("cover_a", ">=", 1.0),
            MasterRow("cover_b", ">=", 1.0),
        ],
        columns=[
            Column("a", 2.0, {"cover_a": 1.0}),
            Column("b", 2.0, {"cover_b": 1.0}),
        ],
    )
    seen_duals = []

    def pricing_oracle(context: PricingContext) -> PricingResult:
        seen_duals.append(set(context.duals))
        if context.iteration == 0:
            return PricingResult([Column("ab", 3.0, {"cover_a": 1.0, "cover_b": 1.0})])
        return PricingResult([], proven_no_negative_reduced_cost=True)

    result = solve_column_generation(
        problem,
        master_solver=highs_master_solver,
        pricing_oracle=pricing_oracle,
        max_iterations=5,
    )

    assert [column.id for column in problem.columns] == ["a", "b", "ab"]
    assert result.termination_reason == "proven_no_negative_reduced_cost"
    assert [record.accepted_column_ids for record in result.iterations] == [
        ("ab",),
        (),
    ]
    assert seen_duals == [{"cover_a", "cover_b"}, {"cover_a", "cover_b"}]
    assert result.master_solution.objective == pytest.approx(3.0)
    assert result.column_values == pytest.approx({"a": 0.0, "b": 0.0, "ab": 1.0})


def test_duplicate_column_ids_are_skipped_by_default():
    problem = ColumnGenerationProblem(
        rows=[MasterRow("cover", ">=", 1.0)],
        columns=[Column("existing", 1.0, {"cover": 1.0})],
    )

    accepted = problem.add_columns(
        [
            Column("existing", 0.5, {"cover": 1.0}),
            Column("new", 0.75, {"cover": 1.0}),
        ]
    )

    assert [column.id for column in accepted] == ["new"]
    assert [column.id for column in problem.columns] == ["existing", "new"]
