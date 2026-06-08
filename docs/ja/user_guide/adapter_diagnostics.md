# Adapter 固有 diagnostics

どの Solver Adapter でも、求解結果として返す OMMX 側の型は
{class}`~ommx.v1.Solution` です。これは Adapter 共通の出力であり、どの backend
solver で解いた場合でも、decode 済みの OMMX state、feasibility、optimality、
objective value を同じ形で扱うためのものです。

diagnostics はこれとは意図的に別の枠組みです。diagnostics は、共通の
{class}`~ommx.v1.Solution` contract には入らない solver 側の詳しい情報を保持するための
adapter 固有の仕組みです。例えば backend の termination status、primal / dual bound、
gap、実行時間、node 数、solution pool、adapter 固有の warning などが該当します。
そのため、diagnostics の形と意味は adapter と backend solver ごとに定義されます。

共通の OMMX 結果が必要な場合は {class}`~ommx.v1.Solution` を参照してください。
backend solver が求解中に何を観測し、何を報告し、どこまで証明したかを確認したい場合に
diagnostics を使います。

共通の入口は {meth}`~ommx.adapter.SolverAdapter.solve` の予約済み `diagnostics`
keyword です。adapter は {class}`~ommx.adapter.DiagnosticsSink` を受け取り、
backend 固有の dataclass report を
{meth}`DiagnosticsSink.record() <ommx.adapter.DiagnosticsSink.record>` で記録します。
どの report type を出力するかは adapter ごとに決まります。追加情報がない adapter は
sink に何も記録しなくても構いません。

## 直接 solve して diagnostics を取得する

adapter を直接呼ぶ場合は、`ommx.adapter` から export されている
`DiagnosticCollector` を diagnostics sink として渡します。collector には adapter が
記録した typed diagnostic report instance がそのまま保存されます。

以下は PySCIPOpt Adapter の例です。現在の PySCIPOpt Adapter は
{class}`~ommx_pyscipopt_adapter.SCIPTerminationReport` を 1 つ記録します。

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

`collector.diagnostics` は list です。adapter は複数の diagnostic report を記録でき、
具体的な item type は adapter 固有です。

## Experiment に diagnostics を保存する

{py:meth}`~ommx.experiment.Run.log_solve` を使う場合、ユーザー側から
`diagnostics` keyword を渡さないでください。この keyword は `Run.log_solve` が予約しており、
adapter に diagnostics sink を渡して、記録された diagnostics を Experiment Artifact の
Solve entry に保存します。

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

Experiment から {py:attr}`~ommx.experiment.Solve.diagnostics` で読み出した
diagnostics は、元の dataclass instance ではなく dictionary の list として返ります。
これにより、保存済み Artifact は求解時に使われた Python class 定義から独立して読めます。

## PySCIPOpt Adapter: SCIPTerminationReport

PySCIPOpt Adapter は、SCIP 側の termination summary として
{class}`~ommx_pyscipopt_adapter.SCIPTerminationReport` を出力します。現在の adapter は、
`model.optimize()` が終了した後、PySCIPOpt model を OMMX Solution に decode する前に
1 つの report を記録します。このため、decode の段階で
{exc}`~ommx.adapter.InfeasibleDetected` や
{exc}`~ommx.adapter.UnboundedDetected` などの adapter exception が発生する場合でも、
SCIP の終了 report を確認できます。

{class}`~ommx_pyscipopt_adapter.SCIPTerminationReport` は
{meth}`OMMXPySCIPOptAdapter.solve(..., diagnostics=...) <ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter.solve>`
が出力します。

| Field | 意味 |
|---|---|
| {attr}`~ommx_pyscipopt_adapter.SCIPTerminationReport.status` | `"optimal"`、`"infeasible"`、`"unbounded"` などの SCIP termination status。 |
| {attr}`~ommx_pyscipopt_adapter.SCIPTerminationReport.primal_bound` | 終了時点で SCIP が報告した primal bound。 |
| {attr}`~ommx_pyscipopt_adapter.SCIPTerminationReport.dual_bound` | 終了時点で SCIP が報告した dual bound。 |
| {attr}`~ommx_pyscipopt_adapter.SCIPTerminationReport.gap` | `getGap()` が返す SCIP の relative gap。 |
| {attr}`~ommx_pyscipopt_adapter.SCIPTerminationReport.objective_value` | SCIP の incumbent objective value。解が見つかっていない場合は `None`。 |
| {attr}`~ommx_pyscipopt_adapter.SCIPTerminationReport.node_count` | SCIP が処理した branch-and-bound node 数。 |
| {attr}`~ommx_pyscipopt_adapter.SCIPTerminationReport.solution_count` | SCIP が保持している solution 数。 |
| {attr}`~ommx_pyscipopt_adapter.SCIPTerminationReport.solving_time_sec` | SCIP の求解時間、秒単位。 |
| {attr}`~ommx_pyscipopt_adapter.SCIPTerminationReport.scip_version` | PySCIPOpt 経由で使われた SCIP version。 |
| {attr}`~ommx_pyscipopt_adapter.SCIPTerminationReport.pyscipopt_version` | PySCIPOpt package version。取得できない場合は `None`。 |

bound と gap は SCIP から直接取得した値です。time limit などで最適性が証明されていない場合や、
OMMX Solution に decode できなかった場合に、SCIP がどこまで証明していたかを確認するために使えます。

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

Experiment から読み出した report は dictionary として表現されます。

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

実際の値は instance、SCIP、PySCIPOpt の version に依存します。
