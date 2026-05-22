from __future__ import annotations

from collections.abc import Iterable

from ommx_column_generation import (
    Column,
    ColumnGenerationProblem,
    MasterRow,
    PricingContext,
    PricingResult,
    highs_master_solver,
    solve_column_generation,
)


def finite_catalog_pricing(candidates: Iterable[Column]):
    """Build a tiny pricing oracle by scanning a finite column catalog."""

    remaining = {column.id: column for column in candidates}

    def pricing_oracle(context: PricingContext) -> PricingResult:
        existing_ids = {column.id for column in context.columns}
        best_column = None
        best_reduced_cost = 0.0

        for column_id, column in remaining.items():
            if column_id in existing_ids:
                continue

            reduced_cost = column.cost - sum(
                context.duals.get(row_id, 0.0) * coefficient
                for row_id, coefficient in column.coefficients.items()
            )
            if reduced_cost < best_reduced_cost - context.tolerance:
                best_column = column
                best_reduced_cost = reduced_cost

        if best_column is None:
            return PricingResult([], proven_no_negative_reduced_cost=True)

        print(
            "pricing:",
            f"add column {best_column.id!r}",
            f"with reduced cost {best_reduced_cost:.3f}",
        )
        return PricingResult([best_column])

    return pricing_oracle


def main() -> None:
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
    candidate_columns = [
        Column("ab", 3.0, {"cover_a": 1.0, "cover_b": 1.0}),
    ]

    result = solve_column_generation(
        problem,
        master_solver=highs_master_solver,
        pricing_oracle=finite_catalog_pricing(candidate_columns),
    )

    print("termination:", result.termination_reason)
    print("objective:", result.master_solution.objective)
    print("column values:", result.column_values)
    print("iterations:")
    for record in result.iterations:
        print(
            f"  {record.iteration}:",
            f"objective={record.master_objective:.3f}",
            f"accepted={record.accepted_column_ids}",
        )


if __name__ == "__main__":
    main()
