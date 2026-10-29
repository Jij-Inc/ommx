---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: Python 3 (ipykernel)
  language: python
  name: python3
---

複数のAdapterで最適化問題を解いて結果を比較する
=========================================

OMMX Adapterで問題を解けるようになったら、同じInstanceを複数のAdapterへ渡して結果を比較できます。このページでは、HiGHSとPySCIPOptのどちらにも変換なしで渡せる簡単なモデルを使います。[Python SDKとAdapterのインストール](../getting_started/install.md)に従い、v3のSDK・両Adapterとmatplotlibを用意してください。

`solve()`は元のInstanceを変更しないため、1つのInstanceを再利用できます。Adapterによって必要な変換が異なる場合も、それぞれが独立したcopyを準備します。独自のPolicyを使う方法は[Preparationで変換する制約族を選ぶ](./customize_preparation.md)を参照してください。

ここでは、HiGHSとSCIPのどちらも直接受け取れる簡単なナップザック問題を考えましょう：

$$
\begin{aligned}
\mathrm{maximize} \quad & \sum_{i=0}^{N-1} v_i x_i \\
\mathrm{s.t.} \quad & \sum_{i=0}^{N-1} w_i x_i - W \leq 0, \\
& x_{i} \in \{ 0, 1\} 
\end{aligned}
$$

```{code-cell} ipython3
from ommx import Instance

v = [10, 13, 18, 31, 7, 15]
w = [11, 25, 20, 35, 10, 33]
W = 47
N = len(v)

instance = Instance.maximize()
x = [instance.new_binary("x", subscripts=[i]) for i in range(N)]
instance.objective = sum(v[i] * x[i] for i in range(N))
instance.add_constraint(
    sum(w[i] * x[i] for i in range(N)) <= W,
    "重量制限",
)
```

## 複数のソルバーで問題を解く

HiGHSとSCIPのAdapterを使って、先ほど作ったInstanceを解きます。
ほかの選択肢は[Adapterの選び方](../user_guide/supported_ommx_adapters.md)を参照してください。

```{code-cell} ipython3
from ommx_highs_adapter import OMMXHighsAdapter
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter


# 利用するAdapterの一覧
adapters = {
    "highs": OMMXHighsAdapter,
    "scip": OMMXPySCIPOptAdapter,
}

# 各Adapterを介して問題を解く
solutions = {
    name: adapter.solve(instance) for name, adapter in adapters.items()
}
```

## 結果の比較

まず、同じInstanceに対して報告された目的値と実行可能性を比べます。最大化と最小化で
良い目的値の向きが異なるため、結果の`solution.sense`も一緒に確認します。

```{code-cell} ipython3
import pandas

comparison = pandas.DataFrame([
    {
        "adapter": name,
        "sense": repr(solution.sense),
        "objective": solution.objective,
        "feasible": solution.feasible,
        "optimality": repr(solution.optimality),
    }
    for name, solution in solutions.items()
]).set_index("adapter")
comparison
```

目的値はOMMXが報告する値です。実行可能性は変数の範囲・種類と制約を含みます。
`Sense.Maximize`は最大化、`Optimality.Optimal`は最適性が確認された結果を表します。
実行可能な候補が得られたこと、目的値が一致したこと、最適性が確定したことは別々に読みます。
この例は同じ入力の比較手順を示すもので、どちらのソルバーが一般に優れるかを示しません。

### 比較してよい情報を分ける

| 情報 | 比較するときの基準 |
|---|---|
| 報告目的値・sense | 同じ問題、同じ目的関数の意味であること |
| 元の問題に対する実行可能性 | 同じ評価対象と許容誤差であること |
| optimality | 対象の問題とAdapterが表す保証を確認する |
| backendのbound・gap・終了理由 | 実際に解いた定式化・目的関数・backend固有の定義を確認する |
| 実行時間 | データ、設定、環境、繰り返し方を定めて別途測定する |

別の変換済みInstanceを明示的に解く場合は、必要に応じて候補を元のInstanceでも評価します。
値の復元や変換後のclass membershipだけでは、元の問題の実行可能性や大域的最適性は
保証されません。[変換の保証](../user_guide/transformation_guarantees.md)と
[変換後の結果](../user_guide/evaluate_and_interpret.md)で評価対象を確認してください。

boundやgapを調べる場合は[diagnosticsのHow-to](inspect_adapter_diagnostics.md)へ進みます。
特にペナルティや目的値の符号変更を含む定式化では、backendのboundをそのまま
元の目的値のboundと扱わないでください。Experimentはこの比較の前提ではありません。

### 候補の変数値を図にする

{py:meth}`~ommx.Solution.extract_decision_variables`で、名前が`x`の変数の値を取り出します。
戻り値の辞書は添字のtupleをキーに持ちます。このモデルは1次元なので、
`key[0]`がモデル作成時の添字`i`に対応します。

```{code-cell} ipython3
from matplotlib import pyplot as plt

marks = {
    "highs": "o",
    "scip": "+",
}

for name, solution in solutions.items():
    x = solution.extract_decision_variables("x")
    subscripts = [key[0] for key in x.keys()]
    plt.plot(subscripts, x.values(), marks[name], label=name)

plt.legend()
```

分析する作業によっては `decision_variables_df` で得られる `pandas.DataFrame` を縦に結合すると便利です。

```{code-cell} ipython3
import pandas

decision_variables = pandas.concat([
    solution.decision_variables_df().assign(solver=solver)
    for solver, solution in solutions.items()
])
decision_variables
```
