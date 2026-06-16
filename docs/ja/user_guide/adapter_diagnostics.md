# Adapter 固有 diagnostics

Adapter diagnostics は、portable な {class}`~ommx.v1.Solution` には入らない
solver 側の情報を残すための仕組みです。decode 済みの OMMX 結果を見るときは
{class}`~ommx.v1.Solution` を使います。backend solver が何を観測し、何を報告し、
どこまで証明したかを確認したい場合に diagnostics を使います。

## PySCIPOpt で diagnostics を記録する

PySCIPOpt Adapter は、`solve()` に {class}`~ommx.adapter.DiagnosticCollector` を渡すと
SCIP の progress と termination 情報を記録します。通常は
{class}`~ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer` を通して読みます。

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
:alt: SCIP の primal bound と dual bound の推移

{class}`~ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer` で読み出した
SCIP の primal / dual bound の推移。
```

`progress_history_df` は `solving_time_sec` を index にした pandas DataFrame です。
`dual_bound`、`gap`、`incumbent_objective` などの Series property も同じ time index を使うので、
そのまま時間軸の plot に使えます。`termination_result` は最終的な SCIP report を表す dictionary です。

```python
dual_bound = analyze.dual_bound
gap = analyze.gap
incumbents = analyze.incumbent_objective
termination = analyze.termination_result
```

DataFrame / Series helper は pandas を必要とします。pandas が使えない環境では、
progress sample には `progress_history_records`、最終 report には `termination_result` を使ってください。

### PySCIPOpt が記録するもの

PySCIPOpt Adapter は 2 種類の SCIP diagnostics を記録します。

{class}`~ommx_pyscipopt_adapter.SCIPProgressSnapshot` は、SCIP event callback から記録される
progress sample です。現在は `BESTSOLFOUND` と `DUALBOUNDIMPROVED` を監視しています。
progress snapshot には `solving_time_sec`、`node_count`、`primal_bound`、`dual_bound`、
`gap`、`incumbent_objective` などが含まれます。

{class}`~ommx_pyscipopt_adapter.SCIPTerminationReport` は、`model.optimize()` が終了した後、
PySCIPOpt model を OMMX Solution に decode する前に記録される最終 report です。
`status`、`primal_bound`、`dual_bound`、`gap`、`objective_value`、node 数、LP / cut counter、
primal-dual integral、timing、SCIP / PySCIPOpt version metadata などが含まれます。

progress snapshot は callback 時点の観測値です。SCIP は `BESTSOLFOUND` callback を、
集計済みの統計がすべて更新される前に呼ぶことがあります。終了時点の値は termination report を参照してください。

完全な member list は API Reference の
{class}`~ommx_pyscipopt_adapter.SCIPProgressSnapshot`、
{class}`~ommx_pyscipopt_adapter.SCIPTerminationReport`、
{class}`~ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer` を参照してください。

## HiGHS で diagnostics を記録する

HiGHS Adapter は、`solve()` に {class}`~ommx.adapter.DiagnosticCollector` を渡すと
MIP progress と termination 情報を記録します。通常は
{class}`~ommx_highs_adapter.HighsDiagnosticsAnalyzer` を通して読みます。

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

`progress_history_df` は `running_time_sec` を index にした pandas DataFrame です。
`dual_bound`、`gap`、`primal_bound` などの Series property も同じ time index を使うので、
そのまま時間軸の plot に使えます。

{class}`~ommx_highs_adapter.HighsProgressSnapshot` は、HiGHS の MIP logging callback から
記録される progress sample です。progress snapshot には `running_time_sec`、
`mip_node_count`、`mip_primal_bound`、`mip_dual_bound`、`mip_gap` などが含まれます。

{class}`~ommx_highs_adapter.HighsTerminationReport` は、`model.run()` が終了した後、
HiGHS model を OMMX Solution に decode する前に記録される最終 report です。
`status`、`objective_value`、`mip_dual_bound`、`mip_gap`、`mip_node_count`、
iteration count、feasibility violation summary、runtime、HiGHS version metadata などが含まれます。
終了時点の scalar 値が必要な場合は、`termination_result` または `termination_*` property を使ってください。

### 失敗時の処理

直接取得は、OMMX Solution への decode が失敗する場合にも有用です。PySCIPOpt Adapter と
HiGHS Adapter は decode の前に termination report を記録するため、
{exc}`~ommx.adapter.InfeasibleDetected` や {exc}`~ommx.adapter.UnboundedDetected` などの
adapter exception が発生しても、collector には solver の最終 status や bound が残ります。

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

## Experiment 連携

{py:meth}`~ommx.experiment.Run.log_solve` を使う場合、ユーザー側から `diagnostics` keyword を
渡さないでください。この keyword は `Run.log_solve` が予約しています。diagnostics 収集は
デフォルトでは無効なので、Experiment Artifact の Solve entry に diagnostics を保存したい場合は
`store_diagnostics=True` を指定してください。このとき adapter に diagnostics sink が渡されます。

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

Experiment から {py:attr}`~ommx.experiment.Solve.diagnostics` で読み出した diagnostics は、
元の dataclass instance ではなく dictionary です。これにより、保存済み Artifact は求解時に使われた
Python class 定義から独立して読めます。直接取得した場合と同じ records / DataFrame / Series view が
必要な場合は、その list をそのまま {class}`~ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer` に渡してください。

{meth}`~ommx.adapter.SolverAdapter.solve` が OMMX Solution を返す前に例外を投げた場合でも、
可能な限り `Run.log_solve` は failed Solve entry を記録します。この entry は
`status == "failed"` または `"interrupted"`、output Solution なし、失敗前に収集済みの
diagnostics あり、という形で保存されます。diagnostics が保存されるのは `store_diagnostics=True`
の場合です。

adapter diagnostics の contract は API Reference の
{class}`~ommx.adapter.DiagnosticsSink`、
{class}`~ommx.adapter.DiagnosticCollector`、
{meth}`~ommx.adapter.SolverAdapter.solve` を参照してください。
