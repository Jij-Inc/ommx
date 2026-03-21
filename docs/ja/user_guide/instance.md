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

# ommx.v1.Instance

[`ommx.v1.Instance`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance) は最適化問題自体（数理モデル）を記述するためのデータ構造です。次のコンポーネントから構成されます。

- 決定変数 ([`decision_variables`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.decision_variables))
- 目的関数（[`objective`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.objective)）
- 制約条件（[`constraints`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.constraints)）
- 最大化・最小化（[`sense`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.sense)）

例えば簡単な最適化問題を考えましょう

$$
\begin{align}
\max \quad & x + y \\
\text{subject to} \quad & x y  = 0 \\
& x, y \in \{0, 1\}
\end{align}
$$

これに対応する `ommx.v1.Instance` は次のようになります。

```{code-cell} ipython3
from ommx.v1 import Instance, DecisionVariable

x = DecisionVariable.binary(1, name='x')
y = DecisionVariable.binary(2, name='y')

instance = Instance.from_components(
    decision_variables=[x, y],
    objective=x + y,
    constraints=[x * y == 0],
    sense=Instance.MAXIMIZE
)
```

これらのコンポーネントはそれぞれに対応するプロパティが用意されています。目的関数については前節で説明した [`ommx.v1.Function`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Function) の形に変換されます。

```{code-cell} ipython3
instance.objective
```

`sense` は最大化問題を表す `Instance.MAXIMIZE` または最小化問題を表す `Instance.MINIMIZE` が設定されます。

```{code-cell} ipython3
instance.sense == Instance.MAXIMIZE
```

## 決定変数

決定変数と制約条件については [`pandas.DataFrame`](https://pandas.pydata.org/pandas-docs/stable/reference/frame.html) の形式で取得できます

```{code-cell} ipython3
instance.decision_variables
```

まず `kind` と `lower`, `upper` は数理モデルとして必須の情報です。

- `kind` はその決定変数の種類でBinary, Integer, Continuousに加えてSemiInteger, SemiContinuousがあります。
- `lower` と `upper` はその決定変数の下限と上限です。Binaryの場合は $[0, 1]$ になります。

加えてOMMXは数理最適化を実務上のデータ分析に統合した時に必要になるようなメタデータを統合的に扱う事を目指して設計されているので、決定変数のメタデータを保持することができます。これらは数理モデル自体には影響を与えないので必須の情報ではありませんがデータ分析や可視化の際に有用です。

- `name` は人間が読める形の決定変数の名前です。OMMXでは決定変数は常にIDで識別されるのこの名前は重複することがあります。後述する `subscripts` と合わせて利用することが想定されています。
- `description` はその決定変数についてのより詳細な説明です。
- 多くの数理最適化問題を扱う際、多次元配列として決定変数を扱うことが多いです。例えば $x_i + y_i \leq 1, \forall i \in [1, N]$ のような添字 $i$ を持った制約条件を考えるのが普通でしょう。この時 `x` と `y` はそれぞれの決定変数の名前なので `name` に保存し、$i$ に相当する部分を `subscripts` に保存します。`subscripts` は整数のリストであり、もし添字が整数で表現できない倍は `dict[str, str]` 型として保存できる `parameters` というプロパティが用意されています。

なお直接 [`ommx.v1.DecisionVariable`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.DecisionVariable) のリストが欲しい場合は [`decision_variables`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.decision_variables) プロパティを使うことができます

```{code-cell} ipython3
for v in instance.decision_variables:
    print(f"{v.id=}, {v.name=}")
```

決定変数のIDから `ommx.v1.DecisionVariable` を取得するには [`get_decision_variable_by_id`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.get_decision_variable_by_id) メソッドを使うことができます

```{code-cell} ipython3
x1 = instance.get_decision_variable_by_id(1)
print(f"{x1.id=}, {x1.name=}")
```

## 制約条件
次に制約条件を見てみましょう

```{code-cell} ipython3
instance.constraints_df
```

OMMXでは制約条件もIDで管理されます。このIDは決定変数のIDとは独立です。上の例で `x * y == 0` のように制約条件を作った場合は自動的に連番が振られるようになっています。手動でIDを設定するには [`set_id`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Constraint.set_id) メソッドを使うことができます。

```{code-cell} ipython3
c = (x * y == 0).set_id(100)
print(f"{c.id=}")
```

制約条件に必須の情報は `id` と `equality` です。`equality` はその制約条件が等式制約 ([`Constraint.EQUAL_TO_ZERO`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Constraint.EQUAL_TO_ZERO)) か不等式制約 ([`Constraint.LESS_THAN_OR_EQUAL_TO_ZERO`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Constraint.LESS_THAN_OR_EQUAL_TO_ZERO)) かを表します。$f(x) \geq 0$のタイプの制約条件は $-f(x) \leq 0$ として扱われることに注意してくください。

制約条件にも決定変数と同様にメタデータを保存することができます。決定変数と同様に `name`, `description`, `subscripts`, `parameters` が利用できます。これらは [`add_name`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Constraint.add_name), [`add_description`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Constraint.add_description), [`add_subscripts`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Constraint.add_subscripts), [`add_parameters`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Constraint.add_parameters) メソッドで設定できます。

```{code-cell} ipython3
c = (x * y == 0).set_id(100).add_name("prod-zero")
print(f"{c.id=}, {c.name=}")
```

また [`constraints`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.constraints) プロパティを使うことで直接 [`ommx.v1.Constraint`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Constraint) のリストを取得でき、また制約条件のIDから `ommx.v1.Constraint` を取得するには [`get_constraint_by_id`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.get_constraint_by_id) メソッドを使うことができます

```{code-cell} ipython3
for c in instance.constraints:
    print(c)
```
