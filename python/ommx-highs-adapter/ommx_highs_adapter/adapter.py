from __future__ import annotations

from dataclasses import asdict, dataclass, fields
from typing import Any, Callable, Iterable, Mapping, cast

import highspy
import numpy as np

from highspy.highs import highs_linear_expression
from opentelemetry import trace

from ommx.v1 import Instance, DecisionVariable, Solution, Constraint, State, Function
from ommx.adapter import (
    DiagnosticsSink,
    SolverAdapter,
    InfeasibleDetected,
    UnboundedDetected,
)

from .exception import OMMXHighsAdapterError

_tracer = trace.get_tracer("ommx.adapter.highs")


@dataclass(frozen=True, slots=True)
class HighsProgressSnapshot:
    """HiGHS MIP solve progress observed from one logging callback."""

    event: str
    """HiGHS callback message, currently ``"MIP logging"``."""

    solving_time_sec: float
    """HiGHS runtime when the callback ran."""

    mip_node_count: int
    """MIP branch-and-bound nodes processed at the callback."""

    simplex_iteration_count: int
    """Simplex iterations at the callback."""

    ipm_iteration_count: int
    """Interior-point iterations at the callback."""

    pdlp_iteration_count: int
    """PDLP iterations at the callback."""

    objective_value: float
    """Objective value reported at the callback."""

    primal_bound: float
    """MIP primal bound reported at the callback."""

    dual_bound: float
    """MIP dual bound reported at the callback."""

    gap: float
    """MIP relative gap reported at the callback."""

    @classmethod
    def from_callback_event(cls, event: Any) -> HighsProgressSnapshot:
        data = event.data_out
        return cls(
            event=event.message,
            solving_time_sec=data.running_time,
            mip_node_count=int(data.mip_node_count),
            simplex_iteration_count=int(data.simplex_iteration_count),
            ipm_iteration_count=int(data.ipm_iteration_count),
            pdlp_iteration_count=int(data.pdlp_iteration_count),
            objective_value=data.objective_function_value,
            primal_bound=data.mip_primal_bound,
            dual_bound=data.mip_dual_bound,
            gap=data.mip_gap,
        )


@dataclass(frozen=True, slots=True)
class HighsTerminationReport:
    """HiGHS-side termination summary recorded after ``model.run()``.

    The HiGHS adapter records this report before decoding the optimized HiGHS
    model back into an OMMX solution. It is therefore available even when
    decoding raises an adapter exception such as infeasible or unbounded
    detection.
    """

    status: str
    """HiGHS model status, such as ``"Optimal"`` or ``"Infeasible"``."""

    objective_value: float | None
    """Objective value reported by HiGHS, or ``None`` when no objective is valid."""

    mip_dual_bound: float
    """MIP dual bound reported by ``HighsInfo.mip_dual_bound``."""

    mip_gap: float
    """MIP relative gap reported by ``HighsInfo.mip_gap``."""

    mip_node_count: int
    """Number of MIP branch-and-bound nodes processed by HiGHS."""

    simplex_iteration_count: int
    """Number of simplex iterations."""

    ipm_iteration_count: int
    """Number of interior-point iterations."""

    crossover_iteration_count: int
    """Number of crossover iterations after interior-point optimization."""

    pdlp_iteration_count: int
    """Number of PDLP iterations."""

    primal_dual_integral: float
    """HiGHS primal-dual integral."""

    primal_solution_status: int
    """HiGHS primal solution status code."""

    dual_solution_status: int
    """HiGHS dual solution status code."""

    max_integrality_violation: float
    """Maximum integrality violation in the final solution."""

    max_primal_infeasibility: float
    """Maximum primal infeasibility in the final solution."""

    max_dual_infeasibility: float
    """Maximum dual infeasibility in the final solution."""

    run_time_sec: float
    """HiGHS runtime in seconds."""

    highs_version: str
    """HiGHS version used through highspy."""

    highs_githash: str
    """HiGHS git hash reported by highspy."""

    @classmethod
    def from_model(cls, model: highspy.Highs) -> HighsTerminationReport:
        status = model.getModelStatus()
        status_name = model.modelStatusToString(status)
        if status == highspy.HighsModelStatus.kNotset:
            raise OMMXHighsAdapterError(
                f"The model may not be optimized. [status: {status_name}]"
            )

        info = model.getInfo()
        objective_value = None
        if status not in {
            highspy.HighsModelStatus.kInfeasible,
            highspy.HighsModelStatus.kUnbounded,
            highspy.HighsModelStatus.kUnboundedOrInfeasible,
            highspy.HighsModelStatus.kModelError,
            highspy.HighsModelStatus.kPresolveError,
            highspy.HighsModelStatus.kSolveError,
            highspy.HighsModelStatus.kPostsolveError,
            highspy.HighsModelStatus.kLoadError,
            highspy.HighsModelStatus.kNotset,
        }:
            objective_value = info.objective_function_value

        return cls(
            status=status_name,
            objective_value=objective_value,
            mip_dual_bound=info.mip_dual_bound,
            mip_gap=info.mip_gap,
            mip_node_count=int(info.mip_node_count),
            simplex_iteration_count=int(info.simplex_iteration_count),
            ipm_iteration_count=int(info.ipm_iteration_count),
            crossover_iteration_count=int(info.crossover_iteration_count),
            pdlp_iteration_count=int(info.pdlp_iteration_count),
            primal_dual_integral=float(getattr(info, "primal_dual_integral")),
            primal_solution_status=int(info.primal_solution_status),
            dual_solution_status=int(info.dual_solution_status),
            max_integrality_violation=info.max_integrality_violation,
            max_primal_infeasibility=info.max_primal_infeasibility,
            max_dual_infeasibility=info.max_dual_infeasibility,
            run_time_sec=model.getRunTime(),
            highs_version=model.version(),
            highs_githash=model.githash(),
        )


class HighsDiagnosticsAnalyzer:
    """Post-processor for HiGHS diagnostics.

    The analyzer accepts either typed diagnostics collected by
    :class:`ommx.adapter.DiagnosticCollector` or dictionaries loaded from
    :attr:`ommx.experiment.Solve.diagnostics`.
    """

    _progress_snapshots: tuple[HighsProgressSnapshot, ...]
    _progress_history_records: tuple[dict[str, object], ...]
    _termination_result: dict[str, object] | None

    def __init__(self, diagnostics: Iterable[Any]) -> None:
        progress_snapshots: list[HighsProgressSnapshot] = []
        progress_history_records: list[dict[str, object]] = []
        termination_results: list[dict[str, object]] = []

        for diagnostic in diagnostics:
            if isinstance(diagnostic, HighsProgressSnapshot):
                progress_snapshots.append(diagnostic)
            if (record := _as_progress_history_record(diagnostic)) is not None:
                progress_history_records.append(record)
            if (result := _as_termination_result(diagnostic)) is not None:
                termination_results.append(result)

        self._progress_snapshots = tuple(progress_snapshots)
        self._progress_history_records = tuple(progress_history_records)
        self._termination_result = (
            termination_results[-1] if termination_results else None
        )

    @property
    def progress_snapshots(self) -> tuple[HighsProgressSnapshot, ...]:
        """Typed progress snapshots in the order they were recorded."""
        return self._progress_snapshots

    @property
    def progress_history_records(self) -> list[dict[str, object]]:
        """Return one dictionary per HiGHS MIP logging callback."""
        return [dict(record) for record in self._progress_history_records]

    @property
    def progress_history_df(self) -> Any:
        """Return progress snapshots as a pandas DataFrame indexed by time."""
        return _dataframe(
            self.progress_history_records,
            _dataclass_field_names(HighsProgressSnapshot),
            index="solving_time_sec",
        )

    @property
    def event(self) -> Any:
        """Return progress event names indexed by solving time."""
        return self._progress_series("event")

    @property
    def mip_node_count(self) -> Any:
        """Return MIP node counts indexed by solving time."""
        return self._progress_series("mip_node_count")

    @property
    def node_count(self) -> Any:
        """Alias for :attr:`mip_node_count`."""
        return self.mip_node_count

    @property
    def simplex_iteration_count(self) -> Any:
        """Return simplex iteration counts indexed by solving time."""
        return self._progress_series("simplex_iteration_count")

    @property
    def ipm_iteration_count(self) -> Any:
        """Return interior-point iteration counts indexed by solving time."""
        return self._progress_series("ipm_iteration_count")

    @property
    def pdlp_iteration_count(self) -> Any:
        """Return PDLP iteration counts indexed by solving time."""
        return self._progress_series("pdlp_iteration_count")

    @property
    def objective_value(self) -> Any:
        """Return objective values indexed by solving time."""
        return self._progress_series("objective_value")

    @property
    def mip_primal_bound(self) -> Any:
        """Alias for :attr:`primal_bound`."""
        return self.primal_bound

    @property
    def primal_bound(self) -> Any:
        """Return MIP primal bounds indexed by solving time."""
        return self._progress_series("primal_bound")

    @property
    def mip_dual_bound(self) -> Any:
        """Alias for :attr:`dual_bound`."""
        return self.dual_bound

    @property
    def dual_bound(self) -> Any:
        """Return MIP dual bounds indexed by solving time."""
        return self._progress_series("dual_bound")

    @property
    def mip_gap(self) -> Any:
        """Alias for :attr:`gap`."""
        return self.gap

    @property
    def gap(self) -> Any:
        """Return MIP gaps indexed by solving time."""
        return self._progress_series("gap")

    @property
    def termination_result(self) -> dict[str, object] | None:
        """Return the terminal HiGHS report as one dictionary, if present."""
        if self._termination_result is None:
            return None
        return dict(self._termination_result)

    @property
    def termination_status(self) -> str | None:
        """Return the terminal HiGHS model status, if present."""
        return cast(str | None, self._termination_value("status"))

    @property
    def termination_objective_value(self) -> float | None:
        """Return the terminal objective value, if present."""
        return cast(float | None, self._termination_value("objective_value"))

    @property
    def termination_mip_dual_bound(self) -> float | None:
        """Return the terminal HiGHS MIP dual bound, if present."""
        return cast(float | None, self._termination_value("mip_dual_bound"))

    @property
    def termination_mip_gap(self) -> float | None:
        """Return the terminal HiGHS MIP gap, if present."""
        return cast(float | None, self._termination_value("mip_gap"))

    @property
    def termination_mip_node_count(self) -> int | None:
        """Return the terminal HiGHS MIP node count, if present."""
        return cast(int | None, self._termination_value("mip_node_count"))

    def _termination_value(self, column: str) -> object | None:
        if self._termination_result is None:
            return None
        return self._termination_result[column]

    def _progress_series(self, column: str) -> Any:
        return self.progress_history_df[column]


def _as_progress_history_record(diagnostic: Any) -> dict[str, object] | None:
    if isinstance(diagnostic, HighsProgressSnapshot):
        return cast(dict[str, object], asdict(diagnostic))
    if not isinstance(diagnostic, Mapping):
        return None
    if not _has_dataclass_fields(diagnostic, HighsProgressSnapshot):
        return None
    return _select_dataclass_fields(diagnostic, HighsProgressSnapshot)


def _as_termination_result(diagnostic: Any) -> dict[str, object] | None:
    if isinstance(diagnostic, HighsTerminationReport):
        return cast(dict[str, object], asdict(diagnostic))
    if not isinstance(diagnostic, Mapping):
        return None
    if not _has_dataclass_fields(diagnostic, HighsTerminationReport):
        return None
    return _select_dataclass_fields(diagnostic, HighsTerminationReport)


def _dataclass_field_names(dataclass_type: type[Any]) -> list[str]:
    return [field.name for field in fields(dataclass_type)]


def _has_dataclass_fields(
    diagnostic: Mapping[Any, Any], dataclass_type: type[Any]
) -> bool:
    return set(_dataclass_field_names(dataclass_type)) <= diagnostic.keys()


def _select_dataclass_fields(
    diagnostic: Mapping[Any, Any], dataclass_type: type[Any]
) -> dict[str, object]:
    return {
        name: cast(object, diagnostic[name])
        for name in _dataclass_field_names(dataclass_type)
    }


def _dataframe(
    records: list[dict[str, object]], columns: list[str], *, index: str | None = None
) -> Any:
    try:
        import pandas as pd
    except ImportError as error:
        raise ImportError(
            "pandas is required for HighsDiagnosticsAnalyzer DataFrame and Series "
            "properties. Use progress_history_records without pandas."
        ) from error
    dataframe = pd.DataFrame.from_records(records, columns=columns)
    if index is not None:
        dataframe = dataframe.set_index(index)
    return dataframe


class OMMXHighsAdapter(SolverAdapter):
    """
    OMMX Adapter for HiGHS solver.

    This adapter translates OMMX optimization problems (ommx.v1.Instance) into HiGHS-compatible
    formats and converts HiGHS solutions back to OMMX format (ommx.v1.Solution).

    Translation Specifications
    ==========================

    Decision Variables
    ------------------
    The adapter handles the following translations for decision variables:

    **ID Management**:

    - OMMX: Variables managed by IDs (non-sequential integers)
    - HiGHS: Variables managed by array indices (0-based sequential)
    - Mapping maintained internally for bidirectional conversion

    **Variable Types**:

    .. list-table::
       :header-rows: 1

       * - OMMX Type
         - HiGHS Type
         - Bounds
       * - ``DecisionVariable.BINARY``
         - ``HighsVarType.kInteger``
         - ``[0, 1]``
       * - ``DecisionVariable.INTEGER``
         - ``HighsVarType.kInteger``
         - ``[var.bound.lower, var.bound.upper]``
       * - ``DecisionVariable.CONTINUOUS``
         - ``HighsVarType.kContinuous``
         - ``[var.bound.lower, var.bound.upper]``
       * - ``DecisionVariable.SEMI_INTEGER``
         - **Not supported** (support planned)
         - \\-
       * - ``DecisionVariable.SEMI_CONTINUOUS``
         - **Not supported** (support planned)
         - \\-

    **Note**: Semi-integer and semi-continuous variables are planned for future support but are
    currently unsupported. Using these variable types will raise an ``OMMXHighsAdapterError``.

    Constraints
    -----------
    **Supported Function Types**:

    - Constant functions (ommx.v1.Function.constant)
    - Linear functions (ommx.v1.Function.linear)

    **Constraint Types**:

    .. list-table::
       :header-rows: 1

       * - OMMX Constraint
         - Mathematical Form
         - HiGHS Constraint
       * - ``Constraint.EQUAL_TO_ZERO``
         - f(x) = 0
         - ``const_expr == 0``
       * - ``Constraint.LESS_THAN_OR_EQUAL_TO_ZERO``
         - f(x) ≤ 0
         - ``const_expr <= 0``

    **Constant Constraint Handling**:

    - Equality: Skip if \\|constant\\| ≤ 1e-10, error if \\|constant\\| > 1e-10
    - Inequality: Skip if constant ≤ 1e-10, error if constant > 1e-10

    **Constraint ID Management**:

    - OMMX constraint IDs converted to HiGHS constraint names via ``str(constraint.id)``

    Objective Function
    ------------------
    **Optimization Direction**:

    .. list-table::
       :header-rows: 1

       * - OMMX Direction
         - HiGHS Method
       * - ``Instance.MINIMIZE``
         - ``model.minimize(...)``
       * - ``Instance.MAXIMIZE``
         - ``model.maximize(...)``

    **Function Types**:

    - Constant objectives: Processing skipped
    - Linear objectives: Converted to HiGHS linear expressions

    Solution Decoding
    -----------------
    **Variable Values**: Extracted from HiGHS ``solution.col_value`` using maintained ID mapping

    **Optimality Status**: Set to ``OPTIMALITY_OPTIMAL`` when HiGHS returns ``kOptimal``

    **Dual Variables**: Extracted from ``solution.row_dual`` for constraints

    Error Handling
    --------------
    **Unsupported Features**:

    - Quadratic functions (HiGHS supports linear problems only)
    - Semi-integer variables (``DecisionVariable.SEMI_INTEGER``, kind=4) - support planned
    - Semi-continuous variables (``DecisionVariable.SEMI_CONTINUOUS``, kind=5) - support planned
    - Constraint types other than ``EQUAL_TO_ZERO``/``LESS_THAN_OR_EQUAL_TO_ZERO``

    **Solver Status Mapping**:

    .. list-table::
       :header-rows: 1

       * - HiGHS Status
         - Exception
       * - ``kInfeasible``
         - ``InfeasibleDetected``
       * - ``kUnbounded``
         - ``UnboundedDetected``
       * - ``kNotset``
         - ``OMMXHighsAdapterError``

    Limitations
    -----------
    1. Linear problems only (no quadratic constraints or objectives)
    2. Constraint forms limited to equality (= 0) and inequality (≤ 0)
    3. Variable types limited to Binary, Integer, and Continuous

       - Semi-integer (SEMI_INTEGER) support is planned but not yet implemented
       - Semi-continuous (SEMI_CONTINUOUS) support is planned but not yet implemented

    Examples
    --------
    >>> from ommx_highs_adapter import OMMXHighsAdapter
    >>> from ommx.v1 import Instance, DecisionVariable
    >>>
    >>> # Define problem
    >>> x = DecisionVariable.binary(0)
    >>> y = DecisionVariable.integer(1, lower=0, upper=10)
    >>> instance = Instance.from_components(
    ...     decision_variables=[x, y],
    ...     objective=2*x + 3*y,
    ...     constraints={0: x + y <= 5},
    ...     sense=Instance.MAXIMIZE,
    ... )
    >>>
    >>> # Solve
    >>> solution = OMMXHighsAdapter.solve(instance)
    >>> print(f"Optimal value: {solution.objective}")
    Optimal value: 15.0
    >>> print(f"Variables: {solution.state.entries}")
    Variables: {0: 0.0, 1: 5.0}

    """

    def __init__(self, ommx_instance: Instance, *, verbose: bool = False):
        """
        Initialize the adapter with an OMMX instance.

        Parameters
        ----------
        ommx_instance : Instance
            The OMMX optimization problem to solve
        verbose : bool, default=False
            If True, enable HiGHS's console logging
        """
        with _tracer.start_as_current_span("convert"):
            super().__init__(ommx_instance)
            self.instance = ommx_instance
            self.model = highspy.Highs()

            # the default is for `log_to_console` to be True, so we
            # turn it off unless user requests it
            if not verbose:
                self.model.setOptionValue("log_to_console", False)

            self.var_ids = {}
            self.highs_vars = []

            self._set_decision_variables()
            self._set_objective()
            self._set_constraints()

    @classmethod
    def solve(
        cls,
        ommx_instance: Instance,
        *,
        verbose: bool = False,
        diagnostics: DiagnosticsSink | None = None,
    ) -> Solution:
        """
        Solve an OMMX optimization problem using HiGHS solver.

        This method provides a convenient interface for solving optimization problems
        without needing to manually instantiate the adapter. It handles the complete
        workflow: translation to HiGHS format, solving, and result conversion.

        Parameters
        ----------
        ommx_instance : Instance
            The OMMX optimization problem to solve. Must satisfy HiGHS adapter
            requirements: linear objective function (constant or linear terms only),
            linear constraints (constant or linear terms only), variables of type
            Binary, Integer, or Continuous only (Semi-integer and Semi-continuous
            support is planned but not yet implemented), and constraints of type
            ``EQUAL_TO_ZERO`` or ``LESS_THAN_OR_EQUAL_TO_ZERO`` only.

        verbose : bool, default=False
            If True, enable HiGHS's console logging for debugging

        Returns
        -------
        Solution
            The solution containing:
            - Variable values in solution.state.entries
            - Objective value in solution.objective
            - Constraint evaluations in solution.constraints
            - Optimality status in solution.optimality
            - Dual variables (if available) in constraint.dual_variable

        Raises
        ------
        InfeasibleDetected
            When the optimization problem has no feasible solution
        UnboundedDetected
            When the optimization problem is unbounded
        OMMXHighsAdapterError
            When the problem contains unsupported features or HiGHS encounters an error

        Examples
        --------
        **Knapsack Problem**

        >>> from ommx.v1 import Instance, DecisionVariable, Solution
        >>> from ommx_highs_adapter import OMMXHighsAdapter
        >>>
        >>> p = [10, 13, 18, 32, 7, 15]  # profits
        >>> w = [11, 15, 20, 35, 10, 33]  # weights
        >>> x = [DecisionVariable.binary(i) for i in range(6)]
        >>> instance = Instance.from_components(
        ...     decision_variables=x,
        ...     objective=sum(p[i] * x[i] for i in range(6)),
        ...     constraints={0: sum(w[i] * x[i] for i in range(6)) <= 47},
        ...     sense=Instance.MAXIMIZE,
        ... )
        >>>
        >>> solution = OMMXHighsAdapter.solve(instance)
        >>> sorted([(id, value) for id, value in solution.state.entries.items()])
        [(0, 1.0), (1, 0.0), (2, 0.0), (3, 1.0), (4, 0.0), (5, 0.0)]
        >>> solution.feasible
        True
        >>> assert solution.optimality == Solution.OPTIMAL
        >>> solution.objective
        42.0

        **Infeasible Problem**

        >>> x = DecisionVariable.integer(0, upper=3, lower=0)
        >>> instance = Instance.from_components(
        ...     decision_variables=[x],
        ...     objective=x,
        ...     constraints={0: x >= 4},  # Impossible: x ≤ 3 and x ≥ 4
        ...     sense=Instance.MAXIMIZE,
        ... )
        >>> OMMXHighsAdapter.solve(instance)  # doctest: +IGNORE_EXCEPTION_DETAIL
        Traceback (most recent call last):
            ...
        ommx.adapter.InfeasibleDetected: Model was infeasible
        """
        # TODO would have added an unbounded example/doctest above,
        # but the same example used with pyscipopt isn't being correctly
        # detected as unbounded by HiGHS (simply returned x=0 as the solution)
        # requires further investigation
        #
        # the example for reference:
        # ```
        # >>> from ommx.v1 import Instance, DecisionVariable
        # >>> from ommx_highs_adapter import OMMXHighsAdapter

        # >>> x = DecisionVariable.integer(0, lower=0)
        # >>> instance = Instance.from_components(
        # ...     decision_variables=[x],
        # ...     objective=x,
        # ...     constraints={},
        # ...     sense=Instance.MAXIMIZE,
        # ... )

        # >>> OMMXHighsAdapter.solve(instance)
        # Traceback (most recent call last):
        #     ...
        # ommx.adapter.UnboundedDetected: Model was unbounded
        # ````
        with _tracer.start_as_current_span("solve") as span:
            span.set_attribute("adapter", f"{cls.__module__}.{cls.__qualname__}")
            adapter = cls(ommx_instance, verbose=verbose)
            model = adapter.solver_input
            diagnostics_callback: Callable[[Any], None] | None = None
            if diagnostics is not None:

                def record_progress(event: Any) -> None:
                    diagnostics.record(HighsProgressSnapshot.from_callback_event(event))

                diagnostics_callback = record_progress
                model.cbMipLogging.subscribe(diagnostics_callback)
            with _tracer.start_as_current_span("call"):
                try:
                    model.run()
                finally:
                    if diagnostics_callback is not None:
                        model.cbMipLogging.unsubscribe(diagnostics_callback)
            if diagnostics is not None:
                diagnostics.record(HighsTerminationReport.from_model(model))
            return adapter.decode(model)

    @property
    def solver_input(self) -> highspy.Highs:
        """
        The HiGHS model generated from the OMMX instance.

        Returns
        -------
        highspy.Highs
            The HiGHS model ready for optimization. This model contains:
            - Decision variables translated from OMMX IDs to HiGHS indices
            - Constraints converted to HiGHS linear expressions
            - Objective function set according to optimization direction
        """
        return self.model

    def decode(self, data: highspy.Highs) -> Solution:
        """
        Convert an optimized HiGHS model back to an OMMX Solution.

        This method translates HiGHS solver results into OMMX format, including
        variable values, optimality status, and dual variable information.

        Parameters
        ----------
        data : highspy.Highs
            The HiGHS model that has been optimized. Must be the same model
            returned by solver_input property.

        Returns
        -------
        Solution
            Complete OMMX solution containing:
            - Variable values mapped back to original OMMX IDs
            - Constraint evaluations and feasibility status
            - Optimality information from HiGHS
            - Dual variables for linear constraints

        Raises
        ------
        OMMXHighsAdapterError
            If the model has not been optimized yet
        InfeasibleDetected
            If HiGHS determined the problem is infeasible
        UnboundedDetected
            If HiGHS determined the problem is unbounded

        Notes
        -----
        This method should only be used after solving the model with HiGHS.
        Any modifications to the HiGHS model structure after creation may
        make the decoding process incompatible.

        The dual variables are extracted from HiGHS's row_dual and mapped
        to OMMX constraints based on their order. Only constraints with
        valid dual information will have the dual_variable field set.

        Examples
        --------
        >>> from ommx_highs_adapter import OMMXHighsAdapter
        >>> from ommx.v1 import Instance, DecisionVariable
        >>>
        >>> x = DecisionVariable.binary(0)
        >>> instance = Instance.from_components(
        ...     decision_variables=[x],
        ...     objective=x,
        ...     constraints={},
        ...     sense=Instance.MAXIMIZE,
        ... )
        >>>
        >>> adapter = OMMXHighsAdapter(instance)
        >>> model = adapter.solver_input
        >>> model.run()  # doctest: +ELLIPSIS
        <...>
        >>> solution = adapter.decode(model)
        >>> solution.objective
        1.0
        """
        # TODO check if model is optimized
        with _tracer.start_as_current_span("decode"):
            state = self.decode_to_state(data)
            solution = self.instance.evaluate(state)

            # set optimality
            if data.getModelStatus() == highspy.HighsModelStatus.kOptimal:
                solution.optimality = Solution.OPTIMAL

            # dual variables
            solution_info = data.getSolution()
            row_dual = solution_info.row_dual
            row_dual_len = len(row_dual)

            for constraint_id in solution.constraint_ids:
                if constraint_id < row_dual_len:
                    solution.set_dual_variable(constraint_id, row_dual[constraint_id])

            return solution

    def decode_to_state(self, data: highspy.Highs) -> State:
        """
        Extract variable values from an optimized HiGHS model as an OMMX State.

        Parameters
        ----------
        data : highspy.Highs
            The optimized HiGHS model

        Returns
        -------
        State
            OMMX state containing variable values mapped to original OMMX IDs

        Raises
        ------
        OMMXHighsAdapterError
            If the model has not been optimized
        InfeasibleDetected
            If the model is infeasible
        UnboundedDetected
            If the model is unbounded

        Examples
        --------
        >>> from ommx_highs_adapter import OMMXHighsAdapter
        >>> from ommx.v1 import Instance, DecisionVariable
        >>>
        >>> x1 = DecisionVariable.integer(1, lower=0, upper=5)
        >>> instance = Instance.from_components(
        ...     decision_variables=[x1],
        ...     objective=x1,
        ...     constraints={},
        ...     sense=Instance.MINIMIZE,
        ... )
        >>> adapter = OMMXHighsAdapter(instance)
        >>> model = adapter.solver_input
        >>> model.run()  # doctest: +ELLIPSIS
        <...>
        >>> state = adapter.decode_to_state(model)
        >>> state.entries
        {1: 0.0}
        """
        status = data.getModelStatus()
        if status == highspy.HighsModelStatus.kNotset:
            raise OMMXHighsAdapterError("Model has not been optimized")
        elif status == highspy.HighsModelStatus.kInfeasible:
            raise InfeasibleDetected("Model was infeasible")
        elif status == highspy.HighsModelStatus.kUnbounded:
            raise UnboundedDetected("Model was unbounded")

        solution = data.getSolution()
        return State(
            entries={
                var.id: solution.col_value[i]
                for i, var in enumerate(self.instance.used_decision_variables)
            }
        )

    def _set_decision_variables(self):
        num_cols = len(self.instance.used_decision_variables)
        lower = np.zeros(num_cols)
        upper = np.zeros(num_cols)
        types = []
        var_ids = []

        for i, var in enumerate(self.instance.used_decision_variables):
            var_ids.append(var.id)
            if var.kind == DecisionVariable.BINARY:
                lower[i] = 0
                upper[i] = 1
                types.append(highspy.HighsVarType.kInteger)
            elif var.kind == DecisionVariable.INTEGER:
                lower[i] = var.bound.lower
                upper[i] = var.bound.upper
                types.append(highspy.HighsVarType.kInteger)
            elif var.kind == DecisionVariable.CONTINUOUS:
                lower[i] = var.bound.lower
                upper[i] = var.bound.upper
                types.append(highspy.HighsVarType.kContinuous)
            else:
                raise OMMXHighsAdapterError(
                    f"Unsupported decision variable kind: "
                    f"id: {var.id}, kind: {var.kind}"
                )
        self.highs_vars = self.model.addVariables(
            var_ids, lb=lower.tolist(), ub=upper.tolist(), type=types
        )

    def _linear_expr_conversion(self, ommx_func: Function):
        # NOTE we explicityly don't convert to `highspy.highs.highs_linear_expression`
        # before returning as the callers want to check whether the returned
        # value is a constant float.
        if ommx_func.degree() >= 2:
            raise OMMXHighsAdapterError(
                "HiGHS Adapter currently only supports linear problems"
            )
        return (
            sum(
                coeff * self.highs_vars[id]
                for (id, coeff) in ommx_func.linear_terms.items()
            )
            + ommx_func.constant_term
        )

    def _set_objective(self):
        obj = self._linear_expr_conversion(self.instance.objective)
        if isinstance(obj, float):
            return
        if self.instance.sense == Instance.MAXIMIZE:
            self.model.maximize(highs_linear_expression(obj))
        elif self.instance.sense == Instance.MINIMIZE:
            self.model.minimize(highs_linear_expression(obj))
        else:
            raise OMMXHighsAdapterError(f"Unsupported sense: {self.instance.sense}")

    def _set_constraints(self):
        for cid, constr in self.instance.constraints.items():
            const_expr = self._linear_expr_conversion(constr.function)
            if isinstance(const_expr, float):
                val = const_expr
                if constr.equality == Constraint.EQUAL_TO_ZERO:
                    if abs(val) > 1e-10:
                        raise OMMXHighsAdapterError(
                            "Infeasible constant equality constraint"
                        )
                    continue
                elif constr.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO:
                    if val > 1e-10:
                        raise OMMXHighsAdapterError(
                            "Infeasible constant inequality constraint"
                        )
                    continue
            else:
                const_expr = highs_linear_expression(const_expr)
                if constr.equality == Constraint.EQUAL_TO_ZERO:
                    self.model.addConstr(const_expr == 0, str(cid))
                elif constr.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO:
                    self.model.addConstr(const_expr <= 0, str(cid))
                else:
                    raise OMMXHighsAdapterError(
                        f"Unsupported constraint equality kind: {constr.equality}"
                    )
