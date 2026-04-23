# OMMX Python SDK 3.0.x

```{note}
Python SDK 3.0.0にはAPIの破壊的な変更が含まれます。マイグレーションガイドを [Python SDK v2 to v3 Migration Guide](https://github.com/Jij-Inc/ommx/blob/main/PYTHON_SDK_MIGRATION_GUIDE.md) にまとめてあります。
```

## Unreleased

### 🆕 OpenTelemetryベースのトレーシング/プロファイリング ([#816](https://github.com/Jij-Inc/ommx/pull/816), [#823](https://github.com/Jij-Inc/ommx/pull/823), [#826](https://github.com/Jij-Inc/ommx/pull/826), [#828](https://github.com/Jij-Inc/ommx/pull/828), [#829](https://github.com/Jij-Inc/ommx/pull/829))

従来の `log` + `pyo3-log` 経由のPython `logging` ブリッジを廃止し、Rustコアを `tracing` + `pyo3-tracing-opentelemetry` ベースに切り替えて、Python OTel SDKを通じて可視化できるようになりました。

`ommx.tracing` モジュールに2つの入口を用意しています:

- **`%%ommx_trace`** — Jupyterセル単位でスパンツリーとChrome Trace JSONダウンロードリンクを表示するセルマジック
- **`capture_trace` / `@traced`** — 通常のPythonスクリプト／テスト／CIから同じ機能を使うためのコンテキストマネージャとデコレータ

詳しい使い方、独自 `TracerProvider` の設定方法、トラブルシューティングは [トレースとプロファイリング](../user_guide/tracing.md) を参照してください。

### 🆕 `Function.evaluate_bound` を Python から利用可能に ([#831](https://github.com/Jij-Inc/ommx/pull/831))

{class}`~ommx.v1.Function` に {meth}`Function.evaluate_bound <ommx.v1.Function.evaluate_bound>` が追加され、各変数の区間を与えると関数値の範囲を含む {class}`~ommx.v1.Bound` を返せるようになりました。Python 側で実行可能領域の事前解析や簡単な presolve を行う際に利用できます。

```python
from ommx.v1 import Function, Linear, Bound

f = Function(Linear(terms={1: 2}, constant=3))  # 2*x1 + 3
b = f.evaluate_bound({1: Bound(0.0, 2.0)})
# b.lower == 3.0, b.upper == 7.0
```

評価は単項式ごとに行って和を取るため、真の値域に対して sound な over-approximation にはなりますが、同じ変数を持つ複数の項がある場合は一般に tight ではありません（区間演算における dependency problem）。`bounds` に含まれていない変数 ID は unbounded として扱われます。

## 3.0.0 Alpha 2

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_3.0.0a2-orange?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-3.0.0a2)

詳細な変更点は上のGitHub Releaseをご覧ください。以下に主な変更点をまとめます。これはプレリリースバージョンです。APIは最終的なリリースまでに変更される可能性があります。

### ⚠ `Constraint.id` フィールドの削除 ([#806](https://github.com/Jij-Inc/ommx/pull/806))

{class}`~ommx.v1.Constraint` およびその派生型 ({class}`~ommx.v1.IndicatorConstraint` / {class}`~ommx.v1.OneHotConstraint` / {class}`~ommx.v1.Sos1Constraint` / {class}`~ommx.v1.EvaluatedConstraint` / {class}`~ommx.v1.SampledConstraint` / {class}`~ommx.v1.RemovedConstraint`) から `id` フィールド（および `.id` getter、`set_id()`、`id=` コンストラクタ引数）が削除されました。制約IDは {meth}`Instance.from_components <ommx.v1.Instance.from_components>` に渡す `dict[int, Constraint]` のキーとしてのみ保持されます。

```python
# Before (2.5.1)
c = Constraint(function=x + y, equality=Constraint.EQUAL_TO_ZERO, id=5)
Instance.from_components(..., constraints=[c], ...)

# After (3.0.0a2)
c = Constraint(function=x + y, equality=Constraint.EQUAL_TO_ZERO)
Instance.from_components(..., constraints={5: c}, ...)
```

グローバル ID カウンタ（`next_constraint_id` 等）や制約単体の `to_bytes` / `from_bytes` も削除されています。詳細および移行手順は [Python SDK v2 to v3 Migration Guide](https://github.com/Jij-Inc/ommx/blob/main/PYTHON_SDK_MIGRATION_GUIDE.md) を参照してください。

### 🆕 特殊制約型の整備 ([#789](https://github.com/Jij-Inc/ommx/pull/789), [#790](https://github.com/Jij-Inc/ommx/pull/790), [#795](https://github.com/Jij-Inc/ommx/pull/795), [#796](https://github.com/Jij-Inc/ommx/pull/796), [#798](https://github.com/Jij-Inc/ommx/pull/798))

通常制約に加えて以下の3種類の特殊制約を、すべて第一級の制約型として `Instance.from_components` に `indicator_constraints=` / `one_hot_constraints=` / `sos1_constraints=` として渡せるようになりました。{class}`~ommx.v1.Solution` / {class}`~ommx.v1.SampleSet` にも対応する DataFrame (`*_constraints_df`) が提供されます。

- {class}`~ommx.v1.IndicatorConstraint` — バイナリ変数による条件付き制約 (新規追加)
- {class}`~ommx.v1.OneHotConstraint` — 従来 `ConstraintHints.OneHot` として扱われていた one-hot 制約
- {class}`~ommx.v1.Sos1Constraint` — 従来 `ConstraintHints.Sos1` として扱われていた SOS1 制約

具体的な使い方、評価結果の参照、Indicator 制約の relax / restore ワークフローについては [特殊制約型](../user_guide/special_constraints.md) を参照してください。

これに伴い旧 API である `ConstraintHints` / `OneHot` / `Sos1` クラス、`Instance.constraint_hints` プロパティ、PySCIPOpt Adapter の `use_sos1` フラグは削除されています。

### ⚠ `removed_reason` カラムを別テーブルに分離 ([#796](https://github.com/Jij-Inc/ommx/pull/796))

v2.5.1 までは {attr}`Solution.constraints_df <ommx.v1.Solution.constraints_df>` に `removed_reason` カラムが含まれていましたが、v3.0.0a2 ではこれを {attr}`Solution.removed_reasons_df <ommx.v1.Solution.removed_reasons_df>` という別テーブルに分離しました。従来の形が必要な場合は join してください。同じ変更が {class}`~ommx.v1.SampleSet` にも適用されています。

```python
# Before (2.5.1)
df = solution.constraints_df  # 'removed_reason' カラムを含む

# After (3.0.0a2)
df = solution.constraints_df.join(solution.removed_reasons_df)
```

Indicator / OneHot / SOS1 それぞれに対応する `*_removed_reasons_df` も提供されています。

### 🆕 Adapter Capability モデル ([#790](https://github.com/Jij-Inc/ommx/pull/790), [#805](https://github.com/Jij-Inc/ommx/pull/805), [#810](https://github.com/Jij-Inc/ommx/pull/810), [#811](https://github.com/Jij-Inc/ommx/pull/811), [#814](https://github.com/Jij-Inc/ommx/pull/814))

特殊制約の追加に伴い、Adapter が自身のサポートする制約型を `ADDITIONAL_CAPABILITIES` クラス属性で宣言する仕組みを導入しました。`super().__init__(instance)` が呼ばれると、未宣言の特殊制約は自動的に通常の制約へ変換（indicator/SOS1 は Big-M、one-hot は線形等式）されてから solver に渡されます。

**既存の OMMX Adapter は Python SDK 3.0.0 に対応するため `super().__init__(instance)` を呼ぶよう変更する必要があります。** 現在 PySCIPOpt Adapter は Indicator 制約と SOS1 をサポート宣言しています。

詳細および手動での変換 API については [Adapter Capability モデルと制約変換](../user_guide/capability_model.md) を参照してください。

### 🔄 numpy スカラ型のサポート ([#794](https://github.com/Jij-Inc/ommx/pull/794))

{class}`~ommx.v1.Function` のコンストラクタが `numpy.integer` および `numpy.floating` を受け付けるようになりました。v2.5.1 では `Function(numpy.int64(3))` は `TypeError` になっていました。

## 3.0.0 Alpha 1

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_3.0.0a1-orange?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-3.0.0a1)

詳細な変更点は上のGitHub Releaseをご覧ください。以下に主な変更点をまとめます。これはプレリリースバージョンです。APIは最終的なリリースまでに変更される可能性があります。

### `ommx.v1` および `ommx.artifact` 型の完全なRust再エクスポート ([#770](https://github.com/Jij-Inc/ommx/pull/770), [#771](https://github.com/Jij-Inc/ommx/pull/771), [#774](https://github.com/Jij-Inc/ommx/pull/774), [#775](https://github.com/Jij-Inc/ommx/pull/775), [#782](https://github.com/Jij-Inc/ommx/pull/782))

Python SDK 3.0.0は完全にRust/PyO3ベースになります。
2.0.0ではコア実装がRustで書き直されましたが、互換性のためにPythonラッパークラスが残されていました。3.0.0ではそれらのPythonラッパーを完全に削除し、`ommx.v1` およｂ `ommx.artifact` の全型がRustからの直接再エクスポートとなり、`protobuf` Pythonランタイム依存も排除されます。また旧来PyO3実装へのアクセスを提供していた `.raw` 属性も廃止されました。

### Sphinxへの移行、ReadTheDocsでのホスティング開始 ([#780](https://github.com/Jij-Inc/ommx/pull/780), [#785](https://github.com/Jij-Inc/ommx/pull/785))

v2ではSphinxベースのAPI ReferenceとJupyter Bookベースのドキュメントがそれぞれ[GitHub Pages](https://jij-inc.github.io/ommx/ja/introduction.html)でホストされていましたが、v3ではSphinxに完全移行し、[ReadTheDocs](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/)でホスティングを開始しました。GitHub Pagesは2.5.1の段階のドキュメントが引き続きホストされますが、今後の更新はReadTheDocsのみで行われます。
