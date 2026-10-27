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
すべての種類の制約を扱います。プロパティは所有元への参照だけを保持し、制約を収集しません。
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

Attachedハンドルは、同一の所有元オブジェクトと型付きIDを参照するときに等しくなります。
Removedと結果のスナップショットは、メタデータや削除理由を含む保持データを比較します。
`rows == dict(rows)` や `rows.values()`・`rows.items()` への包含判定も、この行の比較に従います。
別々のモデルのハンドルは、モデルの内容が同じでも等しくなりません。

`rows.get(id)` は型付きIDが存在しなければ `None` を返します。
`rows.get(id, default)` では代わりの値を指定できます。添字アクセスと同様に、
具体的なIDに対応する行の型を保ち、返り値型には代わりの値の型も含まれます。

コレクションへの代入・削除はできません。`keys()`・`values()`・`items()` は所有元の変更を反映するビューです。
`iter(rows)` は呼び出し時に型付きIDを保持します。標準Pythonビューのイテレーターでは、
最初の `next()` 時にIDを保持します。順序は種類順（通常・Indicator・OneHot・SOS1）、次に番号順です。
値は列挙された時点で取得します。保持したキーがその後削除されると、値の取得時に `KeyError` になります。
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

IDは不変で、`copy.copy(id)` と `copy.deepcopy(id)` は自身を返します。
IDをキーや値に含む辞書などをdeepcopyしても、制約の種類は保持されます。
pickleで保存・復元してもIDの種類と値は保持されます。DataFrameのインデックスや列に含まれるIDも同様です。

### 将来の特殊制約への対応

特殊制約の種類の追加は、破壊的変更ではなくAPIの拡張として扱います。
現在知られている種類が、将来のすべての種類を網羅しているという保証はしません。
`.removed_constraints` を含むコレクションの型は
`Mapping[ConstraintIDBase, ConstraintView]` です。IDの列挙は
{class}`~ommx.ConstraintIDBase`、値の列挙は共通メタデータを参照する
{class}`~ommx.ConstraintView` を返します。具体的なIDで添字アクセスした場合は、
モデルの `ConstraintID` に対する `AttachedConstraint` のように、具体的な返り値型を維持します。

ソルバーアダプターや変換処理は、扱えない種類を明示的に拒否する必要があります。
制約を黙って読み飛ばすと、最適化問題を緩和してしまう可能性があります。
例えば通常制約だけを変換できる処理では、事前の適用可否チェックがある場合も、
変換時に次のような分岐を残します。

```python
for cid in instance.constraints:
    if not isinstance(cid, ConstraintID):
        raise NotImplementedError(f"Unsupported constraint family: {type(cid).__name__}")
    constraint = instance.constraints[cid]  # AttachedConstraint
    # この通常制約を変換先のモデルへ追加する。
```

別種類に対応するときは具体的なID型の分岐を追加し、未対応種類を拒否する経路を残します。
新しい種類を古い利用コードへ渡した場合の失敗は、この拡張契約に沿った動作です。
数学的意味を解釈しないメタデータの表示などは、共通インターフェースで扱えます。
