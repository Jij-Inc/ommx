# Adapterの終了理由・bound・gapを調べる

Adapter diagnosticsは、ソルバーの終了理由や探索の進捗を読むための記録です。
候補の目的値や実行可能性は{class}`~ommx.Solution`で読み、ソルバー側の報告を
調べたいときにdiagnosticsを使います。

このページは、求解結果の目的値や実行可能性を読めるユーザー向けです。
[インストール手順](../getting_started/install.md)のSDKとPySCIPOpt Adapterを用意してください。
最初の例は小さなモデルをこの場で作るため、データのダウンロードやExperimentは不要です。

## 小さな問題で終了理由を読む

PySCIPOpt Adapter は、`solve()` に {class}`~ommx.adapter.DiagnosticCollector` を渡すと
SCIP の progress と termination 情報を記録します。通常は
{class}`~ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer` を通して読みます。

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

この問題の最適な候補は`x=1, y=0`で、目的値は3です。`termination_result`の
`status`が`"optimal"`なら、SCIPは最適性を確認して終了したと報告しています。

`Solution`はOMMXの問題に対する候補の評価です。`termination_result`はソルバーが
どのように終了したかの報告で、同じ情報ではありません。時間制限で終了しても候補は
実行可能な場合があり、候補が実行可能でも最適性が確定したとは限りません。

boundやgapは、探索がどこまで進んだかを読む手がかりです。

- `primal_bound`：ソルバーが見つけた最良の実行可能な候補側のbound。
- `dual_bound`：ソルバーが証明した、これより良い最適値は存在しないという側のbound。
- `gap`：両者の隔たりを表すソルバーの報告値。具体的な計算方法はソルバーによって異なります。

例えば最大化問題では、候補の目的値が最適値の下側、証明されたboundが上側を押さえます。
最小化問題では上下が逆です。候補がまだ見つかっていない場合などは、有限なboundやgapが
得られるとは限りません。

これらはソルバーが実際に解いた定式化に関する値です。変換で目的関数の符号や
スケール、ペナルティが変わる場合、OMMXの報告目的値とそのまま比較しないでください。
比較の軸は[Adapterの比較](compare_adapters.md)、対応関係の判断は
[変換の保証](../user_guide/transformation_guarantees.md)を参照してください。

## 進捗を図にする

小さな問題はすぐ終了するため、進捗の途中点がほとんどないことがあります。
次は任意の発展例です。公開MIPLIBデータを取得するネットワーク接続と、図を描く
matplotlibが必要です。記録の仕組みは先ほどと同じです。

```python
from ommx import dataset

progress_instance = dataset.miplib2017("air05")
diag = adapter.DiagnosticCollector()
solution = Adapter.solve(progress_instance, diagnostics=diag)
analyze = SCIPDiagnosticsAnalyzer(diag.diagnostics)
analyze.progress_history_df[["primal_bound", "dual_bound"]].loc[5:].plot()
```

```{figure} ../user_guide/assets/adapter_diagnostics_bounds.png
:alt: SCIP の primal bound と dual bound の推移

{class}`~ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer` で読み出した
SCIP の primal / dual bound の推移。
```

横軸はソルバーの経過時間（秒）です。進捗の記録は途中時点の観測なので、
終了時点の値を読みたい場合は`termination_result`を使ってください。
pandasを使わずに途中の記録を読む場合は`progress_history_records`を使えます。

記録するイベントや各field、DataFrame・Seriesの詳細はAPI Referenceの
{class}`~ommx_pyscipopt_adapter.SCIPProgressSnapshot`、
{class}`~ommx_pyscipopt_adapter.SCIPTerminationReport`、
{class}`~ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer` を参照してください。

## HiGHS で diagnostics を記録する

HiGHSを使う場合だけ{ref}`HiGHS Adapterを追加 <install-highs-adapter>`します。
以下は最初の小さなモデルの`instance`を使います。

HiGHS Adapter は、`solve()` に {class}`~ommx.adapter.DiagnosticCollector` を渡すと
MIP progress と termination 情報を記録します。通常は
{class}`~ommx_highs_adapter.HighsDiagnosticsAnalyzer` を通して読みます。

```python
from ommx import adapter
from ommx_highs_adapter import OMMXHighsAdapter, HighsDiagnosticsAnalyzer

diag = adapter.DiagnosticCollector()
solution = OMMXHighsAdapter.solve(instance, diagnostics=diag)

analysis = HighsDiagnosticsAnalyzer(diag.diagnostics)

print(analysis.termination_result)
```

小さな問題では途中の記録がほとんどなくても、終了時の報告を確認できます。
終了理由やboundのfield名はSCIPと同一とは限りません。読み出せる情報は
{class}`~ommx_highs_adapter.HighsTerminationReport`、途中の観測は
{class}`~ommx_highs_adapter.HighsProgressSnapshot`、読み出し方は
{class}`~ommx_highs_adapter.HighsDiagnosticsAnalyzer`を参照してください。

## Solutionを返せなかったときの報告を読む

直接取得は、OMMX Solution への decode が失敗する場合にも有用です。PySCIPOpt Adapter と
HiGHS Adapter は decode の前に termination report を記録するため、
{exc}`~ommx.adapter.InfeasibleDetected` や {exc}`~ommx.adapter.UnboundedDetected` などの
adapter exception が発生しても、collector には solver の最終 status や bound が残ります。

```python
from ommx import Instance
from ommx.adapter import DiagnosticCollector, UnboundedDetected
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter, SCIPDiagnosticsAnalyzer

unbounded_instance = Instance.maximize()
z = unbounded_instance.new_continuous("z", lower=0)
unbounded_instance.objective = z

collector = DiagnosticCollector()
try:
    OMMXPySCIPOptAdapter.solve(unbounded_instance, diagnostics=collector)
except UnboundedDetected:
    analysis = SCIPDiagnosticsAnalyzer(collector.diagnostics)
    print(analysis.termination_result)
```

## 任意：Experimentに記録する

ここまでの調査にはExperimentは必要ありません。記録も残したい場合は
[Experiment Tutorial](../tutorial/experiment_management)で記録先の作り方を確認してください。

{py:meth}`~ommx.experiment.Run.log_solve` または
{py:meth}`~ommx.experiment.Run.log_sample` を使う場合、ユーザー側から `diagnostics` keyword を
渡さないでください。この keyword は logging method が予約しています。diagnostics 収集は
デフォルトでは無効なので、Experiment Artifact の対応するSolveまたはSampling entryにdiagnosticsを
保存したい場合は `store_diagnostics=True` を指定してください。このときadapterにdiagnostics sinkが渡されます。

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

保存した{py:attr}`~ommx.experiment.Solve.diagnostics`も同じAnalyzerで読めます。
Adapterが例外を投げた場合も、可能な限り失敗の記録と収集済みdiagnosticsが残ります。
保存はbest-effortなので、必ず残る保証ではありません。保存対象や失敗時の扱いは
{py:meth}`~ommx.experiment.Run.log_solve`と{py:meth}`~ommx.experiment.Run.log_sample`を参照してください。

adapter diagnostics の contract は API Reference の
{class}`~ommx.adapter.DiagnosticsSink`、
{class}`~ommx.adapter.DiagnosticCollector`、
{meth}`~ommx.adapter.SolverAdapter.solve` または
{meth}`~ommx.adapter.SamplerAdapter.sample` を参照してください。
