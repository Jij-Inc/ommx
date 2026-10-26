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

ommx.SampleSet
=================

[StateからSolutionへの評価](./solution.md)は一つの候補を扱いました。複数の候補を同じInstanceで評価する場合は、{class}`~ommx.Samples` と {class}`~ommx.SampleSet` を使います。候補はサンプラーと呼ばれるソルバーから得ることも、手動で用意することもできます。

| データ構造  | 説明 |
|:----------|:-----|
| {class}`~ommx.Samples` | 変数IDから値への対応を、候補ごとに保持したもの |
| {class}`~ommx.SampleSet` | 決定変数の値に加えて、目的関数や制約条件の評価を行ったもの |

`Samples`が`State`に相当し、`SampleSet`が`Solution`に相当します。実行可能性や目的値の意味もSolutionと同じです。

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
from ommx import ConstraintID
from ommx import DecisionVariable, Instance, Sense

x = [DecisionVariable.binary(i) for i in range(3)]

instance = Instance.from_components(
    decision_variables=x,
    objective=x[0] + 2*x[1] + 3*x[2],
    constraints={ConstraintID(0): sum(x) == 1},
    sense=Sense.Maximize,
)
```

通常はサンプラーと呼ばれるソルバーによって解を求めることになりますが、ここでは簡単のために手動で用意します。`ommx.Samples` はその名の通り複数のサンプルを持つことができ、一つのサンプルは `ommx.State` と同じように決定変数のIDに対する値として表現されます。

また個々のサンプルにはIDが振られています。サンプラーによっては内部でIDを発行し、そのIDによってログを識別する事があるので、サンプルのIDを指定できるようになっています。IDは省略することもでき、その場合 `0` から順に振られます。

```{code-cell} ipython3
from ommx import Samples

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

`ommx.Solution` は `Instance.evaluate` によって得られましたが、`ommx.SampleSet` は `Instance.evaluate_samples` によって得られます。

```{code-cell} ipython3
sample_set = instance.evaluate_samples(samples)
sample_set.summary
```

`summary`属性は各サンプルの目的値と実行可能性をデータフレーム形式で表示します。 `sample_id=2` のサンプルは制約条件を満たしていないので `feasible=False` となっています。このテーブルはFeasibleなものを上に、さらにその中で`SampleSet.sense`に応じてより良い目的関数の値を持つものを上に表示します。

```{note}
`evaluate_samples` には `Samples` を渡す代わりに、サンプルIDをキーとする辞書や、値の割り当てを並べたリストを直接渡すこともできます。
```

個々のサンプルの取り出し
---------------------
`SampleSet.get`を使用して各サンプルをサンプルIDによって `ommx.Solution`形式で取得できます：

```{code-cell} ipython3
from ommx import Solution

solution = sample_set.get(sample_id=0)
assert isinstance(solution, Solution)

print(f"{solution.objective=}")
solution.decision_variables_df()
```

得られた候補の中から最良の実行可能なものを選ぶ
-------------
`SampleSet.best_feasible`は、得られた実行可能なサンプルの中で、`sense`に応じて最も良い目的値を持つものを返します。候補の集合で最良であることは、元の問題の最適解であることを意味しません。

```{code-cell} ipython3
solution = sample_set.best_feasible

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
    assert False # best_feasible should raise ValueError
except ValueError as e:
    print(e)
```

```{note}
実行可能でない解のうちどれが最善かは非常に多彩な基準が考えられるため、OMMXでは提供していません。必要に応じて自分で実装してください。
```

変換済みの問題を評価する場合
-------------------------

ここまでの例では、作成した問題をそのまま評価しました。Instanceを変換した場合は、変換済みのInstanceで候補を評価します。

現在解く問題から外した制約を、結果の検査のために残したものをremoved constraintsと呼びます。また、現在の目的関数とは別に、報告する目的値を計算するための式（出力目的関数）を保持することがあります。このようなInstanceで`evaluate_samples`を呼ぶと、保持情報も使った評価結果がSampleSetに入ります。

どの制約を実行可能性の判定に使うかや、ソルバー向けの目的値との違いは[変換後の問題とRemoved constraints](./removed_constraints.md)を参照してください。得られた候補の中で最良であることと、元の問題全体の最適性は、この場合も区別します。
