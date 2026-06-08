# Adapter-specific Diagnostics

Every Solver Adapter returns the same OMMX-side result type:
{class}`~ommx.v1.Solution`. This is the portable output of a solve. It gives
users a common way to read the decoded OMMX state, feasibility, optimality, and
objective value regardless of which backend solver produced the result.

Diagnostics are intentionally different. They are an adapter-specific framework
for preserving detailed solver-side information that does not fit into the
common {class}`~ommx.v1.Solution` contract. Examples include backend termination
status, primal and dual bounds, gaps, timings, node counts, solution pools, or
adapter-specific warnings. The shape and meaning of diagnostics are therefore
defined by each adapter and backend solver.

Use {class}`~ommx.v1.Solution` when you need the common OMMX result. Use
diagnostics when you need to understand what the backend solver observed,
reported, or proved during the solve.

The common entry point is the reserved `diagnostics` keyword on
{meth}`~ommx.adapter.SolverAdapter.solve`. An adapter receives a
{class}`~ommx.adapter.DiagnosticsSink` and records backend-specific dataclass
reports with {meth}`DiagnosticsSink.record() <ommx.adapter.DiagnosticsSink.record>`.
Each adapter decides which report types it emits, and adapters that have no
extra information may leave the sink empty.

## Collect Diagnostics Directly

When calling an adapter directly, pass `DiagnosticCollector` from `ommx.adapter`
as the diagnostics sink. The collector stores typed diagnostic report instances
exactly as the adapter records them.

The following example uses the PySCIPOpt Adapter, which currently records one
{class}`~ommx_pyscipopt_adapter.SCIPTerminationReport`.

```python
from ommx.adapter import DiagnosticCollector
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter, SCIPTerminationReport

collector = DiagnosticCollector()

solution = OMMXPySCIPOptAdapter.solve(
    instance,
    diagnostics=collector,
)

report = collector.diagnostics[0]
assert isinstance(report, SCIPTerminationReport)

print(report.status)
print(report.primal_bound, report.dual_bound, report.gap)
```

`collector.diagnostics` is a list because an adapter may record multiple
diagnostic reports. The concrete item types are adapter-specific.

## Store Diagnostics in an Experiment

When using {py:meth}`~ommx.experiment.Run.log_solve`, do not pass the
`diagnostics` keyword yourself. `Run.log_solve` owns that reserved keyword,
passes a diagnostics sink to the adapter, and stores recorded diagnostics with
the Solve entry in the Experiment Artifact.

```python
from ommx.experiment import Experiment
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

with Experiment() as experiment:
    with experiment.run() as run:
        solution = run.log_solve(OMMXPySCIPOptAdapter, instance)

loaded_experiment = experiment
solve = loaded_experiment.runs[0].solves[0]

print(solve.diagnostics)
```

Diagnostics loaded from an Experiment through
{py:attr}`~ommx.experiment.Solve.diagnostics` are returned as a list of
dictionaries, not as the original dataclass instances. This keeps stored
Artifacts independent of the Python class definitions used when the solve was
recorded.

## PySCIPOpt Adapter: SCIPTerminationReport

The PySCIPOpt Adapter emits
{class}`~ommx_pyscipopt_adapter.SCIPTerminationReport`, a SCIP-side termination
summary. The current adapter records one report after `model.optimize()` finishes
and before the PySCIPOpt model is decoded back into an OMMX Solution. This means
the report is available even when the subsequent decode step raises an adapter
exception such as {exc}`~ommx.adapter.InfeasibleDetected` or
{exc}`~ommx.adapter.UnboundedDetected`.

{class}`~ommx_pyscipopt_adapter.SCIPTerminationReport` is emitted by
{meth}`OMMXPySCIPOptAdapter.solve(..., diagnostics=...) <ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter.solve>`.

| Field | Meaning |
|---|---|
| {attr}`~ommx_pyscipopt_adapter.SCIPTerminationReport.status` | SCIP termination status, such as `"optimal"`, `"infeasible"`, or `"unbounded"`. |
| {attr}`~ommx_pyscipopt_adapter.SCIPTerminationReport.primal_bound` | SCIP primal bound at termination. |
| {attr}`~ommx_pyscipopt_adapter.SCIPTerminationReport.dual_bound` | SCIP dual bound at termination. |
| {attr}`~ommx_pyscipopt_adapter.SCIPTerminationReport.gap` | SCIP relative gap reported by `getGap()`. |
| {attr}`~ommx_pyscipopt_adapter.SCIPTerminationReport.objective_value` | SCIP incumbent objective value, or `None` if SCIP found no solution. |
| {attr}`~ommx_pyscipopt_adapter.SCIPTerminationReport.node_count` | Number of branch-and-bound nodes processed by SCIP. |
| {attr}`~ommx_pyscipopt_adapter.SCIPTerminationReport.solution_count` | Number of solutions stored by SCIP. |
| {attr}`~ommx_pyscipopt_adapter.SCIPTerminationReport.solving_time_sec` | SCIP solving time in seconds. |
| {attr}`~ommx_pyscipopt_adapter.SCIPTerminationReport.scip_version` | SCIP version used through PySCIPOpt. |
| {attr}`~ommx_pyscipopt_adapter.SCIPTerminationReport.pyscipopt_version` | PySCIPOpt package version, if available. |

The bounds and gap come directly from SCIP. They are useful for understanding a
time-limited or otherwise non-optimal termination, and for checking what SCIP had
proved when no OMMX Solution could be decoded.

```python
from ommx.adapter import DiagnosticCollector, UnboundedDetected
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

collector = DiagnosticCollector()

try:
    OMMXPySCIPOptAdapter.solve(instance, diagnostics=collector)
except UnboundedDetected:
    report = collector.diagnostics[0]
    assert report.status == "unbounded"
    print(report.dual_bound, report.gap)
```

When the report is loaded back from an Experiment, it is represented as a
dictionary:

```python
[
    {
        "status": "optimal",
        "primal_bound": 42.0,
        "dual_bound": 42.0,
        "gap": 0.0,
        "objective_value": 42.0,
        "node_count": 1,
        "solution_count": 1,
        "solving_time_sec": 0.01,
        "scip_version": "9.2.1",
        "pyscipopt_version": "6.0.0",
    }
]
```

The exact values depend on the instance, SCIP, and PySCIPOpt versions.
