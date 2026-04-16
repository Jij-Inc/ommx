# OMMX Python SDK 3.0.x

```{note}
Python SDK 3.0.0にはAPIの破壊的な変更が含まれます。マイグレーションガイドを [Python SDK v2 to v3 Migration Guide](https://github.com/Jij-Inc/ommx/blob/main/PYTHON_SDK_MIGRATION_GUIDE.md) にまとめてあります。
```

## Unreleased

### Indicator Constraintのサポート ([#789](https://github.com/Jij-Inc/ommx/pull/789), [#790](https://github.com/Jij-Inc/ommx/pull/790), [#795](https://github.com/Jij-Inc/ommx/pull/795), [#796](https://github.com/Jij-Inc/ommx/pull/796))

{class}`~ommx.v1.IndicatorConstraint` がOMMXの第一級機能として追加されました。Indicator Constraintは条件付き制約を表現します。ユーザーが定義したバイナリ変数 `z` に対して、`z = 1` の時のみ制約 `f(x) <= 0`（または `f(x) = 0`）が課されます。`z = 0` の時、制約は無条件に満たされます。

#### Indicator Constraintの作成

{meth}`Constraint.with_indicator() <ommx.v1.Constraint.with_indicator>` を使って既存の制約から {class}`~ommx.v1.IndicatorConstraint` を作成できます:

```python
from ommx.v1 import DecisionVariable, Sense, Instance

z = DecisionVariable.binary(0)
x = DecisionVariable.continuous(1, lower=0, upper=10)

# z = 1 → x <= 5
ic = (x <= 5).with_indicator(z)

instance = Instance.from_components(
    decision_variables=[z, x],
    indicator_constraints=[ic],
    objective=x,
    sense=Sense.Minimize,
)
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
df = solution.constraints_df.join(solution.removed_reasons_df)
```

これは通常の制約とindicator constraintの両方に適用されます。

### Adapter Capabilityモデル ([#790](https://github.com/Jij-Inc/ommx/pull/790))

{class}`~ommx.v1.IndicatorConstraint` のような特殊な制約型が追加されソルバー毎に対応・未対応が分かれるため、Adapter Capabilityモデルが導入されました。Adapterは `ADDITIONAL_CAPABILITIES` でサポートするCapabilityを宣言し、{meth}`Instance.check_capabilities() <ommx.v1.Instance.check_capabilities>` で問題の互換性を検証します。

```python
from ommx.v1 import AdditionalCapability
from ommx.adapter import SolverAdapter

class MySolverAdapter(SolverAdapter):
    ADDITIONAL_CAPABILITIES = frozenset({AdditionalCapability.Indicator})
```

現在、PySCIPOpt AdapterがIndicator Constraintのサポートを宣言しています。**各OMMX AdapterはPython SDK 3.0.0に対応する際に変更が必要になります。** 具体的には、Capability自動チェックのために `super().__init__(instance)` を呼び出す必要があります。

## 3.0.0 Alpha 1

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_3.0.0a1-orange?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-3.0.0a1)

詳細な変更点は上のGitHub Releaseをご覧ください。以下に主な変更点をまとめます。これはプレリリースバージョンです。APIは最終的なリリースまでに変更される可能性があります。

### `ommx.v1` および `ommx.artifact` 型の完全なRust再エクスポート ([#770](https://github.com/Jij-Inc/ommx/pull/770), [#771](https://github.com/Jij-Inc/ommx/pull/771), [#774](https://github.com/Jij-Inc/ommx/pull/774), [#775](https://github.com/Jij-Inc/ommx/pull/775), [#782](https://github.com/Jij-Inc/ommx/pull/782))

Python SDK 3.0.0は完全にRust/PyO3ベースになります。
2.0.0ではコア実装がRustで書き直されましたが、互換性のためにPythonラッパークラスが残されていました。3.0.0ではそれらのPythonラッパーを完全に削除し、`ommx.v1` およｂ `ommx.artifact` の全型がRustからの直接再エクスポートとなり、`protobuf` Pythonランタイム依存も排除されます。また旧来PyO3実装へのアクセスを提供していた `.raw` 属性も廃止されました。

### Sphinxへの移行、ReadTheDocsでのホスティング開始 ([#780](https://github.com/Jij-Inc/ommx/pull/780), [#785](https://github.com/Jij-Inc/ommx/pull/785))

v2ではSphinxベースのAPI ReferenceとJupyter Bookベースのドキュメントがそれぞれ[GitHub Pages](https://jij-inc.github.io/ommx/ja/introduction.html)でホストされていましたが、v3ではSphinxに完全移行し、[ReadTheDocs](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/)でホスティングを開始しました。GitHub Pagesは2.5.1の段階のドキュメントが引き続きホストされますが、今後の更新はReadTheDocsのみで行われます。
