from __future__ import annotations

from collections.abc import Callable, Hashable, Iterable, Mapping
from dataclasses import dataclass, field
from typing import Any, Literal, Protocol

from ommx.v1 import DecisionVariable, Instance, Solution

RowSense = Literal["<=", ">=", "=="]
ColumnVariableKind = Literal["continuous", "binary"]


@dataclass(frozen=True)
class MasterRow:
    """A row of the restricted master problem."""

    id: Hashable
    sense: RowSense
    rhs: float
    name: str | None = None

    def __post_init__(self) -> None:
        if self.sense not in ("<=", ">=", "=="):
            raise ValueError(f"Unsupported row sense: {self.sense}")


@dataclass(frozen=True)
class Column:
    """A column of the restricted master problem."""

    id: Hashable
    cost: float
    coefficients: Mapping[Hashable, float]
    payload: Any = None


@dataclass(frozen=True)
class RestrictedMasterProblem:
    """OMMX representation of a restricted master problem."""

    instance: Instance
    row_id_to_constraint_id: dict[Hashable, int]
    row_id_to_sense: dict[Hashable, RowSense]
    column_id_to_variable_id: dict[Hashable, int]

    def raw_duals(self, solution: Solution) -> dict[Hashable, float]:
        """Extract adapter duals keyed by original row ID."""

        duals: dict[Hashable, float] = {}
        for row_id, constraint_id in self.row_id_to_constraint_id.items():
            value = solution.get_dual_variable(constraint_id)
            if value is None:
                raise RuntimeError(f"Missing dual variable for row {row_id!r}")
            duals[row_id] = value
        return duals

    def duals(self, solution: Solution) -> dict[Hashable, float]:
        """Extract row duals in the original master row orientation.

        RMP rows with sense ``>=`` are represented in OMMX as
        ``rhs - lhs <= 0``. Their adapter duals are therefore sign-flipped before
        being exposed to pricing oracles.
        """

        raw = self.raw_duals(solution)
        return {
            row_id: -value if self.row_id_to_sense[row_id] == ">=" else value
            for row_id, value in raw.items()
        }

    def column_values(self, solution: Solution) -> dict[Hashable, float]:
        """Extract lambda values from an RMP solution keyed by original column ID."""

        entries = solution.state.entries
        values: dict[Hashable, float] = {}
        for column_id, variable_id in self.column_id_to_variable_id.items():
            values[column_id] = entries.get(variable_id, 0.0)
        return values


@dataclass
class ColumnGenerationProblem:
    """Rows and current columns of a column generation master problem."""

    rows: list[MasterRow]
    columns: list[Column] = field(default_factory=list)
    sense: Literal["minimize", "maximize"] = "minimize"
    objective_offset: float = 0.0

    def __post_init__(self) -> None:
        if self.sense not in ("minimize", "maximize"):
            raise ValueError(f"Unsupported problem sense: {self.sense}")
        _ensure_unique((row.id for row in self.rows), "row")
        _ensure_unique((column.id for column in self.columns), "column")

    def add_columns(self, columns: Iterable[Column], *, skip_duplicates: bool = True) -> list[Column]:
        """Append columns and return the columns that were accepted."""

        known = {column.id for column in self.columns}
        accepted: list[Column] = []
        for column in columns:
            if column.id in known:
                if skip_duplicates:
                    continue
                raise ValueError(f"Duplicate column ID: {column.id!r}")
            self.columns.append(column)
            known.add(column.id)
            accepted.append(column)
        return accepted

    def build_restricted_master(
        self, *, column_kind: ColumnVariableKind = "continuous"
    ) -> RestrictedMasterProblem:
        """Build the current restricted master problem as an OMMX Instance."""

        if not self.columns:
            raise ValueError("At least one column is required to build an RMP")
        if column_kind not in ("continuous", "binary"):
            raise ValueError(f"Unsupported column variable kind: {column_kind}")

        lambda_vars: list[DecisionVariable] = []
        column_id_to_variable_id: dict[Hashable, int] = {}
        for index, column in enumerate(self.columns):
            parameters = {"org.ommx.column_generation.column_id": repr(column.id)}
            if column_kind == "continuous":
                variable = DecisionVariable.continuous(
                    index,
                    lower=0.0,
                    name="lambda",
                    subscripts=[index],
                    parameters=parameters,
                )
            else:
                variable = DecisionVariable.binary(
                    index,
                    name="lambda",
                    subscripts=[index],
                    parameters=parameters,
                )
            lambda_vars.append(variable)
            column_id_to_variable_id[column.id] = index

        objective = _zero(lambda_vars)
        objective += self.objective_offset
        for variable, column in zip(lambda_vars, self.columns):
            objective += column.cost * variable

        constraints = {}
        row_id_to_constraint_id: dict[Hashable, int] = {}
        row_id_to_sense: dict[Hashable, RowSense] = {}
        for constraint_id, row in enumerate(self.rows):
            lhs = _zero(lambda_vars)
            for variable, column in zip(lambda_vars, self.columns):
                coefficient = column.coefficients.get(row.id, 0.0)
                if coefficient:
                    lhs += coefficient * variable

            if row.sense == "<=":
                constraint = lhs - row.rhs <= 0
            elif row.sense == ">=":
                constraint = row.rhs - lhs <= 0
            else:
                constraint = lhs - row.rhs == 0

            parameters = {
                "org.ommx.column_generation.row_id": repr(row.id),
                "org.ommx.column_generation.row_sense": row.sense,
            }
            if row.name is not None:
                constraint = constraint.add_name(row.name)
            constraint = constraint.add_parameters(parameters)
            constraints[constraint_id] = constraint
            row_id_to_constraint_id[row.id] = constraint_id
            row_id_to_sense[row.id] = row.sense

        instance_sense = (
            Instance.MINIMIZE if self.sense == "minimize" else Instance.MAXIMIZE
        )
        instance = Instance.from_components(
            decision_variables=lambda_vars,
            objective=objective,
            constraints=constraints,
            sense=instance_sense,
        )
        return RestrictedMasterProblem(
            instance=instance,
            row_id_to_constraint_id=row_id_to_constraint_id,
            row_id_to_sense=row_id_to_sense,
            column_id_to_variable_id=column_id_to_variable_id,
        )


@dataclass(frozen=True)
class PricingContext:
    iteration: int
    rows: tuple[MasterRow, ...]
    columns: tuple[Column, ...]
    master_solution: Solution
    duals: Mapping[Hashable, float]
    tolerance: float


@dataclass(frozen=True)
class PricingResult:
    columns: list[Column]
    proven_no_negative_reduced_cost: bool = False


class PricingOracle(Protocol):
    def __call__(self, context: PricingContext) -> PricingResult:
        ...


MasterSolver = Callable[[Instance], Solution]


@dataclass(frozen=True)
class IterationRecord:
    iteration: int
    master_objective: float
    duals: Mapping[Hashable, float]
    generated_column_ids: tuple[Hashable, ...]
    accepted_column_ids: tuple[Hashable, ...]
    proven_no_negative_reduced_cost: bool


@dataclass(frozen=True)
class ColumnGenerationResult:
    master_solution: Solution
    restricted_master: RestrictedMasterProblem
    final_solution: Solution | None
    iterations: tuple[IterationRecord, ...]
    termination_reason: Literal[
        "no_columns",
        "proven_no_negative_reduced_cost",
        "max_iterations",
    ]

    @property
    def column_values(self) -> dict[Hashable, float]:
        return self.restricted_master.column_values(self.master_solution)


def solve_column_generation(
    problem: ColumnGenerationProblem,
    *,
    master_solver: MasterSolver,
    pricing_oracle: PricingOracle,
    final_solver: MasterSolver | None = None,
    max_iterations: int = 100,
    tolerance: float = 1e-6,
) -> ColumnGenerationResult:
    """Run a minimal column generation loop.

    The input problem is mutated by appending accepted columns.
    """

    if max_iterations < 0:
        raise ValueError("max_iterations must be non-negative")

    iteration_records: list[IterationRecord] = []
    termination_reason: Literal[
        "no_columns",
        "proven_no_negative_reduced_cost",
        "max_iterations",
    ] = "max_iterations"

    rmp = problem.build_restricted_master(column_kind="continuous")
    master_solution = master_solver(rmp.instance)

    for iteration in range(max_iterations):
        duals = rmp.duals(master_solution)
        context = PricingContext(
            iteration=iteration,
            rows=tuple(problem.rows),
            columns=tuple(problem.columns),
            master_solution=master_solution,
            duals=duals,
            tolerance=tolerance,
        )
        pricing_result = pricing_oracle(context)
        accepted = problem.add_columns(pricing_result.columns, skip_duplicates=True)
        iteration_records.append(
            IterationRecord(
                iteration=iteration,
                master_objective=master_solution.objective,
                duals=dict(duals),
                generated_column_ids=tuple(column.id for column in pricing_result.columns),
                accepted_column_ids=tuple(column.id for column in accepted),
                proven_no_negative_reduced_cost=pricing_result.proven_no_negative_reduced_cost,
            )
        )

        if not accepted:
            termination_reason = (
                "proven_no_negative_reduced_cost"
                if pricing_result.proven_no_negative_reduced_cost
                else "no_columns"
            )
            break

        rmp = problem.build_restricted_master(column_kind="continuous")
        master_solution = master_solver(rmp.instance)

    final_solution = None
    if final_solver is not None:
        final_rmp = problem.build_restricted_master(column_kind="binary")
        final_solution = final_solver(final_rmp.instance)

    return ColumnGenerationResult(
        master_solution=master_solution,
        restricted_master=rmp,
        final_solution=final_solution,
        iterations=tuple(iteration_records),
        termination_reason=termination_reason,
    )


def _ensure_unique(values: Iterable[Hashable], label: str) -> None:
    seen: set[Hashable] = set()
    for value in values:
        if value in seen:
            raise ValueError(f"Duplicate {label} ID: {value!r}")
        seen.add(value)


def _zero(variables: list[DecisionVariable]):
    return 0 * variables[0]
