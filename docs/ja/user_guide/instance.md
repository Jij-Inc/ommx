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

# ommx.Instance

[`ommx.Instance`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.Instance) は最適化問題自体（数理モデル）を記述するためのデータ構造です。次のコンポーネントから構成されます。

- 決定変数 ([`decision_variables`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.Instance.decision_variables))
- 目的関数（[`objective`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.Instance.objective)）
- 制約条件（[`constraints`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.Instance.constraints)）
- 最大化・最小化（[`sense`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.Instance.sense)）

例えば簡単な最適化問題を考えましょう

$$
\begin{aligned}
\max \quad & x + y \\
\text{subject to} \quad & x y  = 0 \\
& x, y \in \{0, 1\}
\end{aligned}
$$

これに対応する `ommx.Instance` は次のようになります。

```{code-cell} ipython3
from ommx import Instance, DecisionVariable

x = DecisionVariable.binary(1, name='x')
y = DecisionVariable.binary(2, name='y')

instance = Instance.from_components(
    decision_variables=[x, y],
    objective=x + y,
    constraints={0: x * y == 0},
    sense=Instance.MAXIMIZE
)
```

これらのコンポーネントはそれぞれに対応するプロパティが用意されています。目的関数については前節で説明した [`ommx.Function`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.Function) の形に変換されます。

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
instance.decision_variables_df()
```

まず `kind` と `lower`, `upper` は数理モデルとして必須の情報です。

- `kind` はその決定変数の種類でBinary, Integer, Continuousに加えてSemiInteger, SemiContinuousがあります。
- `lower` と `upper` はその決定変数の下限と上限です。Binaryの場合は $[0, 1]$ になります。

加えてOMMXは数理最適化を実務上のデータ分析に統合した時に必要になるようなメタデータを統合的に扱う事を目指して設計されているので、決定変数のメタデータを保持することができます。これらは数理モデル自体には影響を与えないので必須の情報ではありませんがデータ分析や可視化の際に有用です。

- `name` は人間が読める形の決定変数の名前です。OMMXでは決定変数は常にIDで識別されるのこの名前は重複することがあります。後述する `subscripts` と合わせて利用することが想定されています。
- `description` はその決定変数についてのより詳細な説明です。
- 多くの数理最適化問題を扱う際、多次元配列として決定変数を扱うことが多いです。例えば $x_i + y_i \leq 1, \forall i \in [1, N]$ のような添字 $i$ を持った制約条件を考えるのが普通でしょう。この時 `x` と `y` はそれぞれの決定変数の名前なので `name` に保存し、$i$ に相当する部分を `subscripts` に保存します。`subscripts` は整数のリストであり、もし添字が整数で表現できない倍は `dict[str, str]` 型として保存できる `parameters` というプロパティが用意されています。

なお直接 [`ommx.DecisionVariable`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.DecisionVariable) のリストが欲しい場合は [`decision_variables`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.Instance.decision_variables) プロパティを使うことができます

```{code-cell} ipython3
for v in instance.decision_variables:
    print(f"{v.id=}, {v.name=}")
```

決定変数のIDから `ommx.DecisionVariable` を取得するには [`get_decision_variable_by_id`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.Instance.get_decision_variable_by_id) メソッドを使うことができます

```{code-cell} ipython3
x1 = instance.get_decision_variable_by_id(1)
print(f"{x1.id=}, {x1.name=}")
```

## 制約条件
次に制約条件を見てみましょう

```{code-cell} ipython3
instance.constraints_df()
```

OMMXでは制約条件もIDで管理されます。このIDは決定変数のIDとは独立です。制約条件のIDは `Instance` に登録する際に決まります: [`Instance.from_components`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.Instance.from_components) に渡す `constraints` 辞書のキーがそのまま制約条件のIDになります。

制約条件に必須の情報は `equality` です。`equality` はその制約条件が等式制約 ([`Constraint.EQUAL_TO_ZERO`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.Constraint.EQUAL_TO_ZERO)) か不等式制約 ([`Constraint.LESS_THAN_OR_EQUAL_TO_ZERO`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.Constraint.LESS_THAN_OR_EQUAL_TO_ZERO)) かを表します。$f(x) \geq 0$のタイプの制約条件は $-f(x) \leq 0$ として扱われることに注意してください。

制約条件にも決定変数と同様にメタデータを保存することができます。決定変数と同様に `name`, `description`, `subscripts`, `parameters` が利用できます。これらのメタデータ全体を置き換える場合は `set_name`, `set_description`, `set_subscripts`, `set_parameters` を使います。既存の値に追記または merge したい場合は `add_subscripts`, `add_parameter`, `add_parameters` を使います。

```{code-cell} ipython3
c = (x * y == 0).set_name("prod-zero")
print(f"{c.name=}")
```

また [`constraints`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.Instance.constraints) プロパティを使うことで制約条件IDをキーとする `dict[int, ommx.Constraint]` を直接取得できます。制約条件のIDから `ommx.Constraint` を取得するには [`get_constraint_by_id`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.Instance.get_constraint_by_id) メソッドを使うことができます。

```{code-cell} ipython3
for cid, c in instance.constraints.items():
    print(f"id={cid}: {c}")
```

## 記号的な代入

`Instance.substitute` は目的関数と有効な制約条件に現れる決定変数を、指定した関数式で置き換えます。これは整数変数を新しいバイナリ変数で表現する binary encoding のような変換で使われます。

この操作は代数的な書き換えです。代入された変数の `kind`, `lower`, `upper` を、置換後の式に対する制約へ自動的には変換しません。例えば `x1` が binary で、`x1` を `x2 + x3` に置き換えても、OMMX は `0 <= x2 + x3` や `x2 + x3 <= 1` を追加しません。`x1` が integer の場合も、置換後の式が整数値を取るという制約は追加されません。

代入された変数は従属変数として記録されるため、解を評価するときに値を復元できます。その bound や kind は `Solution.feasible()` で検証されますが、置換後の式に対するソルバー制約としては渡されません。つまり `substitute` だけでは、最適化モデルとして等価な変換であることは保証されません。

これは意図した仕様です。制約を緩和する操作のように、モデルを意図的に変える変換もあります。一方で log encoding や独自の binary encoding のような変換は、エンコーディング自体が元の変数の domain を保つように構築されるため正当化できます。

一般の代入でモデルの意味を保存したい場合は、必要な制約を明示的に追加してください。保守的な方法は、元の変数を消去せずに linking equality を追加することです。

```python
instance.add_constraint(x1 - (x2 + x3) == 0)
```

`substitute` で `x1` を消去する場合は、置換後の式に必要な bound 制約を別途追加します。

```python
expr = x2 + x3
instance.substitute({1: expr})
instance.add_constraint(expr >= 0)
instance.add_constraint(expr <= 1)
```
