# OMMX Python SDK 3.0.x

```{note}
Python SDK 3.0.0にはAPIの破壊的な変更が含まれます。マイグレーションガイドを [Python SDK v2 to v3 Migration Guide](https://github.com/Jij-Inc/ommx/blob/main/PYTHON_SDK_MIGRATION_GUIDE.md) にまとめてあります。
```

## 3.0.0 Alpha 2

### `Constraint.id` フィールドの削除 ([#806](https://github.com/Jij-Inc/ommx/pull/806))

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

### 特殊制約型の整備 ([#789](https://github.com/Jij-Inc/ommx/pull/789), [#790](https://github.com/Jij-Inc/ommx/pull/790), [#795](https://github.com/Jij-Inc/ommx/pull/795), [#796](https://github.com/Jij-Inc/ommx/pull/796), [#798](https://github.com/Jij-Inc/ommx/pull/798))

通常制約に加えて以下の3種類の特殊制約を、すべて第一級の制約型として `Instance.from_components` に `indicator_constraints=` / `one_hot_constraints=` / `sos1_constraints=` として渡せるようになりました。{class}`~ommx.v1.Solution` / {class}`~ommx.v1.SampleSet` にも対応する DataFrame (`*_constraints_df`) が提供されます。

- {class}`~ommx.v1.IndicatorConstraint` — バイナリ変数による条件付き制約 (新規追加)
- {class}`~ommx.v1.OneHotConstraint` — 従来 `ConstraintHints.OneHot` として扱われていた one-hot 制約
- {class}`~ommx.v1.Sos1Constraint` — 従来 `ConstraintHints.Sos1` として扱われていた SOS1 制約

具体的な使い方、評価結果の参照、Indicator 制約の relax / restore ワークフローについては [特殊制約型](../user_guide/special_constraints.md) を参照してください。

これに伴い旧 API である `ConstraintHints` / `OneHot` / `Sos1` クラス、`Instance.constraint_hints` プロパティ、PySCIPOpt Adapter の `use_sos1` フラグは削除されています。

### `removed_reason` カラムを別テーブルに分離 ([#796](https://github.com/Jij-Inc/ommx/pull/796))

v2.5.1 までは {attr}`Solution.constraints_df <ommx.v1.Solution.constraints_df>` に `removed_reason` カラムが含まれていましたが、v3.0.0a2 ではこれを {attr}`Solution.removed_reasons_df <ommx.v1.Solution.removed_reasons_df>` という別テーブルに分離しました。従来の形が必要な場合は join してください。同じ変更が {class}`~ommx.v1.SampleSet` にも適用されています。

```python
# Before (2.5.1)
df = solution.constraints_df  # 'removed_reason' カラムを含む

# After (3.0.0a2)
df = solution.constraints_df.join(solution.removed_reasons_df)
```

Indicator / OneHot / SOS1 それぞれに対応する `*_removed_reasons_df` も提供されています。

### Adapter Capability モデル ([#790](https://github.com/Jij-Inc/ommx/pull/790), [#805](https://github.com/Jij-Inc/ommx/pull/805), [#810](https://github.com/Jij-Inc/ommx/pull/810), [#811](https://github.com/Jij-Inc/ommx/pull/811), [#814](https://github.com/Jij-Inc/ommx/pull/814))

特殊制約の追加に伴い、Adapter が自身のサポートする制約型を `ADDITIONAL_CAPABILITIES` クラス属性で宣言する仕組みを導入しました。`super().__init__(instance)` が呼ばれると、未宣言の特殊制約は自動的に通常の制約へ変換（indicator/SOS1 は Big-M、one-hot は線形等式）されてから solver に渡されます。

**既存の OMMX Adapter は Python SDK 3.0.0 に対応するため `super().__init__(instance)` を呼ぶよう変更する必要があります。** 現在 PySCIPOpt Adapter は Indicator 制約と SOS1 をサポート宣言しています。

詳細および手動での変換 API については [Adapter Capability モデルと制約変換](../user_guide/capability_model.md) を参照してください。

### numpy スカラ型のサポート ([#794](https://github.com/Jij-Inc/ommx/pull/794))

{class}`~ommx.v1.Function` のコンストラクタが `numpy.integer` および `numpy.floating` を受け付けるようになりました。v2.5.1 では `Function(numpy.int64(3))` は `TypeError` になっていました。

## 3.0.0 Alpha 1

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_3.0.0a1-orange?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-3.0.0a1)

詳細な変更点は上のGitHub Releaseをご覧ください。以下に主な変更点をまとめます。これはプレリリースバージョンです。APIは最終的なリリースまでに変更される可能性があります。

### `ommx.v1` および `ommx.artifact` 型の完全なRust再エクスポート ([#770](https://github.com/Jij-Inc/ommx/pull/770), [#771](https://github.com/Jij-Inc/ommx/pull/771), [#774](https://github.com/Jij-Inc/ommx/pull/774), [#775](https://github.com/Jij-Inc/ommx/pull/775), [#782](https://github.com/Jij-Inc/ommx/pull/782))

Python SDK 3.0.0は完全にRust/PyO3ベースになります。
2.0.0ではコア実装がRustで書き直されましたが、互換性のためにPythonラッパークラスが残されていました。3.0.0ではそれらのPythonラッパーを完全に削除し、`ommx.v1` およｂ `ommx.artifact` の全型がRustからの直接再エクスポートとなり、`protobuf` Pythonランタイム依存も排除されます。また旧来PyO3実装へのアクセスを提供していた `.raw` 属性も廃止されました。

### Sphinxへの移行、ReadTheDocsでのホスティング開始 ([#780](https://github.com/Jij-Inc/ommx/pull/780), [#785](https://github.com/Jij-Inc/ommx/pull/785))

v2ではSphinxベースのAPI ReferenceとJupyter Bookベースのドキュメントがそれぞれ[GitHub Pages](https://jij-inc.github.io/ommx/ja/introduction.html)でホストされていましたが、v3ではSphinxに完全移行し、[ReadTheDocs](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/)でホスティングを開始しました。GitHub Pagesは2.5.1の段階のドキュメントが引き続きホストされますが、今後の更新はReadTheDocsのみで行われます。
