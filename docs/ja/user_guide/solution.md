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

# StateからSolutionへの評価

{class}`~ommx.State` は変数IDから値への対応、{class}`~ommx.Solution` はそれを特定のInstanceで評価した結果です。**値の割り当てだけでは、その問題の実行可能解かどうかは分かりません。** ソルバーの出力でも、手動で用意した候補でも、同じ {meth}`~ommx.Instance.evaluate` で検査できます。

例えば、二つの項目を同時には選べない問題に、候補 $x=1, y=0$ を与えます。

```{code-cell} ipython3
from ommx import Instance, Optimality, State

instance = Instance.maximize()
x = instance.new_binary("x")
y = instance.new_binary("y")
instance.objective = x + y
instance.add_constraint(x * y == 0, "exclusive")

state = State({x.id: 1, y.id: 0})
solution = instance.evaluate(state)

assert solution.state.entries == {x.id: 1.0, y.id: 0.0}
assert solution.feasible
assert solution.objective == 1
assert solution.optimality == Optimality.Unspecified
```

`State` の代わりに `{x.id: 1, y.id: 0}` のような辞書を直接渡すこともできます。

## 実行可能性・目的値・最適性を分けて読む

| 情報 | 分かること | それだけでは分からないこと |
|---|---|---|
| {attr}`~ommx.Solution.feasible` | 変数の取りうる値と制約を満たすか | 他にもっと良い候補があるか |
| {attr}`~ommx.Solution.objective` | この候補の目的値 | この候補が実行可能か、最適か |
| {attr}`~ommx.Solution.optimality` | 記録された最適性の状態 | `evaluate` 自体は最適性を証明しない |

上の候補は実行可能ですが、`evaluate` は他の候補を探索していないため、最適性は `Unspecified` です。ソルバーが最適性を報告した場合にはAdapterがその情報を付与しますが、評価だけで `Optimal` になることはありません。

実行不可能な候補でも、目的関数を計算できる場合は目的値を持ちます。

```{code-cell} ipython3
infeasible = instance.evaluate({x.id: 1, y.id: 1})
assert infeasible.objective == 2
assert not infeasible.feasible
```

最大化問題だからといって、この目的値2の候補を実行可能な目的値1の候補より良い解として選ぶことはできません。また、制約だけでなく、変数の値の種類（kind）や下限・上限（bound）も実行可能性の判定に含まれます。

浮動小数点による評価では許容誤差を使います。`evaluate(..., atol=...)` で指定でき、結果に用いた値は {attr}`~ommx.Solution.feasibility_atol` から取得できます。数値の正規化と検査の正確な規則は {meth}`~ommx.Instance.populate_state` のAPI Referenceを参照してください。

## どの条件を満たさなかったか調べる

Solutionは、Instanceの変数・制約の定義やメタデータに、評価値を加えて保持します。

```{code-cell} ipython3
infeasible.decision_variables_df()
```

決定変数のDataFrameでは、値の種類 `kind`・下限 `lower`・上限 `upper` に加えて、割り当てた値 `value` を確認できます。制約のDataFrameでは、式を評価した `value` と実行可能性を確認できます。

```{code-cell} ipython3
infeasible.constraints_df(kind="regular")
```

通常制約では、この値は制約の左辺 $f(x)$ の値です。等式なら0、不等式なら0以下であることが条件なので、`value` が常に違反量を表すわけではありません。Indicator・OneHot・SOS1の結果は `constraints_df(kind=...)` で選びます。各種類の意味と評価方法は[特殊制約](./special_constraints.md)を参照してください。

(evaluating-transformed-instances)=
## 変換した問題の結果を読む

ソルバーに渡すためにInstanceを変換した場合は、**その変換済みInstanceで、返されたStateを評価します。** 変換済みInstanceは、元の変数の値を復元する情報や、確認すべき制約、出力する目的関数を持ちます。元のInstanceにソルバーのStateをそのまま渡しても、補助変数や依存関係を解釈できるとは限りません。

評価時には、Instanceに記録された固定値や、別の変数から値を計算する式（依存式）を使って、変数の値を復元します。それらがなく現在の問題にも使われない変数（irrelevant）は、省略すると既定値で補います。復元と補完の違いは{ref}`Instanceの変数の役割 <instance-variable-roles>`で説明しています。

この場合は、次の三つを区別してください。

- 値の復元・補完ができるか。
- 元の制約も含めて実行可能か。
- ソルバー側の目的値・最適性を、出力する問題の目的値・最適性として使えるか。

現在の問題で課す制約をactive、そこから外して評価のために保持する制約をremovedと呼びます。removedな制約を含む {attr}`~ommx.Solution.feasible` と、activeな制約だけを使う {attr}`~ommx.Solution.feasible_relaxed`、および出力目的関数との関係は[変換後の問題とRemoved constraints](./removed_constraints.md)で説明します。

複数の候補を同じ規則で評価する場合は[SampleSet](./sample_set.md)を使います。「得られた候補の中で最良」であることも、問題全体の最適性とは別です。

## pandasを使わずに制約を取得する

型付きIDを使って、所有元の現在の状態を参照するコレクションから制約を取得します。

```{code-cell} ipython3
from ommx import ConstraintID

rows = solution.constraints
rows[ConstraintID(0)].violation()
```

`Instance`・`ParametricInstance`・`Solution`・`SampleSet` の `.constraints` は、
4種類すべての制約を扱います。プロパティは所有元への参照だけを保持し、制約を収集しません。
添字アクセス時に、指定された制約を所有元の現在の状態から取得します。
整数キーは種類を判断できないため `TypeError`、存在しない型付きIDは `KeyError` になります。

| DataFrameの `kind` | IDの型 | Solutionの値 | SampleSetの値 |
| --- | --- | --- | --- |
| `"regular"` | `ConstraintID` | `EvaluatedConstraint` | `SampledConstraint` |
| `"indicator"` | `IndicatorConstraintID` | `EvaluatedIndicatorConstraint` | `SampledIndicatorConstraint` |
| `"one_hot"` | `OneHotConstraintID` | `EvaluatedOneHotConstraint` | `SampledOneHotConstraint` |
| `"sos1"` | `Sos1ConstraintID` | `EvaluatedSos1Constraint` | `SampledSos1Constraint` |

モデルの `.constraints` は有効な制約を含み、`Attached*` ハンドルを返します。
そのコンテキストのsetterは所有元を更新します。`.removed_constraints` は削除済みの全種類を含み、
スナップショットを返します。結果のコレクションは有効な制約と保持された削除済み制約の両方を含みます。
結果への各アクセスは、ラベル・provenance・削除理由・評価情報を含む独立したスナップショットを返します。
その変更は結果へ反映されませんが、同じIDへの次のアクセスは所有元の現在の状態を取得します。
結果の更新には `Solution.set_dual_variable` などの所有元のメソッドを使います。

コレクションへの代入・削除はできません。`keys()`・`values()`・`items()` は所有元の変更を反映するビューです。
イテレーターは開始時の型付きIDを、種類の順（通常・Indicator・OneHot・SOS1）、次に番号順で保持し、
値は列挙された時点で取得します。列挙中に削除されたキーの値を取得すると `KeyError` になります。
通常の辞書として収集するには `dict(rows)` を使います。

`constraints_df(*, kind=...)` は1種類をDataFrameとして収集し、すべての所有元で `kind` が必須です。
型付きインデックスは、その種類のコレクションのキーと一致します。モデルでは有効な制約を選び、
`removed=True` で `.constraints` と `.removed_constraints` の両方を含めます。
結果では保持された削除済み制約も常に含めます。SampleSetの行の `violations` と
`feasible(atol=...)` は、サンプルIDをキーとする辞書を返します。

種類が決まる入力、例えば `constraints={1: constraint}` や `instance.get_constraint_by_id(1)` は、
整数を自動昇格し、別種類のIDを拒否します。同じ入力辞書に `1` と `ConstraintID(1)` がある場合は、
同じIDになるため `ValueError` です。コレクション・`constraint_violation(id)`・DataFrameのインデックスには
型付きIDを使います。IDは `int()` や整数インデックスへ変換できません。変数IDとサンプルIDは整数のままです。
