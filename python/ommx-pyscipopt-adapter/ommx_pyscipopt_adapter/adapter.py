from __future__ import annotations
from ommx import ConstraintID, IndicatorConstraintID, Sos1ConstraintID

import copy
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
    Equality,
    Function,
    Instance,
    InstanceClass,
    InstanceClassClause,
    Kind,
    NamedState,
    PolynomialRequirement,
    PreparationPolicy,
    Sense,
    Solution,
    SpecialConstraintKind,
    SpecialConstraintPreparation,
    State,
    ToState,
)

from .exception import OMMXPySCIPOptAdapterError

if TYPE_CHECKING:
    from pyscipopt.scip import Event as SCIPEvent

_tracer = trace.get_tracer("ommx.adapter.pyscipopt")
_SCIP_TERMINATION_EVENT = "TERMINATION"
_OBJECTIVE_AUXILIARY_NAME = "auxiliary_for_linearized_objective"

_QUADRATIC_REGULAR_CONSTRAINT_POLYNOMIAL_REQUIREMENTS = {
    Equality.EqualToZero: PolynomialRequirement.at_most(2),
    Equality.LessThanOrEqualToZero: PolynomialRequirement.at_most(2),
}
_LINEAR_INDICATOR_BODY_POLYNOMIAL_REQUIREMENTS = {
    Equality.EqualToZero: PolynomialRequirement.at_most(1),
    Equality.LessThanOrEqualToZero: PolynomialRequirement.at_most(1),
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

    Inspect why SCIP stopped and how its search progressed. A
    :class:`~ommx.Solution` evaluates a candidate against the OMMX problem;
    diagnostics report what the backend observed or proved about the formulation
    it solved. A feasible candidate alone does not establish optimality.

    .. rubric:: Collect a final report

    Install the SDK and PySCIPOpt Adapter using the
    :doc:`installation guide </getting_started/install>`. This example builds a
    small binary problem: choose at most one item and maximize its value.
    It needs neither an external dataset nor an Experiment.

    >>> from ommx import Instance
    >>> from ommx.adapter import DiagnosticCollector
    >>> from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter, SCIPDiagnosticsAnalyzer
    >>> instance = Instance.maximize()
    >>> x = instance.new_binary("x")
    >>> y = instance.new_binary("y")
    >>> instance.objective = 3 * x + 2 * y
    >>> _ = instance.add_constraint(x + y <= 1)
    >>> collector = DiagnosticCollector()
    >>> solution = OMMXPySCIPOptAdapter.solve(instance, diagnostics=collector)
    >>> analysis = SCIPDiagnosticsAnalyzer(collector.diagnostics)
    >>> solution.objective, solution.feasible
    (3.0, True)
    >>> solution.decision_variables_df()[["name", "value"]].to_dict("records")
    [{'name': 'x', 'value': 1.0}, {'name': 'y', 'value': 0.0}]
    >>> result = analysis.termination_result
    >>> assert result is not None
    >>> result["status"], result["primal_bound"], result["dual_bound"], result["gap"]
    ('optimal', 3.0, 3.0, 0.0)

    The candidate selects ``x=1, y=0``. ``status == "optimal"`` is SCIP's final
    optimality report; it is distinct from OMMX's feasibility check. A time-limited
    solve can return a feasible candidate without proving optimality.

    ``primal_bound`` is the best feasible objective bound found by the solver;
    ``dual_bound`` bounds how good the optimum can still be. For maximization,
    the primal bound is a lower bound and the dual bound an upper bound; their
    roles as upper/lower bounds reverse for minimization. ``gap`` is SCIP's
    reported relative gap, not simply their arithmetic difference. Bounds and
    gaps need not be finite when, for example, no candidate has been found.
    SCIP may also represent infinity by a large finite sentinel.

    These values concern the solver-facing objective. After sign, scale, or
    penalty changes, do not treat them as bounds on the reported OMMX objective
    without the corresponding transformation guarantee. See
    :doc:`result interpretation </user_guide/evaluate_and_interpret>`.

    .. rubric:: Read progress observations

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

    >>> history = analysis.progress_history_df
    >>> history.index.name
    'solving_time_sec'
    >>> history.iloc[-1]["event"]
    'TERMINATION'
    >>> float(analysis.dual_bound.iloc[-1]), float(analysis.gap.iloc[-1])
    (3.0, 0.0)
    >>> assert analysis.incumbent_objective.iloc[-1] == solution.objective
    >>> assert analysis.progress_history_records[-1]["event"] == "TERMINATION"

    Small problems may have few intermediate points. SCIP records
    :class:`SCIPProgressSnapshot` at ``BESTSOLFOUND`` and ``DUALBOUNDIMPROVED``
    callbacks, plus a final ``TERMINATION`` snapshot. Callbacks can run before
    aggregate statistics are updated; read terminal values from the final row
    or :class:`SCIPTerminationReport`. The report also includes node/LP/cut
    counters, primal-dual integral, timings, and solver version metadata.

    .. rubric:: Plot a recorded bound and gap history

    The following figure uses one actual run on MIPLIB ``air05``. It is an
    illustration of diagnostic output, not a performance comparison. The
    :download:`saved diagnostics </api/examples/scip_air05.json>` include the
    source model location, SDK/repository revision, platform, solver versions,
    and settings. No network access or new solve is needed to redraw it.

    Download the :download:`drawing and recording script
    </api/examples/adapter_diagnostics.py>` and JSON into the same directory.
    With matplotlib installed, run ``python adapter_diagnostics.py --data
    scip_air05.json --output diagnostics.png`` in your SDK environment. In this
    repository, use ``uv run --no-sync python
    docs/api/examples/adapter_diagnostics.py``. From the repository root, add
    ``--record`` only to acquire a new run: it downloads/caches MIPLIB data in the Local Registry and solves
    the problem again. Timings and intermediate observations can change.

    The plotting function passes the saved reports to this Analyzer and uses
    its time-indexed DataFrame. It masks NaN/Inf and SCIP's infinity sentinel;
    absent bounds do not become artificial points on the chart. Lines connect
    recorded observations, not every change made by the solver. The final SCIP
    report supplies the terminal row.

    .. literalinclude:: /api/examples/adapter_diagnostics.py
       :language: python
       :pyobject: plot

    .. figure:: /api/assets/adapter_diagnostics_bounds.png
       :alt: Recorded SCIP primal and dual bounds and relative gap over elapsed time

       One recorded SCIP run on air05. The upper panel shows solver objective
       bounds; the lower panel shows SCIP's relative gap. Acquisition conditions
       and unfiltered reports are included in the downloadable JSON.

    .. rubric:: Inspect a report when no Solution is returned

    The Adapter records termination before decoding the solver result. If
    decoding raises an adapter exception, the collector can therefore retain
    the final report. This unbounded problem has no finite optimum:

    >>> from ommx.adapter import UnboundedDetected
    >>> unbounded = Instance.maximize()
    >>> z = unbounded.new_continuous("z", lower=0)
    >>> unbounded.objective = z
    >>> collector = DiagnosticCollector()
    >>> try:
    ...     OMMXPySCIPOptAdapter.solve(unbounded, diagnostics=collector)
    ... except UnboundedDetected:
    ...     failure = SCIPDiagnosticsAnalyzer(collector.diagnostics).termination_result
    >>> assert failure is not None and failure["status"] == "unbounded"

    Errors before the backend finishes need not produce a termination report;
    :attr:`termination_result` is ``None`` when no such report was collected.

    .. rubric:: Read diagnostics saved with an Experiment

    Recording is optional. :meth:`~ommx.experiment.Run.log_solve` owns the
    ``diagnostics`` keyword; pass ``store_diagnostics=True`` instead of a
    collector. Collection is disabled by default. The following example uses
    the small model above and a temporary Local Registry.

    >>> from ommx.experiment import Experiment
    >>> with Experiment.with_temp_local_registry() as experiment:
    ...     with experiment.run() as run:
    ...         solution = run.log_solve(
    ...             OMMXPySCIPOptAdapter, instance, store_diagnostics=True,
    ...         )
    >>> stored = experiment.runs[0].solves[0].diagnostics
    >>> assert stored and all(isinstance(report, dict) for report in stored)
    >>> saved = SCIPDiagnosticsAnalyzer(stored)
    >>> assert saved.termination_result["status"] == "optimal"
    >>> assert saved.dual_bound.iloc[-1] == 3.0

    Stored reports are dictionaries rather than the original Python dataclasses.
    Both forms support the same records, DataFrame, and Series views. If an
    Adapter raises, ``log_solve`` records a failed/interrupted Solve without an
    output Solution when possible, including diagnostics collected before the
    failure. :meth:`~ommx.experiment.Run.log_sample` records sampling outcomes
    separately. Persistence is best-effort; consult those methods for failure
    handling and the reserved keyword contract.
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
    """Solve OMMX problems with SCIP through PySCIPOpt.

    Install the ``ommx-pyscipopt-adapter`` package alongside a compatible OMMX
    SDK to use these examples. :attr:`INPUT_CLASS` defines the exact supported
    input. Nonconstant linear Indicator bodies and SOS1 constraints use SCIP's
    native ``addConsIndicator`` and ``addConsSOS1`` operations. A nonconstant
    equality indicator becomes two inequality indicators. A constant body is
    simplified: a satisfied body needs no row, while an unsatisfied body forces
    its indicator to zero. OneHot constraints are lowered to regular equalities
    by the recommended preparation policy.

    :meth:`solve` prepares an isolated copy, leaving the caller's Instance
    unchanged. :meth:`solve_without_preparation` and the constructor require
    the input to satisfy :attr:`INPUT_CLASS` already.

    **Build, solve, and inspect a 0-1 knapsack problem**

    Select items to maximize their total value while keeping their total weight
    at most the capacity. Each Binary variable is one when its item is selected.
    The names and subscripts preserve the connection to the input data:

    >>> from ommx import Instance, Solution
    >>> from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter
    >>> values = [10, 13, 18, 31, 7, 15]
    >>> weights = [11, 25, 20, 35, 10, 33]
    >>> capacity = 47
    >>> problem = Instance.maximize()
    >>> items = [problem.new_binary("item", subscripts=[i]) for i in range(len(values))]
    >>> problem.objective = sum(value * item for value, item in zip(values, items))
    >>> weight_limit = problem.add_constraint(
    ...     sum(weight * item for weight, item in zip(weights, items)) <= capacity,
    ...     "Weight limit",
    ... )
    >>> solution = OMMXPySCIPOptAdapter.solve(problem)
    >>> assert solution.feasible
    >>> assert solution.optimality == Solution.OPTIMAL
    >>> selected = sorted(
    ...     i for (i,), value in solution.extract_decision_variables("item").items()
    ...     if value > 0.5
    ... )
    >>> selected
    [0, 3]
    >>> assert abs(solution.objective - sum(values[i] for i in selected)) < 1e-6
    >>> assert abs(solution.objective - 41.0) < 1e-6

    :attr:`ommx.Solution.objective` is the returned candidate's objective value;
    the value alone does not establish optimality. Here SCIP proves optimality
    for the problem being evaluated. The decision-variable table provides IDs,
    names, subscripts, and values for tabular analysis. The regular-constraint
    table reports the normalized left-hand side: total weight minus capacity.

    >>> variables = solution.decision_variables_df()
    >>> assert set(variables["name"]) == {"item"}
    >>> assert sorted(variables["value"]) == [0.0, 0.0, 0.0, 0.0, 1.0, 1.0]
    >>> selection = variables[["subscripts", "value"]].copy()
    >>> selection["choice"] = selection["value"].map(
    ...     lambda value: "Include" if value > 0.5 else "Exclude"
    ... )
    >>> assert selection["choice"].value_counts().to_dict() == {"Exclude": 4, "Include": 2}
    >>> constraints = solution.constraints_df(kind="regular")
    >>> residual = constraints.loc[weight_limit.constraint_id, "value"]
    >>> assert abs(residual - (sum(weights[i] for i in selected) - capacity)) < 1e-6
    >>> assert abs(residual - (-1.0)) < 1e-6

    The selected items weigh 46, so the inequality has one unit of slack.
    :class:`ommx.Solution` documents result extraction and evaluation semantics.
    The input can also come from a modeler, :meth:`ommx.Instance.load_mps`, or
    :class:`ommx.artifact.Artifact`; model construction is independent of solve.

    **Native Indicator and SOS1 constraints**

    This model rewards enabling a feature and selecting options. When
    ``enabled`` is one, the Indicator requires ``option_a + option_b <= 1``.
    The SOS1 allows at most one of ``option_b`` and ``option_c`` to be nonzero;
    it does not require either one to be selected. SCIP receives both
    structures directly:

    >>> from ommx import Instance, Sos1Constraint, SpecialConstraintKind
    >>> from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter
    >>> source = Instance.maximize()
    >>> enabled = source.new_binary("enabled")
    >>> option_a = source.new_binary("option_a")
    >>> option_b = source.new_binary("option_b")
    >>> option_c = source.new_binary("option_c")
    >>> source.objective = 10 * enabled + 6 * option_a + 5 * option_b + 4 * option_c
    >>> _ = source.add_indicator_constraint((option_a + option_b <= 1).with_indicator(enabled))
    >>> _ = source.add_sos1_constraint(
    ...     Sos1Constraint(variables=[option_b, option_c], name="exclusive_options")
    ... )
    >>> assert OMMXPySCIPOptAdapter.INPUT_CLASS.contains(source)
    >>> solution = OMMXPySCIPOptAdapter.solve(source)
    >>> assert solution.feasible
    >>> assert abs(solution.objective - 20.0) < 1e-6
    >>> assert source.active_special_constraint_kinds == {
    ...     SpecialConstraintKind.Indicator, SpecialConstraintKind.Sos1,
    ... }
    >>> assert not source.removed_constraints

    The selected values are ``enabled = option_a = option_c = 1`` and
    ``option_b = 0``. The :class:`ommx_highs_adapter.OMMXHighsAdapter`
    Reference solves this same model through lowering, demonstrating the
    distinction between native support and preparation for a target input class.

    **OneHot preparation without changing the source**

    Exactly one of the three binary variables must be one. The easy solve path
    lowers the OneHot constraint on its copy before encoding SCIP input:

    >>> from ommx import Instance, OneHotConstraint, SpecialConstraintKind
    >>> from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter
    >>> choice_model = Instance.maximize()
    >>> choices = [choice_model.new_binary("choice", subscripts=[i]) for i in range(3)]
    >>> choice_model.objective = 5 * choices[0] + 10 * choices[1] + 3 * choices[2]
    >>> _ = choice_model.add_one_hot_constraint(OneHotConstraint(variables=choices))
    >>> assert not OMMXPySCIPOptAdapter.INPUT_CLASS.contains(choice_model)
    >>> solution = OMMXPySCIPOptAdapter.solve(choice_model)
    >>> assert solution.feasible
    >>> assert abs(solution.objective - 10.0) < 1e-6
    >>> assert choice_model.active_special_constraint_kinds == {SpecialConstraintKind.OneHot}
    >>> assert not choice_model.removed_constraints

    **SOS1 with continuous variables**

    Native SOS1 support also applies to continuous variables: at most one may
    be nonzero, and all may be zero.

    >>> from ommx import Instance, Sos1Constraint
    >>> from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter
    >>> sparse_model = Instance.maximize()
    >>> ys = [sparse_model.new_continuous("y", lower=0, upper=10, subscripts=[i]) for i in range(3)]
    >>> sparse_model.objective = sum(ys)
    >>> _ = sparse_model.add_sos1_constraint(Sos1Constraint(variables=ys))
    >>> solution = OMMXPySCIPOptAdapter.solve(sparse_model)
    >>> assert solution.feasible
    >>> assert abs(solution.objective - 10.0) < 1e-6

    **Select a policy and use the strict solve path**

    Here the application permits only OneHot lowering and keeps SOS1 native.
    Constructing the policy does not change the model. ``prepare`` changes the
    Instance in place and stops when the target class is reached; an enabled
    operation need not run if the input already belongs to that class.

    >>> from ommx import (
    ...     Instance, OneHotConstraint, PreparationPolicy, Sos1Constraint,
    ...     SpecialConstraintKind, SpecialConstraintPreparation,
    ... )
    >>> from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter
    >>> model = Instance.maximize()
    >>> primary = [model.new_binary("primary", subscripts=[i]) for i in range(2)]
    >>> optional = [model.new_binary("optional", subscripts=[i]) for i in range(2)]
    >>> model.objective = 10 * primary[0] + 8 * primary[1] + 4 * optional[0] + 3 * optional[1]
    >>> _ = model.add_one_hot_constraint(OneHotConstraint(variables=primary))
    >>> _ = model.add_sos1_constraint(Sos1Constraint(variables=optional))
    >>> policy = PreparationPolicy(
    ...     special_constraints=SpecialConstraintPreparation.lower_special_constraints(
    ...         kinds={SpecialConstraintKind.OneHot}
    ...     )
    ... )
    >>> model.prepare(OMMXPySCIPOptAdapter.INPUT_CLASS, policy)
    >>> assert OMMXPySCIPOptAdapter.check_applicability(model).is_member
    >>> assert model.active_special_constraint_kinds == {SpecialConstraintKind.Sos1}
    >>> solution = OMMXPySCIPOptAdapter.solve_without_preparation(model)
    >>> assert solution.feasible
    >>> assert abs(solution.objective - 14.0) < 1e-6
    >>> assert len(solution.decision_variables_df()) == 4

    The strict call applies no additional preparation. Calling ``solve`` instead
    obtains a fresh recommended policy; it does not reuse the local ``policy``
    variable. A recommendation is an editable starting point, not a guarantee
    that every model can reach :attr:`INPUT_CLASS`.
    """

    INPUT_CLASS: ClassVar[InstanceClass] = InstanceClass(
        [
            InstanceClassClause(
                label="pyscipopt-quadratic-mip",
                allowed_variable_kinds={Kind.Binary, Kind.Integer, Kind.Continuous},
                objective_polynomial_requirement=PolynomialRequirement.at_most(2),
                regular_constraint_polynomial_requirements=(
                    _QUADRATIC_REGULAR_CONSTRAINT_POLYNOMIAL_REQUIREMENTS
                ),
                indicator_body_polynomial_requirements=(
                    _LINEAR_INDICATOR_BODY_POLYNOMIAL_REQUIREMENTS
                ),
                allows_sos1=True,
                allowed_senses={Sense.Minimize, Sense.Maximize},
            )
        ]
    )

    @classmethod
    def recommended_preparation_policy(cls) -> PreparationPolicy:
        """Recommend lowering OneHot constraints before using PySCIPOpt.

        PySCIPOpt accepts Indicator and SOS1 constraints directly, so this
        recommendation preserves those families and lowers only OneHot
        constraints. The easy API applies a fresh policy to an isolated copy;
        callers may instead edit and apply one before invoking
        :meth:`solve_without_preparation`.
        """
        return PreparationPolicy(
            special_constraints=SpecialConstraintPreparation.lower_special_constraints(
                kinds={SpecialConstraintKind.OneHot}
            )
        )

    def __init__(
        self,
        ommx_instance: Instance,
        *,
        initial_state: Optional[ToState | NamedState] = None,
    ):
        """
        :param ommx_instance: The ommx.Instance to solve.
        :param initial_state: Optional initial solution as a State, ID-keyed
            values, or values grouped by name and tuple subscripts. Named values
            are resolved against this instance with ``state_from_named_values``.
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
        initial_state: Optional[ToState | NamedState] = None,
        diagnostics: DiagnosticsSink | None = None,
    ) -> Solution:
        """
        Solve the given ommx.Instance using PySCIPopt, returning an ommx.Solution.

        The input instance is not modified. An isolated copy is prepared with
        the recommended PySCIPOpt policy before preparation-free execution.

        :param ommx_instance: The ommx.Instance to prepare and solve.
        :param initial_state: Optional initial solution as a State, ID-keyed
            values, or values grouped by name and tuple subscripts.

        Examples
        =========

        The :class:`OMMXPySCIPOptAdapter` class documentation contains a complete
        knapsack example from model construction through named-variable
        extraction, tabular analysis, objective value, and constraint slack.

        Infeasible Problem

        .. doctest::

                >>> from ommx import Instance, DecisionVariable, Sense
                >>> from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

                >>> x = DecisionVariable.integer(0, upper=3, lower=0)
                >>> instance = Instance.from_components(
                ...     decision_variables=[x],
                ...     objective=x,
                ...     constraints={0: x >= 4},
                ...     sense=Sense.Maximize,
                ... )

                >>> OMMXPySCIPOptAdapter.solve(instance)
                Traceback (most recent call last):
                    ...
                ommx.InfeasibleDetected: Model was infeasible

        Unbounded Problem

        .. doctest::

                >>> from ommx import Instance, DecisionVariable, Sense
                >>> from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

                >>> x = DecisionVariable.integer(0, lower=0)
                >>> instance = Instance.from_components(
                ...     decision_variables=[x],
                ...     objective=x,
                ...     constraints={},
                ...     sense=Sense.Maximize,
                ... )

                >>> OMMXPySCIPOptAdapter.solve(instance)
                Traceback (most recent call last):
                    ...
                ommx.adapter.UnboundedDetected: Model was unbounded
        """
        prepared = copy.copy(ommx_instance)
        prepared.prepare(cls.INPUT_CLASS, cls.recommended_preparation_policy())
        return cls.solve_without_preparation(
            prepared,
            initial_state=initial_state,
            diagnostics=diagnostics,
        )

    @classmethod
    def solve_without_preparation(
        cls,
        ommx_instance: Instance,
        *,
        initial_state: Optional[ToState | NamedState] = None,
        diagnostics: DiagnosticsSink | None = None,
    ) -> Solution:
        """Solve an exact PySCIPOpt Adapter input without preparing it."""
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
            return adapter.decode(model)

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

        Backend optimality is mapped through the instance's output-objective
        semantics. It remains unspecified when active-formulation optimality
        does not transport to that objective.

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
            ...     sense=Sense.Maximize,
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
                solution.optimality = self.instance.map_active_optimality(
                    Solution.OPTIMAL
                )

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
            ...     sense=Sense.Minimize,
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

        sol = data.getBestSol()
        if sol is None:
            raise OMMXPySCIPOptAdapterError(
                f"There is no feasible solution. [status: {data.getStatus()}]"
            )

        try:
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
            if var.kind == Kind.Binary:
                self.model.addVar(name=str(var.id), vtype="B")
            elif var.kind == Kind.Integer:
                self.model.addVar(
                    name=str(var.id), vtype="I", lb=var.bound.lower, ub=var.bound.upper
                )
            elif var.kind == Kind.Continuous:
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
        if degree is None or degree > 2:
            raise OMMXPySCIPOptAdapterError(
                f"Objective function degree {degree} is not supported. "
                "Only constant, linear, and quadratic objectives are supported."
            )
        if degree == 2:
            # If objective function is quadratic, add the auxiliary variable for the linearized objective function,
            # because the setObjective method in PySCIPOpt does not support quadratic objective functions.
            self.model.addVar(
                name=_OBJECTIVE_AUXILIARY_NAME, vtype="C", lb=None, ub=None
            )

        self.varname_map = {var.name: var for var in self.model.getVars()}

    def _set_objective(self):
        if self.instance.sense == Sense.Maximize:
            sense = "maximize"
        elif self.instance.sense == Sense.Minimize:
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
            auxiliary_var = self.varname_map[_OBJECTIVE_AUXILIARY_NAME]

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
        for cid in self.instance.constraints:
            if isinstance(cid, ConstraintID):
                self._set_regular_constraint(cid)
            elif isinstance(cid, Sos1ConstraintID):
                self._set_sos1_constraint(cid)
            elif isinstance(cid, IndicatorConstraintID):
                self._set_indicator_constraint(cid)
            else:
                raise OMMXPySCIPOptAdapterError(
                    f"Unsupported constraint family: {type(cid).__name__} (id: {cid})"
                )

    def _set_sos1_constraint(self, sos1_id: Sos1ConstraintID):
        sos1 = self.instance.constraints[sos1_id]
        name = f"sos1_{sos1_id}"
        vars = [self.varname_map[str(v)] for v in sos1.variables]
        self.model.addConsSOS1(vars, name=name)

    def _set_regular_constraint(self, cid: ConstraintID):
        constraint = self.instance.constraints[cid]
        # Handle constraint function based on its type
        f = constraint.function
        degree = f.degree()
        if degree == 0:
            # Constant constraint is not passed to SCIP, but checked for feasibility
            constant_value = f.constant_term
            if constraint.equality == Equality.EqualToZero and math.isclose(
                constant_value, 0, abs_tol=1e-6
            ):
                return  # Skip feasible constant constraint
            elif (
                constraint.equality == Equality.LessThanOrEqualToZero
                and constant_value <= 1e-6
            ):
                return  # Skip feasible constant constraint
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

        if constraint.equality == Equality.EqualToZero:
            constr_expr = expr == 0
        elif constraint.equality == Equality.LessThanOrEqualToZero:
            constr_expr = expr <= 0
        else:
            raise OMMXPySCIPOptAdapterError(
                f"Not supported constraint equality: "
                f"id: {cid}, equality: {constraint.equality}"
            )

        self.model.addCons(constr_expr, name=str(cid))

    def _set_indicator_constraint(self, ind_id: IndicatorConstraintID):
        indicator = self.instance.constraints[ind_id]
        f = indicator.function
        degree = f.degree()
        if degree == 0:
            # Constant indicator constraint: check feasibility statically
            # When indicator is ON, the constant constraint must hold
            constant_value = f.constant_term
            is_feasible = (
                indicator.equality == Equality.EqualToZero
                and math.isclose(constant_value, 0, abs_tol=1e-6)
            ) or (
                indicator.equality == Equality.LessThanOrEqualToZero
                and constant_value <= 1e-6
            )
            if is_feasible:
                return  # Always feasible, skip
            # If infeasible when indicator is ON, add indicator constraint
            # that forces indicator to be 0
            binvar = self.varname_map[str(indicator.indicator_variable_id)]
            self.model.addCons(binvar == 0, name=f"ind_{ind_id}_forced_off")
            return
        elif degree == 1:
            expr = self._make_linear_expr(f)
        else:
            raise OMMXPySCIPOptAdapterError(
                f"Indicator constraints must be linear. id: {ind_id}, degree: {degree}"
            )

        binvar = self.varname_map[str(indicator.indicator_variable_id)]

        if indicator.equality == Equality.EqualToZero:
            # Decompose f(x) == 0 into two indicator constraints
            self.model.addConsIndicator(
                expr <= 0, binvar=binvar, name=f"ind_{ind_id}_le"
            )
            self.model.addConsIndicator(
                -expr <= 0, binvar=binvar, name=f"ind_{ind_id}_ge"
            )
        elif indicator.equality == Equality.LessThanOrEqualToZero:
            self.model.addConsIndicator(expr <= 0, binvar=binvar, name=f"ind_{ind_id}")
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

    def _add_initial_state(self, initial_state: ToState | NamedState) -> None:
        if isinstance(initial_state, Mapping) and any(
            isinstance(key, str) for key in initial_state
        ):
            state = self.instance.state_from_named_values(
                cast(NamedState, initial_state)
            )
        else:
            state = State(cast(ToState, initial_state))
        values = {str(var_id): value for var_id, value in state.entries.items()}
        if _OBJECTIVE_AUXILIARY_NAME in self.varname_map:
            # SCIP initializes omitted coordinates to zero. Its objective
            # auxiliary must describe that same complete initial point.
            objective_state = {
                var.id: values.get(str(var.id), 0.0)
                for var in self.instance.used_decision_variables
            }
            values[_OBJECTIVE_AUXILIARY_NAME] = self.instance.objective.evaluate(
                objective_state
            )
        initial_sol = self.model.createSol()
        for var_name, value in values.items():
            if var_name in self.varname_map:
                self.model.setSolVal(initial_sol, self.varname_map[var_name], value)
        # The free=True parameter means that solution will be freed afterwards.
        self.model.addSol(initial_sol, free=True)
