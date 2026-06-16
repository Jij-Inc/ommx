# Adapter-specific Diagnostics

Adapter diagnostics preserve solver-side information that does not fit in the
portable {class}`~ommx.v1.Solution`. Use {class}`~ommx.v1.Solution` for the
decoded OMMX result. Use diagnostics when you need to inspect what the backend
solver observed, reported, or proved.

## Record Diagnostics with the PySCIPOpt Adapter

The PySCIPOpt Adapter records SCIP progress and termination information when you
pass a {class}`~ommx.adapter.DiagnosticCollector` to `solve()`. The usual way to
read that data is through
{class}`~ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer`.

```python
from ommx import adapter, dataset
from ommx_pyscipopt_adapter import (
    OMMXPySCIPOptAdapter as Adapter,
    SCIPDiagnosticsAnalyzer,
)

instance = dataset.miplib2017("air05")

diag = adapter.DiagnosticCollector()
solution = Adapter.solve(instance, diagnostics=diag)

analyze = SCIPDiagnosticsAnalyzer(diag.diagnostics)

analyze.progress_history_df[["primal_bound", "dual_bound"]].loc[5:].plot()
```

```{figure} ./assets/adapter_diagnostics_bounds.png
:alt: SCIP primal and dual bound history over solving time

SCIP primal and dual bound history read through
{class}`~ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer`.
```

`progress_history_df` is a pandas DataFrame indexed by `solving_time_sec`.
Series properties such as `dual_bound`, `gap`, and `incumbent_objective` use the
same time index, so they are ready for time-based plots. `termination_result` is
a dictionary containing the final SCIP report.

```python
dual_bound = analyze.dual_bound
gap = analyze.gap
incumbents = analyze.incumbent_objective
termination = analyze.termination_result
```

The DataFrame and Series helpers require pandas. When pandas is not available,
use `progress_history_records` for progress samples and `termination_result` for
the final report.

### What PySCIPOpt Records

The PySCIPOpt Adapter records two kinds of SCIP diagnostics.

{class}`~ommx_pyscipopt_adapter.SCIPProgressSnapshot` is a progress sample
recorded from SCIP event callbacks. The adapter currently listens for
`BESTSOLFOUND` and `DUALBOUNDIMPROVED`. A progress snapshot includes fields such
as `solving_time_sec`, `node_count`, `primal_bound`, `dual_bound`, `gap`, and
`incumbent_objective`.

{class}`~ommx_pyscipopt_adapter.SCIPTerminationReport` is the final SCIP report
recorded after `model.optimize()` finishes and before the PySCIPOpt model is
decoded back into an OMMX Solution. It includes fields such as `status`,
`primal_bound`, `dual_bound`, `gap`, `objective_value`, node counts, LP and cut
counters, primal-dual integral, timings, and SCIP/PySCIPOpt version metadata.

Progress snapshots are callback-time observations. SCIP may call a
`BESTSOLFOUND` callback before every aggregate statistic has been updated, so
use the termination report for terminal values.

For the complete member lists, see the API Reference for
{class}`~ommx_pyscipopt_adapter.SCIPProgressSnapshot`,
{class}`~ommx_pyscipopt_adapter.SCIPTerminationReport`, and
{class}`~ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer`.

## Record Diagnostics with the HiGHS Adapter

The HiGHS Adapter records MIP progress and termination information when you
pass a {class}`~ommx.adapter.DiagnosticCollector` to `solve()`. Read that data
through {class}`~ommx_highs_adapter.HighsDiagnosticsAnalyzer`.

```python
from ommx import adapter
from ommx_highs_adapter import OMMXHighsAdapter, HighsDiagnosticsAnalyzer

diag = adapter.DiagnosticCollector()
solution = OMMXHighsAdapter.solve(instance, diagnostics=diag)

analysis = HighsDiagnosticsAnalyzer(diag.diagnostics)

analysis.progress_history_df[["mip_primal_bound", "mip_dual_bound"]].plot()
print(analysis.dual_bound)
print(analysis.termination_result)
```

`progress_history_df` is a pandas DataFrame indexed by `running_time_sec`.
Series properties such as `dual_bound`, `gap`, and `primal_bound` use the same
time index, so they are ready for time-based plots.

{class}`~ommx_highs_adapter.HighsProgressSnapshot` is recorded from HiGHS MIP
logging callbacks. A progress snapshot includes fields such as
`running_time_sec`, `mip_node_count`, `mip_primal_bound`, `mip_dual_bound`, and
`mip_gap`.

{class}`~ommx_highs_adapter.HighsTerminationReport` is recorded after
`model.run()` finishes and before the HiGHS model is decoded back into an OMMX
Solution. It includes fields such as `status`, `objective_value`,
`mip_dual_bound`, `mip_gap`, `mip_node_count`, iteration counts, feasibility
violation summaries, runtime, and HiGHS version metadata. Use
`termination_result` or the `termination_*` properties when you need terminal
scalar values.

### Failure Handling

Direct collection is useful when OMMX Solution decoding fails. The PySCIPOpt
and HiGHS Adapters record the termination report before decoding, so the
collector can still contain the final solver status and bounds when the solve
raises an adapter exception such as {exc}`~ommx.adapter.InfeasibleDetected` or
{exc}`~ommx.adapter.UnboundedDetected`.

```python
from ommx.adapter import DiagnosticCollector, UnboundedDetected
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter, SCIPDiagnosticsAnalyzer

collector = DiagnosticCollector()

try:
    OMMXPySCIPOptAdapter.solve(instance, diagnostics=collector)
except UnboundedDetected:
    analysis = SCIPDiagnosticsAnalyzer(collector.diagnostics)
    print(analysis.termination_result)
```

## Experiment Integration

When using {py:meth}`~ommx.experiment.Run.log_solve`, do not pass the
`diagnostics` keyword yourself. `Run.log_solve` owns that reserved keyword,
and diagnostics collection is disabled by default. Set
`store_diagnostics=True` to pass a diagnostics sink to the adapter and store
recorded diagnostics with the Solve entry in the Experiment Artifact.

```python
from ommx.experiment import Experiment
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter, SCIPDiagnosticsAnalyzer

with Experiment() as experiment:
    with experiment.run() as run:
        solution = run.log_solve(
            OMMXPySCIPOptAdapter,
            instance,
            store_diagnostics=True,
        )

solve = experiment.runs[0].solves[0]
analysis = SCIPDiagnosticsAnalyzer(solve.diagnostics)

print(analysis.dual_bound)
print(analysis.termination_result)
```

Diagnostics loaded from an Experiment through
{py:attr}`~ommx.experiment.Solve.diagnostics` are dictionaries, not the original
dataclass instances. This keeps stored Artifacts independent of the Python class
definitions used when the solve was recorded. Pass that list directly to
{class}`~ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer` when you want the same
records, DataFrame, or Series views as direct collection.

If {meth}`~ommx.adapter.SolverAdapter.solve` raises before returning an OMMX
Solution, `Run.log_solve` still records a failed Solve entry when possible. That
entry has `status == "failed"` or `"interrupted"`, no output Solution, and any
diagnostics collected before the failure when `store_diagnostics=True`.

See the API Reference for the adapter diagnostics contract:
{class}`~ommx.adapter.DiagnosticsSink`,
{class}`~ommx.adapter.DiagnosticCollector`, and
{meth}`~ommx.adapter.SolverAdapter.solve`.
