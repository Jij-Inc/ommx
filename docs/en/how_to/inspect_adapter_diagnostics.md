# Inspect Adapter termination, bounds, and gaps

Adapter diagnostics retain solver-side information that is not part of a portable {class}`~ommx.Solution`. Use Solution for the decoded OMMX result and diagnostics to inspect what the backend observed, reported, and established.

This page assumes you can read a result's objective and feasibility. Install the SDK and PySCIPOpt Adapter using the [installation guide](../getting_started/install.md). The first example constructs a small problem locally; no data download or Experiment is required.

## Read termination information for a small problem

Pass a {class}`~ommx.adapter.DiagnosticCollector` to PySCIPOpt's `solve()` to collect SCIP progress and termination information. Read it with {class}`~ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer`.

```python
from ommx import Instance, adapter
from ommx_pyscipopt_adapter import (
    OMMXPySCIPOptAdapter as Adapter,
    SCIPDiagnosticsAnalyzer,
)

instance = Instance.maximize()
x = instance.new_binary("x")
y = instance.new_binary("y")
instance.objective = 3 * x + 2 * y
instance.add_constraint(x + y <= 1)

diag = adapter.DiagnosticCollector()
solution = Adapter.solve(instance, diagnostics=diag)

analyze = SCIPDiagnosticsAnalyzer(diag.diagnostics)

print(solution.objective, solution.feasible)
print(analyze.termination_result)
```

Solution evaluates a candidate against the OMMX problem. `termination_result` reports how the backend stopped; these are not the same information. A time-limited solve may still provide a feasible candidate, and feasibility does not establish optimality.

Bounds and gaps describe the formulation actually solved. If transformations change the objective's sign, scale, or penalties, do not directly compare these with OMMX's reported objective. See [Compare Adapters](compare_adapters.md) for common axes and [Transformation guarantees](../user_guide/transformation_guarantees.md) for correspondence.

## Plot progress

A small problem may finish before producing many intermediate observations. The following optional example needs network access to public MIPLIB data and matplotlib for plotting. Collection works the same way.

```python
from ommx import dataset

instance = dataset.miplib2017("air05")
diag = adapter.DiagnosticCollector()
solution = Adapter.solve(instance, diagnostics=diag)
analyze = SCIPDiagnosticsAnalyzer(diag.diagnostics)
analyze.progress_history_df[["primal_bound", "dual_bound"]].loc[5:].plot()
```

```{figure} ../user_guide/assets/adapter_diagnostics_bounds.png
:alt: SCIP primal and dual bounds over time

SCIP primal and dual bounds read with
{class}`~ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer`.
```

`progress_history_df` is a pandas DataFrame indexed by `solving_time_sec`. Series properties such as `dual_bound`, `gap`, and `incumbent_objective` use the same time index and can be plotted directly. When the diagnostics contain a SCIP termination report, the history includes a final `TERMINATION` row. `termination_result` is a dictionary of the final SCIP report.

```python
dual_bound = analyze.dual_bound
gap = analyze.gap
incumbents = analyze.incumbent_objective
termination = analyze.termination_result
```

DataFrame and Series helpers require pandas. Without pandas, use `progress_history_records` for the history, including a final `TERMINATION` row when present, and `termination_result` for the final report.

### What PySCIPOpt records

The Adapter records two kinds of SCIP diagnostics.

{class}`~ommx_pyscipopt_adapter.SCIPProgressSnapshot` represents progress sampled from SCIP event callbacks and the final termination report. Currently it observes `BESTSOLFOUND` and `DUALBOUNDIMPROVED`, then adds a `TERMINATION` snapshot. Fields include `solving_time_sec`, `node_count`, `primal_bound`, `dual_bound`, `gap`, and `incumbent_objective`.

{class}`~ommx_pyscipopt_adapter.SCIPTerminationReport` is recorded after `model.optimize()` finishes and before decoding the PySCIPOpt model into a Solution. It includes `status`, bounds, gap, objective, node and LP/cut counters, primal-dual integral, timing, and SCIP/PySCIPOpt version metadata.

Callback snapshots are observations at callback time. SCIP may invoke `BESTSOLFOUND` before all aggregate statistics have updated. Use the `TERMINATION` row or report for final values.

See {class}`~ommx_pyscipopt_adapter.SCIPProgressSnapshot`, {class}`~ommx_pyscipopt_adapter.SCIPTerminationReport`, and {class}`~ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer` for complete API details.

## Record HiGHS diagnostics

[Add the HiGHS Adapter](../getting_started/install.md#highs-adapter) only if using it. The following also works with the first small `instance`.

Pass a {class}`~ommx.adapter.DiagnosticCollector` to `solve()` to collect MIP progress and termination information. Read it with {class}`~ommx_highs_adapter.HighsDiagnosticsAnalyzer`.

```python
from ommx import adapter
from ommx_highs_adapter import OMMXHighsAdapter, HighsDiagnosticsAnalyzer

diag = adapter.DiagnosticCollector()
solution = OMMXHighsAdapter.solve(instance, diagnostics=diag)

analysis = HighsDiagnosticsAnalyzer(diag.diagnostics)

analysis.progress_history_df[["primal_bound", "dual_bound"]].plot()
print(analysis.dual_bound)
print(analysis.termination_result)
```

`progress_history_df` is indexed by `solving_time_sec`. Series such as `dual_bound`, `gap`, and `primal_bound` use the same index.

{class}`~ommx_highs_adapter.HighsProgressSnapshot` is sampled from HiGHS MIP logging callbacks and includes solving time, MIP node count, bounds, and gap.

{class}`~ommx_highs_adapter.HighsTerminationReport` is recorded after `model.run()` and before decoding into an OMMX Solution. It includes `status`, `objective_value`, `mip_dual_bound`, `mip_gap`, `mip_node_count`, iteration counts, feasibility-violation summaries, runtime, and HiGHS version metadata. Use `termination_result` or `termination_*` properties for final scalar values.

### Handle failures

Direct collection is also useful when decoding fails. PySCIPOpt and HiGHS record termination before decoding, so a collector can retain final status and bounds even when an Adapter raises {exc}`~ommx.adapter.InfeasibleDetected` or {exc}`~ommx.adapter.UnboundedDetected`.

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

## Optional: Store diagnostics in an Experiment

Experiment is not required for the investigation above. To retain records, start with the [Experiment tutorial](../tutorial/experiment_management).

Do not supply the `diagnostics` keyword yourself to {py:meth}`~ommx.experiment.Run.log_solve` or {py:meth}`~ommx.experiment.Run.log_sample`: those logging methods reserve it. Diagnostic storage is off by default. Set `store_diagnostics=True` to pass a sink to the Adapter and save diagnostics in the corresponding Solve or Sampling entry.

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

{py:attr}`~ommx.experiment.Solve.diagnostics` returns dictionaries, not the original dataclass instances. A stored Artifact can therefore be read independently of the Python class definitions used while solving. Pass the list directly to {class}`~ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer` for the same records, DataFrame, and Series views.

When {meth}`~ommx.adapter.SolverAdapter.solve` raises before returning a Solution, `Run.log_solve` records a failed Solve entry where possible: status `"failed"` or `"interrupted"`, no output Solution, and collected diagnostics when `store_diagnostics=True`. A sampler failure in `Run.log_sample` is recorded separately as a Sampling entry without an output SampleSet.

See {class}`~ommx.adapter.DiagnosticsSink`, {class}`~ommx.adapter.DiagnosticCollector`, {meth}`~ommx.adapter.SolverAdapter.solve`, and {meth}`~ommx.adapter.SamplerAdapter.sample` for exact contracts.
