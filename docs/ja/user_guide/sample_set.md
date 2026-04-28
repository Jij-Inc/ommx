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

ommx.v1.SampleSet
=================

[`ommx.v1.Solution`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/solution_pb2/index.html#module-ommx.v1.solution_pb2)はソルバーが一つの解を返す場合の表現ですが、数理最適化ソルバーによっては複数の解を返す場合があり、主にサンプラーと呼ばれます。OMMXでは複数の解を表現するために次の二つのデータ構造を用意しています：

| データ構造  | 説明 |
|:----------|:-----|
| [`ommx.v1.Samples`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/sample_set_pb2/index.html#ommx.v1.sample_set_pb2.Samples) | 決定変数のIDに対して得られた複数の解の値を列挙したもの |
| [`ommx.v1.SampleSet`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.SampleSet) | 決定変数の値に加えて、目的関数や制約条件の評価を行ったもの |

`Samples`が`State`に相当し、`SampleSet`が`Solution`に相当します。このノートブックでは、`SampleSet`の使い方を説明します。

SampleSetの作成
--------------

簡単な最適化問題を考えてみましょう：

$$
\begin{aligned}
    \max &\quad x_1 + 2 x_2 + 3 x_3 \\
    \text{s.t.} &\quad x_1 + x_2 + x_3 = 1 \\
    &\quad x_1, x_2, x_3 \in \{0, 1\}
\end{aligned}
$$

```{code-cell} ipython3
from ommx.v1 import DecisionVariable, Instance

x = [DecisionVariable.binary(i) for i in range(3)]

instance = Instance.from_components(
    decision_variables=x,
    objective=x[0] + 2*x[1] + 3*x[2],
    constraints={0: sum(x) == 1},
    sense=Instance.MAXIMIZE,
)
```

通常はサンプラーと呼ばれるソルバーによって解を求めることになりますが、ここでは簡単のために手動で用意します。`ommx.v1.Samples` はその名の通り複数のサンプルを持つことができ、一つのサンプルは `ommx.v1.State` と同じように決定変数のIDに対する値として表現されます。

また個々のサンプルにはIDが振られています。サンプラーによっては内部でIDを発行し、そのIDによってログを識別する事があるので、サンプルのIDを指定できるようになっています。IDは省略することもでき、その場合 `0` から順に振られます。

```{code-cell} ipython3
from ommx.v1 import Samples

# Sample IDを指定する場合
samples = Samples({
    0: {0: 1, 1: 0, 2: 0},  # x1 = 1, x2 = x3 = 0
    1: {0: 0, 1: 0, 2: 1},  # x3 = 1, x1 = x2 = 0
    2: {0: 1, 1: 1, 2: 0},  # x1 = x2 = 1, x3 = 0 (infeasible)
})# ^ sample ID
assert isinstance(samples, Samples)

# Sample IDを自動で割り振る場合
samples = Samples([
    {0: 1, 1: 0, 2: 0},  # x1 = 1, x2 = x3 = 0
    {0: 0, 1: 0, 2: 1},  # x3 = 1, x1 = x2 = 0
    {0: 1, 1: 1, 2: 0},  # x1 = x2 = 1, x3 = 0 (infeasible)
])
assert isinstance(samples, Samples)
```

`ommx.v1.Solution` は `Instance.evaluate` によって得られましたが、`ommx.v1.SampleSet` は `Instance.evaluate_samples` によって得られます。

```{code-cell} ipython3
sample_set = instance.evaluate_samples(samples)
sample_set.summary
```

`summary`属性は各サンプルの目的値と実行可能性をデータフレーム形式で表示します。 `sample_id=2` のサンプルは制約条件を満たしていないので `feasible=False` となっています。このテーブルはFeasibleなものを上に、さらにその中で目的関数の値が良いもの（`Instance.sense`に応じて最大化か最小化かが変わります）を上に表示されます。

```{note}
`evaluate_samples` の引数はここでは分かり易いように `to_samples` で変換した `ommx.v1.Samples` を渡していますが、`to_samples` は自動的に呼ばれるので省略することもできます。
```

個々のサンプルの取り出し
---------------------
`SampleSet.get`を使用して各サンプルをサンプルIDによって `ommx.v1.Solution`形式で取得できます：

```{code-cell} ipython3
from ommx.v1 import Solution

solution = sample_set.get(sample_id=0)
assert isinstance(solution, Solution)

print(f"{solution.objective=}")
solution.decision_variables_df()
```

最適解の取り出し
-------------
`SampleSet.best_feasible`は、実行可能なサンプルの中で最大の目的値を持つ最良の実行可能サンプルを返します：

```{code-cell} ipython3
solution = sample_set.best_feasible
assert solution is not None  # 最適な解が存在しない場合は None

print(f"{solution.objective=}")
solution.decision_variables_df()
```

もちろん、最小化問題の場合は最小の目的値のサンプルが返されます。
実行可能なサンプルが存在しない場合はエラーになります。

```{code-cell} ipython3
sample_set_infeasible = instance.evaluate_samples([
    {0: 1, 1: 1, 2: 0},  # Infeasible since x0 + x1 + x2 = 2
    {0: 1, 1: 0, 2: 1},  # Infeasible since x0 + x1 + x2 = 2
])

# Every samples are infeasible
display(sample_set_infeasible.summary)

try:
    sample_set_infeasible.best_feasible
    assert False # best_feasible should raise RuntimeError
except RuntimeError as e:
    print(e)
```

```{note}
実行可能でない解のうちどれが最善かは非常に多彩な基準が考えられるため、OMMXでは提供していません。必要に応じて自分で実装してください。
```
