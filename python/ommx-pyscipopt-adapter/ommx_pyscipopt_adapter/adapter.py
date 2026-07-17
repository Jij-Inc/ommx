from __future__ import annotations
import math
from dataclasses import asdict, dataclass, fields
from typing import TYPE_CHECKING, Any, ClassVar, Iterable, Mapping, Optional, cast

import pyscipopt
from pyscipopt.scip import PY_SCIP_EVENTTYPE as SCIP_EVENTTYPE

from opentelemetry import trace

from ommx.adapter import (
    DiagnosticsSink,
    SolverAdapter,
    InfeasibleDetected,
    UnboundedDetected,
    NoSolutionReturned,
)
from ommx import (
    Constraint,
    DecisionVariable,
    DegreeBound,
    Equality,
    Function,
    Instance,
    InstanceClass,
    InstanceClassClause,
    Kind,
    Sense,
    Solution,
    State,
    ToState,
)

from .exception import OMMXPySCIPOptAdapterError

if TYPE_CHECKING:
    from pyscipopt.scip import Event as SCIPEvent

_tracer = trace.get_tracer("ommx.adapter.pyscipopt")
_SCIP_TERMINATION_EVENT = "TERMINATION"

_QUADRATIC_REGULAR_CONSTRAINT_DEGREE_BOUNDS = {
    Equality.EqualToZero: DegreeBound.at_most(2),
    Equality.LessThanOrEqualToZero: DegreeBound.at_most(2),
}
_LINEAR_INDICATOR_CONSTRAINT_DEGREE_BOUNDS = {
    Equality.EqualToZero: DegreeBound.at_most(1),
    Equality.LessThanOrEqualToZero: DegreeBound.at_most(1),
}


@dataclass(frozen=True, slots=True)
class SCIPTerminationReport:
    """SCIP-side termination summary recorded after ``model.optimize()``.

    The PySCIPOpt adapter records this report before decoding the optimized
    SCIP model back into an OMMX solution. It is therefore available even when
    decoding raises an adapter exception such as infeasible or unbounded
    detection.
    """

    status: str
    """SCIP termination status, such as ``"optimal"``, ``"infeasible"``, or
    ``"unbounded"``.
    """

    primal_bound: float
    """SCIP primal bound at termination."""

    dual_bound: float
    """SCIP dual bound at termination."""

    gap: float
    """SCIP relative gap reported by ``getGap()``."""

    objective_value: float | None
    """Incumbent objective value, or ``None`` when SCIP has no solution."""

    node_count: int
    """Number of branch-and-bound nodes processed by SCIP."""

    total_node_count: int
    """Total processed nodes including restarts."""

    lp_iteration_count: int
    """Total LP iterations."""

    lp_solve_count: int
    """Number of solved LPs."""

    cut_count: int
    """Number of cuts available in SCIP's cut pool."""

    applied_cut_count: int
    """Number of cuts applied by SCIP."""

    solution_count: int
    """Number of solutions stored by SCIP at termination."""

    solution_found_count: int
    """Number of solutions SCIP found during the solve."""

    best_solution_count: int
    """Number of new incumbent solutions SCIP found."""

    max_depth: int
    """Maximum branch-and-bound depth.

    SCIP may report ``-1`` when no branching occurred.
    """

    primal_dual_integral: float
    """SCIP primal-dual integral at termination."""

    solving_time_sec: float
    """SCIP solving time in seconds."""

    presolving_time_sec: float
    """SCIP presolving time in seconds."""

    reading_time_sec: float
    """SCIP reading time in seconds."""

    scip_version: str
    """SCIP version used through PySCIPOpt."""

    pyscipopt_version: str | None
    """PySCIPOpt package version, if available."""

    @classmethod
    def from_model(cls, model: pyscipopt.Model) -> SCIPTerminationReport:
        status = str(model.getStatus())
        if status == "unknown":
            raise OMMXPySCIPOptAdapterError(
                "The model may not be optimized. [status: unknown]"
            )
        solution_count = int(model.getNSols())
        return cls(
            status=status,
            primal_bound=model.getPrimalbound(),
            dual_bound=model.getDualbound(),
            gap=model.getGap(),
            objective_value=model.getObjVal() if solution_count > 0 else None,
            node_count=int(model.getNNodes()),
            total_node_count=int(model.getNTotalNodes()),
            lp_iteration_count=int(model.getNLPIterations()),
            lp_solve_count=int(model.getNLPs()),
            cut_count=int(model.getNCuts()),
            applied_cut_count=int(model.getNCutsApplied()),
            solution_count=solution_count,
            solution_found_count=int(model.getNSolsFound()),
            best_solution_count=int(model.getNBestSolsFound()),
            max_depth=int(model.getMaxDepth()),
            primal_dual_integral=model.getPrimalDualIntegral(),
            solving_time_sec=model.getSolvingTime(),
            presolving_time_sec=model.getPresolvingTime(),
            reading_time_sec=model.getReadingTime(),
            scip_version=(
                f"{model.getMajorVersion()}.{model.getMinorVersion()}.{model.getTechVersion()}"
            ),
            pyscipopt_version=getattr(pyscipopt, "__version__", None),
        )


@dataclass(frozen=True, slots=True)
class SCIPProgressSnapshot:
    """SCIP solve progress observed during a solve.

    The PySCIPOpt adapter records this snapshot for each tracked SCIP event and
    once more from the termination report after ``model.optimize()`` finishes.
    It currently listens for ``BESTSOLFOUND`` and ``DUALBOUNDIMPROVED``. Event
    snapshots are the model state visible from that callback. SCIP may call a
    ``BESTSOLFOUND`` callback before every aggregate model statistic has been
    updated, so use the ``TERMINATION`` snapshot or
    :class:`SCIPTerminationReport` for terminal values.
    """

    event: str
    """Progress marker.

    Callback snapshots use SCIP event names such as ``"BESTSOLFOUND"`` and
    ``"DUALBOUNDIMPROVED"``. The terminal snapshot uses the synthetic
    ``"TERMINATION"`` marker.
    """

    solving_time_sec: float
    """SCIP solving time when the snapshot was recorded."""

    node_count: int
    """Processed branch-and-bound nodes at the snapshot."""

    total_node_count: int
    """Total processed nodes including restarts at the snapshot."""

    lp_iteration_count: int
    """LP iterations at the snapshot."""

    solution_count: int
    """Number of solutions stored by SCIP at the snapshot."""

    primal_bound: float
    """SCIP primal bound reported at the snapshot."""

    dual_bound: float
    """SCIP dual bound reported at the snapshot."""

    gap: float
    """SCIP relative gap reported at the snapshot."""

    incumbent_objective: float | None
    """Objective value of SCIP's current best solution.

    This is ``None`` when PySCIPOpt cannot read an incumbent objective at that
    snapshot.
    """

    @classmethod
    def from_event(
        cls, model: pyscipopt.Model, event: SCIPEvent
    ) -> SCIPProgressSnapshot:
        solution_count = int(model.getNSols())
        primal_bound = model.getPrimalbound()
        return cls(
            event=event.getName(),
            solving_time_sec=model.getSolvingTime(),
            node_count=int(model.getNNodes()),
            total_node_count=int(model.getNTotalNodes()),
            lp_iteration_count=int(model.getNLPIterations()),
            solution_count=solution_count,
            primal_bound=primal_bound,
            dual_bound=model.getDualbound(),
            gap=model.getGap(),
            incumbent_objective=_get_incumbent_objective(model, solution_count),
        )

    @classmethod
    def from_termination_report(
        cls, report: SCIPTerminationReport
    ) -> SCIPProgressSnapshot:
        return cls(
            event=_SCIP_TERMINATION_EVENT,
            solving_time_sec=report.solving_time_sec,
            node_count=report.node_count,
            total_node_count=report.total_node_count,
            lp_iteration_count=report.lp_iteration_count,
            solution_count=report.solution_count,
            primal_bound=report.primal_bound,
            dual_bound=report.dual_bound,
            gap=report.gap,
            incumbent_objective=report.objective_value,
        )


class _SCIPDiagnosticsEventHandler(pyscipopt.Eventhdlr):
    def __init__(self, diagnostics: DiagnosticsSink) -> None:
        super().__init__()
        self.diagnostics = diagnostics

    def eventinit(self) -> None:
        self.model.catchEvent(SCIP_EVENTTYPE.BESTSOLFOUND, self)
        self.model.catchEvent(SCIP_EVENTTYPE.DUALBOUNDIMPROVED, self)

    def eventexit(self) -> None:
        self.model.dropEvent(SCIP_EVENTTYPE.BESTSOLFOUND, self)
        self.model.dropEvent(SCIP_EVENTTYPE.DUALBOUNDIMPROVED, self)

    def eventexec(self, event: SCIPEvent) -> None:
        self.diagnostics.record(SCIPProgressSnapshot.from_event(self.model, event))


def _get_incumbent_objective(
    model: pyscipopt.Model, solution_count: int
) -> float | None:
    if solution_count <= 0:
        return None
    solution = model.getBestSol()
    if solution is None:
        return None
    return model.getSolObjVal(solution)


class SCIPDiagnosticsAnalyzer:
    """Pandas-like post-processor for PySCIPOpt diagnostics.

    The analyzer accepts either typed diagnostics collected by
    :class:`ommx.adapter.DiagnosticCollector` or dictionaries loaded from
    :attr:`ommx.experiment.Solve.diagnostics`.

    :attr:`progress_history_records` returns ``list[dict[str, object]]`` and
    does not require pandas. When diagnostics include a terminal SCIP report,
    the progress history includes a final ``TERMINATION`` row derived from that
    report. :attr:`progress_history_df` returns a pandas DataFrame indexed by
    ``solving_time_sec`` and imports pandas lazily. Time-series properties such
    as :attr:`dual_bound` return pandas Series with the same index.
    :attr:`termination_result` returns the terminal SCIP report as a dictionary.
    """

    _progress_snapshots: tuple[SCIPProgressSnapshot, ...]
    _progress_history_records: tuple[dict[str, object], ...]
    _termination_result: dict[str, object] | None

    def __init__(self, diagnostics: Iterable[Any]) -> None:
        progress_snapshots: list[SCIPProgressSnapshot] = []
        progress_history_records: list[dict[str, object]] = []
        termination_results: list[dict[str, object]] = []

        for diagnostic in diagnostics:
            if isinstance(diagnostic, SCIPProgressSnapshot):
                progress_snapshots.append(diagnostic)
            if (record := _as_progress_history_record(diagnostic)) is not None:
                progress_history_records.append(record)
            if (result := _as_termination_result(diagnostic)) is not None:
                termination_results.append(result)

        termination_result = termination_results[-1] if termination_results else None
        if termination_result is not None:
            terminal_record = _progress_record_from_termination_result(
                termination_result
            )
            if not progress_history_records or not _progress_history_records_equal(
                progress_history_records[-1], terminal_record
            ):
                progress_history_records.append(terminal_record)

        self._progress_snapshots = tuple(progress_snapshots)
        self._progress_history_records = tuple(progress_history_records)
        self._termination_result = termination_result

    @property
    def progress_snapshots(self) -> tuple[SCIPProgressSnapshot, ...]:
        """Typed progress snapshots in the order they were recorded."""
        return self._progress_snapshots

    @property
    def progress_history_records(self) -> list[dict[str, object]]:
        """Return SCIP progress history, with ``TERMINATION`` when present."""
        return [dict(record) for record in self._progress_history_records]

    @property
    def progress_history_df(self) -> Any:
        """Return progress history as a pandas DataFrame indexed by time."""
        return _dataframe(
            self.progress_history_records,
            _dataclass_field_names(SCIPProgressSnapshot),
            index="solving_time_sec",
        )

    @property
    def event(self) -> Any:
        """Return progress event names indexed by solving time."""
        return self._progress_series("event")

    @property
    def node_count(self) -> Any:
        """Return processed node counts indexed by solving time."""
        return self._progress_series("node_count")

    @property
    def total_node_count(self) -> Any:
        """Return total processed node counts indexed by solving time."""
        return self._progress_series("total_node_count")

    @property
    def lp_iteration_count(self) -> Any:
        """Return LP iteration counts indexed by solving time."""
        return self._progress_series("lp_iteration_count")

    @property
    def solution_count(self) -> Any:
        """Return stored solution counts indexed by solving time."""
        return self._progress_series("solution_count")

    @property
    def primal_bound(self) -> Any:
        """Return primal bounds indexed by solving time."""
        return self._progress_series("primal_bound")

    @property
    def dual_bound(self) -> Any:
        """Return dual bounds indexed by solving time."""
        return self._progress_series("dual_bound")

    @property
    def gap(self) -> Any:
        """Return relative gaps indexed by solving time."""
        return self._progress_series("gap")

    @property
    def incumbent_objective(self) -> Any:
        """Return incumbent objectives indexed by solving time."""
        return self._progress_series("incumbent_objective")

    @property
    def termination_result(self) -> dict[str, object] | None:
        """Return the terminal SCIP report as one dictionary, if present."""
        if self._termination_result is None:
            return None
        return dict(self._termination_result)

    def _progress_series(self, column: str) -> Any:
        return self.progress_history_df[column]


def _as_progress_history_record(diagnostic: Any) -> dict[str, object] | None:
    if isinstance(diagnostic, SCIPProgressSnapshot):
        return cast(dict[str, object], asdict(diagnostic))
    if not isinstance(diagnostic, Mapping):
        return None
    if not _has_dataclass_fields(diagnostic, SCIPProgressSnapshot):
        return None
    return _select_dataclass_fields(diagnostic, SCIPProgressSnapshot)


def _as_termination_result(diagnostic: Any) -> dict[str, object] | None:
    if isinstance(diagnostic, SCIPTerminationReport):
        return cast(dict[str, object], asdict(diagnostic))
    if not isinstance(diagnostic, Mapping):
        return None
    if not _has_dataclass_fields(diagnostic, SCIPTerminationReport):
        return None
    return _select_dataclass_fields(diagnostic, SCIPTerminationReport)


def _progress_record_from_termination_result(
    result: Mapping[str, object],
) -> dict[str, object]:
    return {
        "event": _SCIP_TERMINATION_EVENT,
        "solving_time_sec": result["solving_time_sec"],
        "node_count": result["node_count"],
        "total_node_count": result["total_node_count"],
        "lp_iteration_count": result["lp_iteration_count"],
        "solution_count": result["solution_count"],
        "primal_bound": result["primal_bound"],
        "dual_bound": result["dual_bound"],
        "gap": result["gap"],
        "incumbent_objective": result["objective_value"],
    }


def _progress_history_records_equal(
    left: Mapping[str, object],
    right: Mapping[str, object],
) -> bool:
    for name in _dataclass_field_names(SCIPProgressSnapshot):
        if name not in left or name not in right:
            return False
        if not _diagnostic_values_equal(left[name], right[name]):
            return False
    return True


def _diagnostic_values_equal(left: object, right: object) -> bool:
    if _is_nan(left) and _is_nan(right):
        return True
    return left == right


def _is_nan(value: object) -> bool:
    return isinstance(value, float) and math.isnan(value)


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
            "pandas is required for SCIPDiagnosticsAnalyzer DataFrame and Series "
            "properties. Use progress_history_records without pandas."
        ) from error
    dataframe = pd.DataFrame.from_records(records, columns=columns)
    if "incumbent_objective" in dataframe:
        dataframe["incumbent_objective"] = dataframe["incumbent_objective"].astype(
            "Float64"
        )
    if index is not None:
        dataframe = dataframe.set_index(index)
    return dataframe


class OMMXPySCIPOptAdapter(SolverAdapter):
    INPUT_CLASS: ClassVar[InstanceClass | None] = InstanceClass(
        [
            InstanceClassClause(
                label="pyscipopt-quadratic-mip",
                allowed_variable_kinds={Kind.Binary, Kind.Integer, Kind.Continuous},
                objective_degree_bound=DegreeBound.at_most(2),
                regular_constraint_degree_bounds=(
                    _QUADRATIC_REGULAR_CONSTRAINT_DEGREE_BOUNDS
                ),
                indicator_constraint_degree_bounds=(
                    _LINEAR_INDICATOR_CONSTRAINT_DEGREE_BOUNDS
                ),
                allows_sos1=True,
                allowed_senses={Sense.Minimize, Sense.Maximize},
            )
        ]
    )

    def __init__(
        self,
        ommx_instance: Instance,
        *,
        initial_state: Optional[ToState] = None,
    ):
        """
        :param ommx_instance: The ommx.Instance to solve.
        :param initial_state: Optional initial solution state.
        """
        with _tracer.start_as_current_span("convert"):
            self.require_applicable(ommx_instance)
            self.instance = ommx_instance
            self.model = pyscipopt.Model()
            self.model.hideOutput()

            self._set_decision_variables()
            self._set_objective()
            self._set_constraints()

            # Add initial solution if provided
            if initial_state is not None:
                self._add_initial_state(initial_state)

    @classmethod
    def solve(
        cls,
        ommx_instance: Instance,
        *,
        initial_state: Optional[ToState] = None,
        diagnostics: DiagnosticsSink | None = None,
    ) -> Solution:
        """
        Solve the given ommx.Instance using PySCIPopt, returning an ommx.Solution.

        :param ommx_instance: The ommx.Instance to solve.
        :param initial_state: Optional initial solution state.

        Examples
        =========

        KnapSack Problem

        .. doctest::

            >>> from ommx import Instance, DecisionVariable
            >>> from ommx import Solution
            >>> from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

            >>> p = [10, 13, 18, 32, 7, 15]
            >>> w = [11, 15, 20, 35, 10, 33]
            >>> x = [DecisionVariable.binary(i) for i in range(6)]
            >>> instance = Instance.from_components(
            ...     decision_variables=x,
            ...     objective=sum(p[i] * x[i] for i in range(6)),
            ...     constraints={0: sum(w[i] * x[i] for i in range(6)) <= 47},
            ...     sense=Instance.MAXIMIZE,
            ... )

            Solve it

            >>> solution = OMMXPySCIPOptAdapter.solve(instance)

            Check output

            >>> sorted([(id, value) for id, value in solution.state.entries.items()])
            [(0, 1.0), (1, 0.0), (2, 0.0), (3, 1.0), (4, 0.0), (5, 0.0)]
            >>> solution.feasible
            True
            >>> assert solution.optimality == Solution.OPTIMAL

            p[0] + p[3] = 42
            w[0] + w[3] = 46 <= 47

            >>> solution.objective
            42.0
            >>> solution.get_constraint_value(0)
            -1.0

        Infeasible Problem

        .. doctest::

                >>> from ommx import Instance, DecisionVariable
                >>> from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

                >>> x = DecisionVariable.integer(0, upper=3, lower=0)
                >>> instance = Instance.from_components(
                ...     decision_variables=[x],
                ...     objective=x,
                ...     constraints={0: x >= 4},
                ...     sense=Instance.MAXIMIZE,
                ... )

                >>> OMMXPySCIPOptAdapter.solve(instance)
                Traceback (most recent call last):
                    ...
                ommx.adapter.InfeasibleDetected: Model was infeasible

        Unbounded Problem

        .. doctest::

                >>> from ommx import Instance, DecisionVariable
                >>> from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

                >>> x = DecisionVariable.integer(0, lower=0)
                >>> instance = Instance.from_components(
                ...     decision_variables=[x],
                ...     objective=x,
                ...     constraints={},
                ...     sense=Instance.MAXIMIZE,
                ... )

                >>> OMMXPySCIPOptAdapter.solve(instance)
                Traceback (most recent call last):
                    ...
                ommx.adapter.UnboundedDetected: Model was unbounded
        """
        with _tracer.start_as_current_span("solve") as span:
            span.set_attribute("adapter", f"{cls.__module__}.{cls.__qualname__}")
            adapter = cls(ommx_instance, initial_state=initial_state)
            model = adapter.solver_input
            if diagnostics is not None:
                progress_handler = _SCIPDiagnosticsEventHandler(diagnostics)
                model.includeEventhdlr(
                    progress_handler,
                    "ommx_diagnostics",
                    "Collect SCIP progress diagnostics for OMMX",
                )
            with _tracer.start_as_current_span("call"):
                model.optimize()
            if diagnostics is not None:
                termination_report = SCIPTerminationReport.from_model(model)
                diagnostics.record(
                    SCIPProgressSnapshot.from_termination_report(termination_report)
                )
                diagnostics.record(termination_report)
            solution = adapter.decode(model)
            return solution

    @property
    def solver_input(self) -> pyscipopt.Model:
        """The PySCIPOpt model generated from this OMMX instance"""
        return self.model

    def decode(self, data: pyscipopt.Model) -> Solution:
        """Convert optimized pyscipopt.Model and ommx.Instance to ommx.Solution.

        This method is intended to be used if the model has been acquired with
        `solver_input` for further adjustment of the solver parameters, and
        separately optimizing the model.

        Note that alterations to the model may make the decoding process
        incompatible -- decoding will only work if the model still describes
        effectively the same problem as the OMMX instance used to create the
        adapter.

        Examples
        =========

        .. doctest::

            >>> from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter
            >>> from ommx import Instance, DecisionVariable

            >>> p = [10, 13, 18, 32, 7, 15]
            >>> w = [11, 15, 20, 35, 10, 33]
            >>> x = [DecisionVariable.binary(i) for i in range(6)]
            >>> instance = Instance.from_components(
            ...     decision_variables=x,
            ...     objective=sum(p[i] * x[i] for i in range(6)),
            ...     constraints={0: sum(w[i] * x[i] for i in range(6)) <= 47},
            ...     sense=Instance.MAXIMIZE,
            ... )

            >>> adapter = OMMXPySCIPOptAdapter(instance)
            >>> model = adapter.solver_input
            >>> # ... some modification of model's parameters
            >>> model.optimize()

            >>> solution = adapter.decode(model)
            >>> solution.objective
            42.0

        """

        # TODO: Add the feature to store dual variables in `solution`.

        with _tracer.start_as_current_span("decode"):
            state = self.decode_to_state(data)
            solution = self.instance.evaluate(state)

            if (
                data.getStatus() == "optimal"
            ):  # pyscipopt does not appear to have an enum or constant for this
                solution.optimality = Solution.OPTIMAL

            return solution

    def decode_to_state(self, data: pyscipopt.Model) -> State:
        """
        Create an ommx.State from an optimized PySCIPOpt Model.

        Examples
        =========

        .. doctest::

            The following example shows how to solve an unconstrained linear optimization problem with `x1` as the objective function.

            >>> from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter
            >>> from ommx import Instance, DecisionVariable

            >>> x1 = DecisionVariable.integer(1, lower=0, upper=5)
            >>> ommx_instance = Instance.from_components(
            ...     decision_variables=[x1],
            ...     objective=x1,
            ...     constraints={},
            ...     sense=Instance.MINIMIZE,
            ... )
            >>> adapter = OMMXPySCIPOptAdapter(ommx_instance)
            >>> model = adapter.solver_input
            >>> model.optimize()

            >>> ommx_state = adapter.decode_to_state(model)
            >>> ommx_state.entries
            {1: 0.0}

        """
        if data.getStatus() == "unknown":
            raise OMMXPySCIPOptAdapterError(
                "The model may not be optimized. [status: unknown]"
            )

        if data.getStatus() == "infeasible":
            raise InfeasibleDetected("Model was infeasible")

        if data.getStatus() == "unbounded":
            raise UnboundedDetected("Model was unbounded")

        if data.getStatus() == "timelimit":
            # The following condition checks if there is no feasible primal solution.
            # In other words, it is checking for the absence of any feasible solution.
            if data.getNSols() == 0:
                raise NoSolutionReturned("No solution was returned [status: timelimit]")

        # NOTE: It is assumed that getBestSol will return an error
        #       if there is no feasible solution.
        try:
            sol = data.getBestSol()
            # NOTE recreating the map instead of using `self.varname_map`, as
            # this is probably more robust.
            varname_map = {var.name: var for var in data.getVars()}
            return State(
                entries={
                    var.id: sol[varname_map[str(var.id)]]
                    for var in self.instance.used_decision_variables
                }
            )
        except Exception:
            raise OMMXPySCIPOptAdapterError(
                f"There is no feasible solution. [status: {data.getStatus()}]"
            )

    def _set_decision_variables(self):
        for var in self.instance.used_decision_variables:
            if var.kind == DecisionVariable.BINARY:
                self.model.addVar(name=str(var.id), vtype="B")
            elif var.kind == DecisionVariable.INTEGER:
                self.model.addVar(
                    name=str(var.id), vtype="I", lb=var.bound.lower, ub=var.bound.upper
                )
            elif var.kind == DecisionVariable.CONTINUOUS:
                self.model.addVar(
                    name=str(var.id), vtype="C", lb=var.bound.lower, ub=var.bound.upper
                )
            else:
                raise OMMXPySCIPOptAdapterError(
                    f"Unsupported decision variable kind: "
                    f"id: {var.id}, kind: {var.kind}"
                )

        # Check if objective is quadratic to add auxiliary variable
        degree = self.instance.objective.degree()
        if degree > 2:
            raise OMMXPySCIPOptAdapterError(
                f"Objective function degree {degree} is not supported. "
                "Only constant, linear, and quadratic objectives are supported."
            )
        if degree == 2:
            # If objective function is quadratic, add the auxiliary variable for the linearized objective function,
            # because the setObjective method in PySCIPOpt does not support quadratic objective functions.
            self.model.addVar(
                name="auxiliary_for_linearized_objective", vtype="C", lb=None, ub=None
            )

        self.varname_map = {var.name: var for var in self.model.getVars()}

    def _set_objective(self):
        if self.instance.sense == Instance.MAXIMIZE:
            sense = "maximize"
        elif self.instance.sense == Instance.MINIMIZE:
            sense = "minimize"
        else:
            raise OMMXPySCIPOptAdapterError(
                f"Sense not supported: {self.instance.sense}"
            )

        objective = self.instance.objective

        degree = objective.degree()
        if degree == 0:
            self.model.setObjective(objective.constant_term, sense=sense)
        elif degree == 1:
            expr = self._make_linear_expr(objective)
            self.model.setObjective(expr, sense=sense)
        elif degree == 2:
            # The setObjective method in PySCIPOpt does not support quadratic objective functions.
            # So we introduce the auxiliary variable to linearize the objective function,
            # Example:
            #     input problem: min x^2 + y^2
            #
            #     introduce the auxiliary variable z, and the linearized objective function problem is:
            #         min z
            #         s.t. z >= x^2 + y^2
            auxiliary_var = self.varname_map["auxiliary_for_linearized_objective"]

            # Add the auxiliary variable to the objective function.
            self.model.setObjective(auxiliary_var, sense=sense)

            # Add the constraint for the auxiliary variable.
            expr = self._make_quadratic_expr(objective)
            if sense == "minimize":
                constr_expr = auxiliary_var >= expr
            else:  # sense == "maximize"
                constr_expr = auxiliary_var <= expr

            self.model.addCons(constr_expr, name="constraint_for_linearized_objective")
        else:
            raise OMMXPySCIPOptAdapterError(
                "The objective function must be constant, linear, or quadratic."
            )

    def _set_constraints(self):
        # Handle SOS1 constraints (first-class constraint type)
        for sos1_id, sos1 in self.instance.sos1_constraints.items():
            name = f"sos1_{sos1_id}"
            vars = [self.varname_map[str(v)] for v in sos1.variables]
            self.model.addConsSOS1(vars, name=name)

        for cid, constraint in self.instance.constraints.items():
            # Handle constraint function based on its type
            f = constraint.function
            degree = f.degree()
            if degree == 0:
                # Constant constraint is not passed to SCIP, but checked for feasibility
                constant_value = f.constant_term
                if constraint.equality == Constraint.EQUAL_TO_ZERO and math.isclose(
                    constant_value, 0, abs_tol=1e-6
                ):
                    continue  # Skip feasible constant constraint
                elif (
                    constraint.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO
                    and constant_value <= 1e-6
                ):
                    continue  # Skip feasible constant constraint
                else:
                    raise OMMXPySCIPOptAdapterError(
                        f"Infeasible constant constraint was found: id {cid}"
                    )
            elif degree == 1:
                expr = self._make_linear_expr(f)
            elif degree == 2:
                expr = self._make_quadratic_expr(f)
            else:
                raise OMMXPySCIPOptAdapterError(
                    f"Constraints must be either constant, linear or quadratic. "
                    f"id: {cid}, "
                    f"degree: {degree}"
                )

            if constraint.equality == Constraint.EQUAL_TO_ZERO:
                constr_expr = expr == 0
            elif constraint.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO:
                constr_expr = expr <= 0
            else:
                raise OMMXPySCIPOptAdapterError(
                    f"Not supported constraint equality: "
                    f"id: {cid}, equality: {constraint.equality}"
                )

            self.model.addCons(constr_expr, name=str(cid))

        # Handle indicator constraints
        for ind_id, indicator in self.instance.indicator_constraints.items():
            f = indicator.function
            degree = f.degree()
            if degree == 0:
                # Constant indicator constraint: check feasibility statically
                # When indicator is ON, the constant constraint must hold
                constant_value = f.constant_term
                is_feasible = (
                    indicator.equality == Constraint.EQUAL_TO_ZERO
                    and math.isclose(constant_value, 0, abs_tol=1e-6)
                ) or (
                    indicator.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO
                    and constant_value <= 1e-6
                )
                if is_feasible:
                    continue  # Always feasible, skip
                # If infeasible when indicator is ON, add indicator constraint
                # that forces indicator to be 0
                binvar = self.varname_map[str(indicator.indicator_variable_id)]
                self.model.addCons(binvar == 0, name=f"ind_{ind_id}_forced_off")
                continue
            elif degree == 1:
                expr = self._make_linear_expr(f)
            else:
                raise OMMXPySCIPOptAdapterError(
                    f"Indicator constraints must be linear. "
                    f"id: {ind_id}, degree: {degree}"
                )

            binvar = self.varname_map[str(indicator.indicator_variable_id)]

            if indicator.equality == Constraint.EQUAL_TO_ZERO:
                # Decompose f(x) == 0 into two indicator constraints
                self.model.addConsIndicator(
                    expr <= 0, binvar=binvar, name=f"ind_{ind_id}_le"
                )
                self.model.addConsIndicator(
                    -expr <= 0, binvar=binvar, name=f"ind_{ind_id}_ge"
                )
            elif indicator.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO:
                self.model.addConsIndicator(
                    expr <= 0, binvar=binvar, name=f"ind_{ind_id}"
                )
            else:
                raise OMMXPySCIPOptAdapterError(
                    f"Not supported indicator constraint equality: "
                    f"id: {ind_id}, equality: {indicator.equality}"
                )

    def _make_linear_expr(self, f: Function) -> pyscipopt.Expr:
        return (
            pyscipopt.quicksum(
                coeff * self.varname_map[str(id)]
                for id, coeff in f.linear_terms.items()
            )
            + f.constant_term
        )

    def _make_quadratic_expr(self, f: Function) -> pyscipopt.Expr:
        # Quadratic terms
        quad_terms = pyscipopt.quicksum(
            self.varname_map[str(row)] * self.varname_map[str(col)] * coeff
            for (row, col), coeff in f.quadratic_terms.items()
        )

        # Linear terms
        linear_terms = pyscipopt.quicksum(
            coeff * self.varname_map[str(var_id)]
            for var_id, coeff in f.linear_terms.items()
        )

        constant = f.constant_term

        return quad_terms + linear_terms + constant

    def _add_initial_state(self, initial_state: ToState) -> None:
        initial_sol = self.model.createSol()
        for var_id, value in State(initial_state).entries.items():
            var_name = str(var_id)
            if var_name in self.varname_map:
                self.model.setSolVal(initial_sol, self.varname_map[var_name], value)
        # The free=True parameter means that solution will be freed afterwards.
        self.model.addSol(initial_sol, free=True)
