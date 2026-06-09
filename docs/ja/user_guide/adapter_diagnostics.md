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
backend 固有の dataclass diagnostics を
{meth}`DiagnosticsSink.record() <ommx.adapter.DiagnosticsSink.record>` で記録します。
どの diagnostic type を出力するかは adapter ごとに決まります。追加情報がない adapter は
sink に何も記録しなくても構いません。

adapter は solve 中、backend solver の callback 内から `record()` を呼ぶことがあります。
そのため collector は最終的な termination report の前に progress event を受け取れます。
一方、Experiment への保存は 1 Solve あたり 1 つの diagnostics BLOB として行われます。

## 直接 solve して diagnostics を取得する

adapter を直接呼ぶ場合は、`ommx.adapter` から export されている
`DiagnosticCollector` を diagnostics sink として渡します。collector には adapter が
記録した typed diagnostic report instance がそのまま保存されます。

以下は PySCIPOpt Adapter の例です。PySCIPOpt Adapter は、SCIP が監視対象の
progress event を出すたびに {class}`~ommx_pyscipopt_adapter.SCIPProgressSnapshot`
を記録し、その後に
{class}`~ommx_pyscipopt_adapter.SCIPTerminationReport` を 1 つ記録します。

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

`collector.diagnostics` は list です。adapter は複数の diagnostic event や report を記録でき、
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

Solve entry には adapter class name と adapter options がすでに記録されています。
そのため diagnostics は Python type annotation なしで保存されます。どの analyzer に渡すかは
Solve の adapter metadata から判断し、例えば PySCIPOpt Adapter の diagnostics は
{class}`~ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer` に渡します。

現状の Experiment 保存では、{meth}`~ommx.adapter.SolverAdapter.solve` が OMMX Solution
を返した後にだけ diagnostics が保存されます。solve 中または decode 中に adapter が例外を投げた場合、
direct collector にすでに記録された diagnostics は呼び出し側で確認できますが、Experiment の
Solve entry としては保存されません。

## PySCIPOpt Adapter diagnostics

diagnostics が要求された場合、PySCIPOpt Adapter は `model.optimize()` の前に SCIP の
event handler を登録します。現在は `BESTSOLFOUND` と `DUALBOUNDIMPROVED` event を
監視し、観測された event ごとに
{class}`~ommx_pyscipopt_adapter.SCIPProgressSnapshot` を 1 つ記録します。各 snapshot は
SCIP event callback の中で見えている model state です。

SCIP は `BESTSOLFOUND` callback を、集計済みの model 統計がすべて更新される前に呼ぶことがあります。
各 snapshot はその callback から見えている model state として扱い、終了時点の値は
{class}`~ommx_pyscipopt_adapter.SCIPTerminationReport` を参照してください。

PySCIPOpt Adapter は、`model.optimize()` が終了した後、PySCIPOpt model を OMMX
Solution に decode する前に最終的な
{class}`~ommx_pyscipopt_adapter.SCIPTerminationReport` を記録します。このため、
direct {class}`~ommx.adapter.DiagnosticCollector` を渡している場合は、decode の段階で
{exc}`~ommx.adapter.InfeasibleDetected` や
{exc}`~ommx.adapter.UnboundedDetected` などの adapter exception が発生する場合でも、
SCIP の終了 report を確認できます。

diagnostic entry の完全な schema は API Reference を参照してください。

- {class}`~ommx_pyscipopt_adapter.SCIPProgressSnapshot`
- {class}`~ommx_pyscipopt_adapter.SCIPTerminationReport`
- {class}`~ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer`

solve 後の解析には、typed collector の中身にも Experiment から読み出した dictionary にも
{class}`~ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer` を使えます。

```python
from ommx_pyscipopt_adapter import SCIPDiagnosticsAnalyzer

analysis = SCIPDiagnosticsAnalyzer(collector.diagnostics)

progress = analysis.progress_df()
gap_series = analysis.gap_evolution_df()
incumbents = analysis.incumbent_evolution_df()
termination = analysis.termination_report
```

DataFrame helper は pandas を必要とします。pandas が使えない環境では
`progress_records()`、`gap_evolution_records()`、`incumbent_evolution_records()`、
`termination_records()` を使ってください。

bound と gap は SCIP から直接取得した値です。time limit などで最適性が証明されていない場合や、
OMMX Solution に decode できなかった場合に、SCIP がどこまで証明していたかを確認するために使えます。

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

Experiment から読み出した diagnostics では、各 progress event と termination report は
dictionary として表現されます。直接取得した場合と同じ records / DataFrame view が必要な場合は、
その list をそのまま {class}`~ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer` に渡してください。
