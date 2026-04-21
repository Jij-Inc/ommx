# OMMX Python SDK 3.0.x

```{note}
Python SDK 3.0.0にはAPIの破壊的な変更が含まれます。マイグレーションガイドを [Python SDK v2 to v3 Migration Guide](https://github.com/Jij-Inc/ommx/blob/main/PYTHON_SDK_MIGRATION_GUIDE.md) にまとめてあります。
```

## 3.0.0 Alpha 2

### `Constraint.id` フィールドの削除 ([#806](https://github.com/Jij-Inc/ommx/pull/806))

{class}`~ommx.v1.Constraint`, {class}`~ommx.v1.IndicatorConstraint`, {class}`~ommx.v1.OneHotConstraint`, {class}`~ommx.v1.Sos1Constraint`, {class}`~ommx.v1.EvaluatedConstraint`, {class}`~ommx.v1.SampledConstraint`, {class}`~ommx.v1.RemovedConstraint` から `id` フィールド（および `.id` getter、`set_id()`、`id=` コンストラクタ引数）が削除されました。制約IDは {meth}`Instance.from_components <ommx.v1.Instance.from_components>` に渡す `dict[int, Constraint]` のキーとしてのみ保持されます。

比較演算子（`==`, `<=`, `>=`）で生成される `Constraint` はIDを持たず、dictに登録された時点でIDが割り当てられます。

```python
# Before (2.5.1)
c = Constraint(function=x + y, equality=Constraint.EQUAL_TO_ZERO, id=5)
Instance.from_components(..., constraints=[c], ...)

# After (3.0.0a2)
c = Constraint(function=x + y, equality=Constraint.EQUAL_TO_ZERO)
Instance.from_components(..., constraints={5: c}, ...)
```

グローバルID カウンタのヘルパー（`CONSTRAINT_ID_COUNTER`, `next_constraint_id`, `set_constraint_id_counter` など）も `ommx._ommx_rust` から削除されました。新しいIDが必要な場合は {meth}`Instance.next_constraint_id() <ommx.v1.Instance.next_constraint_id>` を使用してください。

また、{class}`~ommx.v1.Constraint` / {class}`~ommx.v1.EvaluatedConstraint` / {class}`~ommx.v1.SampledConstraint` / {class}`~ommx.v1.RemovedConstraint` 単体の `to_bytes` / `from_bytes` も削除されました（単体ではIDを保持できないため）。シリアライズは包含する {class}`~ommx.v1.Instance` / {class}`~ommx.v1.Solution` / {class}`~ommx.v1.SampleSet` に対して行ってください。

#### 制約種別ごとに独立したID空間

通常制約 (`Constraint`), Indicator Constraint, One-hot Constraint, SOS1 Constraint はそれぞれ独立したID空間を持ちます。Rust 側では `ConstraintID`, `IndicatorConstraintID`, `OneHotConstraintID`, `Sos1ConstraintID` という別々の型として定義されており、Python 側でも {meth}`Instance.from_components <ommx.v1.Instance.from_components>` の `constraints=` / `indicator_constraints=` / `one_hot_constraints=` / `sos1_constraints=` に渡す dict はそれぞれ独立した key 空間を持ちます。したがって、例えば通常制約 ID `1` と Indicator 制約 ID `1` は別々の制約として共存できます。

```python
instance = Instance.from_components(
    decision_variables=[...],
    objective=...,
    constraints={1: c},                 # 通常制約 ID=1
    indicator_constraints={1: ic},      # Indicator 制約 ID=1（別空間なので衝突しない）
    one_hot_constraints={1: oh},        # One-hot 制約 ID=1
    sos1_constraints={1: s1},           # SOS1 制約 ID=1
    sense=Instance.MAXIMIZE,
)
```

ただし {meth}`Instance.convert_one_hot_to_constraint <ommx.v1.Instance.convert_one_hot_to_constraint>` 等で特殊制約型を通常制約に変換すると、新たに生成される通常制約は `Constraint` 側のID空間から割り当てられます。

### OneHot / SOS1 を first-class 制約型に昇格 ([#798](https://github.com/Jij-Inc/ommx/pull/798))

これまで `Instance.constraint_hints` のメタデータ（`OneHot` / `Sos1` クラス、`ConstraintHints` ラッパー）として扱っていた one-hot 制約（`sum(x_i) = 1`）と SOS1 制約（高々1変数のみ非ゼロ）を、{class}`~ommx.v1.IndicatorConstraint` と同様の第一級制約型 {class}`~ommx.v1.OneHotConstraint` / {class}`~ommx.v1.Sos1Constraint` に昇格させました。

```python
from ommx.v1 import Instance, DecisionVariable, OneHotConstraint, Sos1Constraint

xs = [DecisionVariable.binary(i) for i in range(3)]

oh = OneHotConstraint(variables=[0, 1, 2])
s1 = Sos1Constraint(variables=[0, 1, 2])

instance = Instance.from_components(
    decision_variables=xs,
    objective=sum(xs),
    constraints={},
    one_hot_constraints={10: oh},
    sos1_constraints={20: s1},
    sense=Instance.MAXIMIZE,
)

for hid, oh in instance.one_hot_constraints.items():
    print(hid, oh.variables)
```

以下の旧 API は削除されました:

- `ommx.v1.ConstraintHints` / `ommx.v1.OneHot` / `ommx.v1.Sos1` クラス
- `Instance.constraint_hints` プロパティおよび `add_constraint_hints` / `with_constraint_hints` メソッド
- PySCIPOpt Adapter の `use_sos1` フラグ（SOS1 は常に first-class 制約として扱われます）

既存の v1 protobuf で `ConstraintHints` として保存されていたデータはパース時に自動で first-class の各コレクションへ変換され、同じ内容で通常の制約としても登録されていた重複エントリは吸収（削除）されます。

### Indicator Constraintのサポート ([#789](https://github.com/Jij-Inc/ommx/pull/789), [#790](https://github.com/Jij-Inc/ommx/pull/790), [#795](https://github.com/Jij-Inc/ommx/pull/795), [#796](https://github.com/Jij-Inc/ommx/pull/796))

{class}`~ommx.v1.IndicatorConstraint` がOMMXの第一級機能として追加されました。Indicator Constraintは条件付き制約を表現します。ユーザーが定義したバイナリ変数 `z` に対して、`z = 1` の時のみ制約 `f(x) <= 0`（または `f(x) = 0`）が課されます。`z = 0` の時、制約は無条件に満たされます。

{meth}`Constraint.with_indicator() <ommx.v1.Constraint.with_indicator>` を使って既存の制約から {class}`~ommx.v1.IndicatorConstraint` を作成できます。PySCIPOpt AdapterはこれをSCIPの [`addConsIndicator`](https://pyscipopt.readthedocs.io/en/latest/api/model.html#pyscipopt.scip.Model.addConsIndicator) に変換します:

```python
from ommx.v1 import DecisionVariable, Instance
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

b = DecisionVariable.binary(0)
x = DecisionVariable.continuous(1, lower=0, upper=10)

# b = 1 → x <= 5
ic = (x <= 5).with_indicator(b)

instance = Instance.from_components(
    decision_variables=[b, x],
    objective=x,
    constraints={0: b >= 1},  # b = 1 を強制
    indicator_constraints={1: ic},
    sense=Instance.MAXIMIZE,
)

solution = OMMXPySCIPOptAdapter.solve(instance)
assert abs(solution.objective - 5.0) < 1e-6
```

#### 評価結果

求解後、{class}`~ommx.v1.Solution` と {class}`~ommx.v1.SampleSet` でindicator constraint用のDataFrameを取得できます:

- {attr}`Solution.indicator_constraints_df <ommx.v1.Solution.indicator_constraints_df>` — カラム: id, indicator_variable_id, equality, value, indicator_active, used_ids, name, subscripts, description
- {attr}`Solution.indicator_removed_reasons_df <ommx.v1.Solution.indicator_removed_reasons_df>` — 緩和されたindicator constraintの除去理由
- {attr}`SampleSet.indicator_constraints_df <ommx.v1.SampleSet.indicator_constraints_df>` / {attr}`SampleSet.indicator_removed_reasons_df <ommx.v1.SampleSet.indicator_removed_reasons_df>` — サンプル毎のバージョン

`indicator_active` カラムにより、「インジケーターがOFFだった（制約は自明に満たされた）」と「インジケーターがONで制約が満たされた」を区別できます。なお、indicator constraintでは双対変数は定義が難しいため、dual variableは含まれません。

#### 緩和と復元

Indicator constraintは通常の制約と同様にrelax/restoreワークフローをサポートします:

- {meth}`Instance.relax_indicator_constraint() <ommx.v1.Instance.relax_indicator_constraint>` — indicator constraintを緩和（無効化）し、理由を記録
- {meth}`Instance.restore_indicator_constraint() <ommx.v1.Instance.restore_indicator_constraint>` — 緩和されたindicator constraintを復元（インジケーター変数が代入済み・固定済みの場合は失敗）

#### {attr}`~ommx.v1.Solution.removed_reasons_df` の分離

この変更に伴い、`removed_reason` は {attr}`~ommx.v1.Solution.constraints_df` のカラムではなくなりました。代わりに {attr}`~ommx.v1.Solution.removed_reasons_df` が {class}`~ommx.v1.Solution` と {class}`~ommx.v1.SampleSet` の両方で別テーブルとして提供され、{attr}`~ommx.v1.Solution.constraints_df` とJOINして使用できます:

```python
# 通常の制約
df = solution.constraints_df.join(solution.removed_reasons_df)

# Indicator constraint
df = solution.indicator_constraints_df.join(solution.indicator_removed_reasons_df)
```

### Adapter Capabilityモデル ([#790](https://github.com/Jij-Inc/ommx/pull/790), [#805](https://github.com/Jij-Inc/ommx/pull/805), [#810](https://github.com/Jij-Inc/ommx/pull/810), [#811](https://github.com/Jij-Inc/ommx/pull/811), [#814](https://github.com/Jij-Inc/ommx/pull/814))

{class}`~ommx.v1.IndicatorConstraint` のような特殊な制約型が追加されソルバー毎に対応・未対応が分かれるため、Adapter Capabilityモデルが導入されました。Adapterは `ADDITIONAL_CAPABILITIES` でサポートするCapabilityを宣言し、{meth}`Instance.reduce_capabilities() <ommx.v1.Instance.reduce_capabilities>` がその集合に含まれない制約タイプを通常の制約へ変換（indicator/SOS1 は Big-M、one-hot は線形等式）してから solver に渡します。`Instance` が現在保持している非標準制約タイプは {attr}`Instance.required_capabilities <ommx.v1.Instance.required_capabilities>` で確認できます。

```python
from ommx.v1 import AdditionalCapability
from ommx.adapter import SolverAdapter

class MySolverAdapter(SolverAdapter):
    ADDITIONAL_CAPABILITIES = frozenset({AdditionalCapability.Indicator})
```

現在、PySCIPOpt Adapter が Indicator 制約と SOS1 のサポートを宣言しています。**各OMMX AdapterはPython SDK 3.0.0に対応する際に変更が必要になります。** 具体的には、未サポートの Capability を自動変換するために `super().__init__(instance)` を呼び出す必要があります。

#### 特殊制約型 → 通常制約への変換API

`reduce_capabilities` は以下の型別の変換APIを組み合わせて実装されています。ユーザーが直接呼ぶことも可能です:

- **One-hot** ([#805](https://github.com/Jij-Inc/ommx/pull/805)): {meth}`Instance.convert_one_hot_to_constraint(id) <ommx.v1.Instance.convert_one_hot_to_constraint>` / {meth}`Instance.convert_all_one_hots_to_constraints() <ommx.v1.Instance.convert_all_one_hots_to_constraints>` — 数学的に等価な等式制約 `x_1 + ... + x_n - 1 == 0` に書き換えます。
- **SOS1** ([#810](https://github.com/Jij-Inc/ommx/pull/810)): {meth}`Instance.convert_sos1_to_constraints(id) <ommx.v1.Instance.convert_sos1_to_constraints>` / {meth}`Instance.convert_all_sos1_to_constraints() <ommx.v1.Instance.convert_all_sos1_to_constraints>` — Big-M による通常制約へ書き換えます。`[0, 1]` 境界のバイナリ変数はそのままインジケータとして再利用され、それ以外の変数には新しいバイナリインジケータ `y_i` が導入されます。
- **Indicator** ([#811](https://github.com/Jij-Inc/ommx/pull/811)): {meth}`Instance.convert_indicator_to_constraint(id) <ommx.v1.Instance.convert_indicator_to_constraint>` / {meth}`Instance.convert_all_indicators_to_constraints() <ommx.v1.Instance.convert_all_indicators_to_constraints>` — {class}`~ommx.v1.IndicatorConstraint` の既存のインジケータ変数を用いた Big-M 書き換えを行います。

変換された元の制約は `removed_one_hot_constraints` / `removed_sos1_constraints` / `removed_indicator_constraints` に移動し、`reason` と新たに生成された通常制約の ID が記録されます。対応する DataFrame アクセサ (`removed_one_hot_constraints_df` / `removed_sos1_constraints_df` / `removed_indicator_constraints_df`) も提供されます。

### numpy スカラ型のサポート ([#794](https://github.com/Jij-Inc/ommx/pull/794))

{class}`~ommx.v1.Function` のコンストラクタが `numpy.integer` および `numpy.floating` を受け付けるようになりました。v2.5.1 では `Function(numpy.int64(3))` は `TypeError` になっていました。

```python
import numpy as np
from ommx.v1 import Function

Function(np.int64(3))     # OK
Function(np.float64(1.5)) # OK
```

## 3.0.0 Alpha 1

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_3.0.0a1-orange?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-3.0.0a1)

詳細な変更点は上のGitHub Releaseをご覧ください。以下に主な変更点をまとめます。これはプレリリースバージョンです。APIは最終的なリリースまでに変更される可能性があります。

### `ommx.v1` および `ommx.artifact` 型の完全なRust再エクスポート ([#770](https://github.com/Jij-Inc/ommx/pull/770), [#771](https://github.com/Jij-Inc/ommx/pull/771), [#774](https://github.com/Jij-Inc/ommx/pull/774), [#775](https://github.com/Jij-Inc/ommx/pull/775), [#782](https://github.com/Jij-Inc/ommx/pull/782))

Python SDK 3.0.0は完全にRust/PyO3ベースになります。
2.0.0ではコア実装がRustで書き直されましたが、互換性のためにPythonラッパークラスが残されていました。3.0.0ではそれらのPythonラッパーを完全に削除し、`ommx.v1` およｂ `ommx.artifact` の全型がRustからの直接再エクスポートとなり、`protobuf` Pythonランタイム依存も排除されます。また旧来PyO3実装へのアクセスを提供していた `.raw` 属性も廃止されました。

### Sphinxへの移行、ReadTheDocsでのホスティング開始 ([#780](https://github.com/Jij-Inc/ommx/pull/780), [#785](https://github.com/Jij-Inc/ommx/pull/785))

v2ではSphinxベースのAPI ReferenceとJupyter Bookベースのドキュメントがそれぞれ[GitHub Pages](https://jij-inc.github.io/ommx/ja/introduction.html)でホストされていましたが、v3ではSphinxに完全移行し、[ReadTheDocs](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/)でホスティングを開始しました。GitHub Pagesは2.5.1の段階のドキュメントが引き続きホストされますが、今後の更新はReadTheDocsのみで行われます。
