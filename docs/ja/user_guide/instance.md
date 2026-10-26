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

# Instance：最適化問題と変数の役割

{class}`~ommx.Instance` は、決定変数の定義域（取りうる値の種類と範囲）、目的関数、制約条件、最大化・最小化をまとめた最適化問題です。特定のソルバーの入力ではなく、**どの値の割り当てが実行可能で、どの目的値を持つか**をOMMXで定義します。そのため、求解しなくても候補を評価できます。

例えば、二つの項目を同時には選べない問題を作ります。

$$
\begin{aligned}
\max \quad & x + y \\
\text{subject to} \quad & xy = 0 \\
& x, y \in \{0, 1\}.
\end{aligned}
$$

```{code-cell} ipython3
from ommx import Instance

instance = Instance.maximize()
x = instance.new_binary("x")
y = instance.new_binary("y")
instance.objective = x + y
exclusive = instance.add_constraint(x * y == 0, "exclusive")

solution = instance.evaluate({x.id: 1, y.id: 0})
assert solution.feasible
assert solution.objective == 1
```

`new_binary` と `add_constraint` はIDを自動的に割り当てます。IDは `x.id` のように参照できます。値の割り当てを表す {class}`~ommx.State` と、評価結果である {class}`~ommx.Solution` の違いは、[StateからSolutionへの評価](./solution.md)で説明します。評価は候補を検査する操作であり、最適解を探索する操作ではありません。

## 問題を構成する情報

| 情報 | 意味 | 参照先 |
|---|---|---|
| 決定変数 | 値の種類と取りうる範囲 | {attr}`~ommx.Instance.decision_variables` |
| 目的関数 | 比較したい数値 | {attr}`~ommx.Instance.objective` |
| 制約条件 | 満たす必要がある条件 | {attr}`~ommx.Instance.constraints` |
| 最大化・最小化 | 目的値の大きい方と小さい方のどちらを選ぶか | {attr}`~ommx.Instance.sense` |

最小化問題は {meth}`~ommx.Instance.minimize` で作成します。目的関数や通常制約の式は {class}`~ommx.Function` で表されます。関数の組み立て方は[Function](./function.md)、Indicator・OneHot・SOS1の成立条件は[特殊制約](./special_constraints.md)を参照してください。

変数と制約は名前ではなくIDで識別します。名前が同じでも、IDが異なれば別の変数・制約です。明示的なIDで既存のコンポーネントを組み立てる場合は {meth}`~ommx.Instance.from_components` を使えます。Instanceは、式や特殊制約が参照する変数が自身に定義されているかを検証します。

(decision-variable-domains)=
## 変数の定義域とメタデータ

決定変数の定義域（domain）は、値の種類 `kind` と下限・上限 `lower`・`upper` で指定します。下限・上限をboundとも呼びます。次のようにInstance上で作成できます。

```{code-cell} ipython3
typed = Instance.minimize()
flag = typed.new_binary("flag")
count = typed.new_integer("count", lower=0, upper=10)
amount = typed.new_continuous("amount", lower=0)
batch = typed.new_semi_integer("batch", lower=2, upper=10)
rate = typed.new_semi_continuous("rate", lower=0.5, upper=4)
```

Binaryは0または1、Integerは整数、Continuousは実数を取ります。SemiInteger・SemiContinuousは、指定範囲内の整数・実数に加えて0も取れます。例えば`batch`には2から10までの整数に加えて0も、`rate`には0.5から4までの実数に加えて0も割り当てられます。boundの正規化や許容誤差は、各 `new_*` メソッドのAPI Referenceで確認できます。

`name`、`subscripts`、`parameters`、`description` は、結果の分析や可視化に使うメタデータです。これらは変数の定義域や制約の成立条件を変えません。例えば $x_i$ のような変数の名前を `name="x"`、添字を `subscripts=[i]` として保存できます。

```{code-cell} ipython3
instance.decision_variables_df()
```

{meth}`~ommx.Instance.decision_variables_df` は、定義やラベルをpandasのDataFrameとして返します。一つの変数をIDから取得する場合は {meth}`~ommx.Instance.get_decision_variable_by_id` を使います。

(retained-and-used-variables)=
## 保持される変数と、現在使われている変数

通常の定式化では、宣言した変数をすべてソルバーに渡すと考えるかもしれません。しかし、変数を固定したり、別の変数で表現したりすると、ソルバーが値を選ぶ必要のない変数が生まれます。OMMXは、結果を元の変数で読めるよう、その変数の定義も保持します。

先ほどの問題で $x=1$ と固定してみましょう。Pythonの {meth}`~ommx.Instance.partial_evaluate` は、元のInstanceを変更せず、新しいInstanceを返します。

```{code-cell} ipython3
fixed = instance.partial_evaluate({x.id: 1})

# xの定義は残るが、現在の式で値を選ぶ必要があるのはyだけ。
assert {v.id for v in fixed.decision_variables} == {x.id, y.id}
assert {v.id for v in fixed.used_decision_variables} == {y.id}

# xの値をもう一度渡す必要はない。
fixed_solution = fixed.evaluate({y.id: 0})
assert fixed_solution.state.entries == {x.id: 1.0, y.id: 0.0}
assert fixed_solution.objective == 1
```

{attr}`~ommx.Instance.decision_variables` は保持されるすべての変数、{attr}`~ommx.Instance.used_decision_variables` は**現在の目的関数とactiveな制約に現れる変数**を返します。activeは現在の問題で課す制約を指し、通常制約だけでなく、Indicator・OneHot・SOS1も含みます。Indicatorを制御するバイナリ変数もusedです。

(instance-variable-roles)=
### 変数の四つの役割

Instance内の各変数は、次のいずれかの役割を持ちます。これは変数の種類（BinaryやInteger）とは別の分類です。

| 役割 | 意味 | 評価に必要な値 |
|---|---|---|
| used | 現在の目的関数やactiveな制約に現れる | 呼び出し側が与える |
| fixed | Instanceに固定値が記録されている | 記録された値を使う |
| dependent | 別の変数の関数として表されている | 記録された式から計算する |
| irrelevant | 上記のどれでもなく、現在の問題では値を選ぶ必要がない | 省略された場合は既定値で補う |

{meth}`~ommx.Instance.decision_variable_role` で一つの変数、{meth}`~ommx.Instance.decision_variable_roles` ですべての変数の役割を確認できます。

### 値の復元と、既定値による補完は違う

fixedとdependentの値は、Instanceが持つ固定値や依存式から決まります。これに対して、irrelevantの値は一意には決まりません。省略されたirrelevant変数には定義域に応じた既定値が入りますが、それはソルバーが選んだ値でも、元の解を一意に復元した値でもありません。

例えば、目的関数にも制約にも使わない変数を宣言できます。

```{code-cell} ipython3
reporting = Instance.minimize()
chosen = reporting.new_binary("chosen")
unused = reporting.new_integer("unused", lower=2, upper=5)
reporting.objective = chosen

completed = reporting.evaluate({chosen.id: 0})
assert completed.state.entries[unused.id] == 2.0

# この値を結果の解釈に使いたいなら、明示的に与える。
explicit = reporting.evaluate({chosen.id: 0, unused.id: 4})
assert explicit.state.entries[unused.id] == 4.0
```

Binary・Integer・Continuousの既定値はbound内で0に最も近い値、SemiInteger・SemiContinuousでは0です。報告したい値や、検査したい候補の値が既にあるなら、省略せずに渡してください。

{meth}`~ommx.Instance.populate_state` はこの復元・補完だけを行い、{meth}`~ommx.Instance.evaluate` はその後に目的関数や制約を評価します。usedな変数の不足を既定値で埋めることはありません。数値の丸めや、fixed・dependentの値も入力した場合の整合性検査は、`populate_state` のAPI Referenceを参照してください。

## 通常制約の参照

通常制約は $f(x)=0$ または $f(x)\leq 0$ として保持されます。例えば $f(x)\geq 0$ は $-f(x)\leq 0$ に書き換えられます。{attr}`~ommx.Constraint.equality` が、等式か不等式かを示します。

```{code-cell} ipython3
instance.constraints_df()
```

{attr}`~ommx.Instance.constraints` は制約IDをキーとする辞書です。制約にも変数と同様のメタデータを付けられます。制約IDの空間は変数IDとは独立し、特殊制約の各種類もそれぞれ独立した制約IDを持ちます。

(simultaneous-bound-tightening)=
## モデルを変換する場合

Instanceは構築後にも変換できます。ただし、**変換結果を評価できることと、元の最適化問題と等価であることは別**です。

例えば {meth}`~ommx.Instance.substitute` は、変数を別の式で置き換え、元の変数をdependentとして復元できるようにします。しかし、元の変数の定義域を置換先の式に対する制約へ自動変換するわけではありません。独自の変換を作る場合は、定義域を含めて何を保存するかを確認する必要があります。

また、{meth}`~ommx.Instance.tighten_bounds_simultaneously_once` は通常制約から変数のboundを締める操作です。対象となる式、許容誤差、数値計算上の制約、制約を選ぶ方法はAPI Referenceを参照してください。

### 結果の確認用に保持する情報

Instanceは、現在の目的関数と制約以外にも、結果を確認するための情報を保持できます。例えば{class}`~ommx.NamedFunction`は、名前を付けて保存し、目的関数や制約とは別に値を確認する式です。

また、変換で現在課す条件から外した制約をremoved、ソルバー向けとは別に評価用に保持する目的関数を出力目的関数と呼びます。名前付き関数・removedな制約・出力目的関数だけに現れる変数は、それだけではusedにはなりません。

それらにirrelevant変数が現れる場合、省略した値には先ほどの既定値が使われます。特定の候補を検査したいなら、その値を明示的に渡してください。保持された制約と目的値をどう読むかは[変換後の問題とRemoved constraints](./removed_constraints.md)で説明します。ここまでの変数の役割と候補評価を確認してから、変換後の結果を読む際に参照できます。
