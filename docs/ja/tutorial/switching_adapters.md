---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: ommx-update-books (3.9.23)
  language: python
  name: python3
---

複数のAdapterで最適化問題を解いて結果を比較する
=========================================

OMMX Adapterは共通化されたAPIを持つため、同じ問題を複数のソルバーで解いて結果を比較できます。[PySCIPOpt Adapterで特殊制約をそのまま解く](./solve_special_constraints_with_pyscipopt_adapter.md)と[Adapter向けにInstanceを準備する](./prepare_instance_for_adapter.md)では、Adapterによって直接受け取れるInstanceが異なる例を見ました。このページでは、HiGHSとPySCIPOptのどちらにも変換なしで渡せる簡単なモデルを使います。

各Adapterは、変換なしで直接受け取れる入力を `INPUT_CLASS` として示します。同じ `Instance` をそのまま比較に使えるのは、そのInstanceがすべてのAdapterの `INPUT_CLASS` に入る場合です。入らない場合は、Adapterごとにコピーを作ってPreparationしてください。

ここでは、HiGHSとSCIPのどちらも直接受け取れる簡単なナップザック問題を考えましょう：

$$
\begin{aligned}
\mathrm{maximize} \quad & \sum_{i=0}^{N-1} v_i x_i \\
\mathrm{s.t.} \quad & \sum_{i=0}^{n-1} w_i x_i - W \leq 0, \\
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

ここではOMMX SDK本体と一緒に開発されているOSSへのAdapterを使いましょう。
OSSでないソルバーについてもAdapterが存在し、同じインターフェースで使う事ができます。
対応しているソルバーごとのAdapter一覧は[Supported Adapters](../user_guide/supported_ommx_adapters.md)にまとめられています。



ここではOSSのHiGHSとSCIPのAdapterを使ってナップザック問題を解いてみましょう。

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

今回のナップザック問題は簡単なのでどれも最適解が得られます。

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
