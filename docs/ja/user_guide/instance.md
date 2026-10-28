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

# Instance：最適化問題の基本モデル

{class}`~ommx.Instance` は、決定変数の定義域（取りうる値の種類と範囲）、目的関数、制約条件、最大化・最小化をまとめた最適化問題です。特定のソルバーの入力ではなく、**どの値の割り当てが実行可能で、どの目的値を持つか**をOMMXで定義します。そのため、求解しなくても候補を評価できます。

制約条件には、数式の等式・不等式で表す**通常制約**と、「ちょうど一つを選ぶ」などの関係を専用の型で表す**特殊制約**があります。どちらもInstanceが表す問題の一部です。

例えば、通常制約を使って、二つの項目を同時には選べない問題を作ります。

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
| 通常制約 | 数式の等式・不等式で表す条件 | {attr}`~ommx.Instance.constraints` |
| 特殊制約 | 条件付きの制約や、変数の組に対する選択条件など | [特殊制約の意味と評価](./special_constraints.md) |
| 最大化・最小化 | 目的値の大きい方と小さい方のどちらを選ぶか | {attr}`~ommx.Instance.sense` |

最小化問題は {meth}`~ommx.Instance.minimize` で作成します。目的関数や通常制約の式は {class}`~ommx.Function` で表されます。関数の組み立て方は[Function](./function.md)を参照してください。

現在はIndicator・OneHot・SOS1が用意されており、特殊制約の種類は今後も増やしていくことを想定しています。特殊制約もOMMXが成立条件を定める問題の一部であり、特定のソルバーへの指示ではありません。各種類の意味と、利用できる型・操作への入口は[特殊制約のGuide](./special_constraints.md)にあります。各ソルバーが直接受け取れる種類は異なるため、Adapterの入力条件とは分けて考えます。

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

{meth}`~ommx.Instance.decision_variables_df` は、定義やラベルをpandasのDataFrameとして返します。一つの変数の独立したコピーをIDから取得する場合は {meth}`~ommx.Instance.get_decision_variable_by_id` を使います。Instance上の名前や説明を編集する方法は、後述の{ref}`既存モデルのメタデータ編集 <instance-edit-metadata>`で説明します。

## 保持される変数と、現在使われている変数

通常の定式化では、宣言した変数をすべてソルバーに渡すと考えるかもしれません。しかし、変数を固定したり、別の変数で表現したりすると、ソルバーが値を選ぶ必要のない変数が生まれます。OMMXは、結果を元の変数で読めるよう、その変数の定義も保持します。

{attr}`~ommx.Instance.decision_variables` は保持されるすべての変数、{attr}`~ommx.Instance.used_decision_variables` は**現在の目的関数と、現在課している制約に現れる変数**を返します。

両者が異なる理由と、候補を評価するときに固定値・依存式・既定値をどう使うかは、[変数の役割と値の復元・補完](./variable_roles.md)で説明します。最初の求解や候補評価では、問題に使った変数の値を与えて結果を読むところから始められます。

## 通常制約の参照

通常制約は $f(x)=0$ または $f(x)\leq 0$ として保持されます。例えば $f(x)\geq 0$ は $-f(x)\leq 0$ に書き換えられます。{attr}`~ommx.Constraint.equality` が、等式か不等式かを示します。

```{code-cell} ipython3
instance.constraints_df(kind="regular")
```

{attr}`~ommx.Instance.constraints` は、全種類の有効な制約を取得するコレクションです。通常制約なら `instance.constraints[ConstraintID(0)]` のように型付きIDでアクセスします。IDの型で制約の種類を指定するため、裸の整数は受け付けません。IDの番号は、removedを含む全種類を通じて同じInstance内で一意です。返されるAttachedハンドルを通じて、所有元の制約を参照・更新できます。コレクションを取得した時点では制約を収集せず、アクセス時に該当する制約を取得します。removedな制約は `instance.removed_constraints` で取得します。

(instance-edit-metadata)=
## 既存モデルの変数・制約に名前や説明を付ける

読み込んだ問題に業務上の名前や説明を付けると、式の確認や結果の分析に使えます。**Instanceに結び付いた参照（Attached handle）を取得して編集します。** 次の例では、このページの冒頭で作った問題の変数・通常制約をIDで取得し、ラベルを付け直します。ファイルから読み込んだInstanceでも同じ手順です。

```{code-cell} ipython3
variable = instance.attached_decision_variable(x.id)
constraint = instance.constraints[exclusive.constraint_id]

variable.set_name("selected")
variable.set_subscripts([0])
variable.set_description("商品0を採用するか")
constraint.set_name("at_most_one")
constraint.set_description("二つの商品を同時には採用しない")

assert instance.attached_decision_variable(x.id).name == "selected"
assert instance.attached_decision_variable(x.id).description == "商品0を採用するか"
assert instance.constraints[exclusive.constraint_id].name == "at_most_one"
assert instance.constraints[exclusive.constraint_id].description == "二つの商品を同時には採用しない"
```

{meth}`~ommx.Instance.attached_decision_variable` が返す{class}`~ommx.AttachedDecisionVariable`と、`instance.constraints[id]`が返す{class}`~ommx.AttachedConstraint`の変更は、そのInstanceに反映されます。名前を変えてもIDは変わりません。ここで変更したのはラベルだけで、変数の定義域・目的関数・制約の式は同じです。数学的な条件を変える操作とは区別します。

一方、`detach()`は取得時点の独立したコピー（snapshot）を返します。次の例では、通常制約のsnapshotの名前だけを変えます。DataFrameも表示・分析用の表であり、どちらを編集してもInstanceへの書き戻しは起きません。

```{code-cell} ipython3
snapshot = constraint.detach()
snapshot.set_name("snapshot_only")

table = instance.decision_variables_df()
table.loc[x.id, "name"] = "table_only"

assert instance.attached_decision_variable(x.id).name == "selected"
assert instance.constraints[exclusive.constraint_id].name == "at_most_one"
```

`constraints_df(kind="regular")`の編集も同様に書き戻されません。変数の`detach()`や{meth}`~ommx.Instance.get_decision_variable_by_id`もsnapshotを返すため、Instanceのラベルを編集するときは上のAttached handleを使います。snapshotの独立性は、{meth}`~ommx.AttachedDecisionVariable.detach`と{meth}`~ommx.AttachedConstraint.detach`のAPI Referenceで確認できます。

付け直した変数名・添字は、{meth}`~ommx.Instance.format_function`や{meth}`~ommx.Instance.display_function`で式を表示するときに使われます。その後に評価・求解したSolutionでは、{meth}`~ommx.Solution.extract_decision_variables`で名前・添字に対応する値を取り出せます。ラベルを整えてから結果を作ると、商品などのモデル上の単位で後処理できます。

## モデルを変換する場合

Instanceは構築後にも変換できます。ただし、**変換結果を評価できることと、元の最適化問題と等価であることは別**です。

変数を固定する、別の式で置き換える、制約を別の表現にする、といった操作では、何を保存するかを確認する必要があります。[変換が何を保存するか](./transformation_guarantees.md)では、実行可能性・目的値・最適性を分けて判断します。一連の操作を試す場合は[Instanceを変換し、結果を確認する](../how_to/transform_instance_without_solver.md)を参照してください。

また、{meth}`~ommx.Instance.tighten_bounds_simultaneously_once` は通常制約から変数のboundを締める操作です。対象となる式、許容誤差、数値計算上の制約、制約を選ぶ方法はAPI Referenceを参照してください。

### 結果の確認用に保持する情報

Instanceは、現在の目的関数と制約以外にも、結果を確認するための情報を保持できます。例えば{class}`~ommx.NamedFunction`は、名前を付けて保存し、目的関数や制約とは別に値を確認する式です。

また、変換で現在課す条件から外した制約をremoved、ソルバー向けとは別に評価用に保持する目的関数を出力目的関数と呼びます。名前付き関数・removedな制約・出力目的関数だけに現れる変数は、それだけではusedにはなりません。

それらだけに現れる変数の値を省略すると、復元・補完した値で評価されます。特定の候補を検査したいなら、その候補の値を明示的に渡してください。[変数の役割](./variable_roles.md)が値の出所を、[変換後の問題とRemoved constraints](./removed_constraints.md)が保持された制約と目的値の読み方を説明します。
