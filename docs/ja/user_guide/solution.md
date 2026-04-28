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

# ommx.v1.Solution

OMMXには数理モデルの解を表す構造体がいくつか存在します

| データ構造 | 説明 |
| --- | --- |
| [`ommx.v1.State`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/solution_pb2/index.html#ommx.v1.solution_pb2.State) | 決定変数のIDに対して解の値を保持したもの。最も単純な解の表現。 |
| [`ommx.v1.Solution`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Solution) | 人間が読む事を想定した解の表現。決定変数の値やそれによる制約条件の評価値に加えて、[`ommx.v1.Instance`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance)に追加された決定変数や制約条件のメタデータも保持している。 |

多くのソルバーは数理モデルを解く事を目的としたソフトウェアなので `ommx.v1.State` に相当する最小限の情報を返しますが、OMMXではユーザーが最適化の結果を容易に確認できる形である `ommx.v1.Solution` を中心として扱います。

`ommx.v1.Solution` は `ommx.v1.Instance.evaluate` メソッドに `ommx.v1.State` あるいは相当する `dict[int, float]` を渡す事で生成されます。前節で見た簡単な最適化問題

$$
\begin{aligned}
\max \quad & x + y \\
\text{subject to} \quad & x y  = 0 \\
& x, y \in \{0, 1\}
\end{aligned}
$$

をここでも考えましょう。これは明らかに実行可能解 $x = 1, y = 0$ を持ちます。

```{code-cell} ipython3
from ommx.v1 import Instance, DecisionVariable

# Create a simple instance
x = DecisionVariable.binary(1, name='x')
y = DecisionVariable.binary(2, name='y')

instance = Instance.from_components(
    decision_variables=[x, y],
    objective=x + y,
    constraints={0: x * y == 0},
    sense=Instance.MAXIMIZE
)

# Create a solution
solution = instance.evaluate({1: 1, 2: 0})  # x=1, y=0
```

生成された `ommx.v1.Soluiton` は `ommx.v1.Instance` からほとんどの情報を引き継ぎます。まず決定変数を見てみましょう。

```{code-cell} ipython3
solution.decision_variables_df()
```

必須であるIDと `kind`, `lower`, `upper` に加えて `name` などのメタデータも引き継ぎます。加えて `value` には `evaluate` で代入された値が追加されます。同様に制約条件にも `value` として評価値が追加されます。

```{code-cell} ipython3
solution.constraints_df()
```

`objective` プロパティには目的関数の値が、`feasible` プロパティには制約条件を満たしているかどうかが格納されます。

```{code-cell} ipython3
print(f"{solution.objective=}, {solution.feasible=}")
```

$x = 1, y = 0$ の時 $xy = 0$ なので制約条件は全て守られているので `feasible` は `True` になります。また目的関数の値は $x + y = 1$ になります。

では実行可能解でないケース、$x = 1, y = 1$ の時はどうなるでしょうか？

```{code-cell} ipython3
solution11 = instance.evaluate({1: 1, 2: 1})  # x=1, y=1
print(f"{solution11.objective=}, {solution11.feasible=}")
```

`feasible = False` となっており、実行可能解でない事が確認できます。
