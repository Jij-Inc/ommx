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
diagnostics with {meth}`DiagnosticsSink.record() <ommx.adapter.DiagnosticsSink.record>`.
Each adapter decides which diagnostic types it emits, and adapters that have no
extra information may leave the sink empty.

Adapters may call `record()` during the solve, including from backend solver
callbacks. A collector can therefore receive progress events before the final
termination report, while Experiment storage still writes one diagnostics BLOB
for each Solve.

## Collect Diagnostics Directly

When calling an adapter directly, pass `DiagnosticCollector` from `ommx.adapter`
as the diagnostics sink. The collector stores typed diagnostic report instances
exactly as the adapter records them.

The following example uses the PySCIPOpt Adapter, which records
{class}`~ommx_pyscipopt_adapter.SCIPProgressSnapshot` whenever SCIP emits a
tracked progress event and then records one
{class}`~ommx_pyscipopt_adapter.SCIPTerminationReport`.

```python
from ommx.adapter import DiagnosticCollector
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter, SCIPTerminationReport

collector = DiagnosticCollector()

solution = OMMXPySCIPOptAdapter.solve(
    instance,
    diagnostics=collector,
)

report = collector.diagnostics[-1]
assert isinstance(report, SCIPTerminationReport)

print(report.status)
print(report.primal_bound, report.dual_bound, report.gap)
```

`collector.diagnostics` is a list because an adapter may record multiple
diagnostic events and reports. The concrete item types are adapter-specific.

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

## PySCIPOpt Adapter Diagnostics

When diagnostics are requested, the PySCIPOpt Adapter attaches a SCIP event
handler before `model.optimize()`. It currently listens for `BESTSOLFOUND` and
`DUALBOUNDIMPROVED` events and records one
{class}`~ommx_pyscipopt_adapter.SCIPProgressSnapshot` for each observed event.
Each snapshot is a model-state sample taken inside the SCIP event callback.

SCIP may call a `BESTSOLFOUND` callback before every aggregate model statistic
has been updated. Treat each snapshot as the model state visible from that SCIP
callback, and use the final
{class}`~ommx_pyscipopt_adapter.SCIPTerminationReport` for terminal values.

The PySCIPOpt Adapter records the final
{class}`~ommx_pyscipopt_adapter.SCIPTerminationReport` after
`model.optimize()` finishes and before the PySCIPOpt model is decoded back into
an OMMX Solution. This means the report is available even when the subsequent
decode step raises an adapter exception such as
{exc}`~ommx.adapter.InfeasibleDetected` or
{exc}`~ommx.adapter.UnboundedDetected`.

See the API Reference for the complete diagnostic entry schemas:

- {class}`~ommx_pyscipopt_adapter.SCIPProgressSnapshot`
- {class}`~ommx_pyscipopt_adapter.SCIPTerminationReport`
- {class}`~ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer`

For post-solve analysis, use
{class}`~ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer` over either the typed
collector contents or dictionaries loaded from an Experiment:

```python
from ommx_pyscipopt_adapter import SCIPDiagnosticsAnalyzer

analysis = SCIPDiagnosticsAnalyzer(collector.diagnostics)

progress = analysis.progress_df()
gap_series = analysis.gap_evolution_df()
incumbents = analysis.incumbent_evolution_df()
termination = analysis.termination_report
```

The DataFrame helpers require pandas. Use `progress_records()`,
`gap_evolution_records()`, `incumbent_evolution_records()`, or
`termination_records()` when pandas is not available.

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
    report = collector.diagnostics[-1]
    assert report.status == "unbounded"
    print(report.dual_bound, report.gap)
```

When diagnostics are loaded back from an Experiment, each progress event and the
termination report are represented as dictionaries. Pass that list directly to
{class}`~ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer` when you want the same
records or DataFrame views as direct collection.
