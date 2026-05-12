# OMMX Python SDK 3.0.x

```{note}
Python SDK 3.0.0にはAPIの破壊的な変更が含まれます。マイグレーションガイドを [Python SDK v2 to v3 Migration Guide](https://github.com/Jij-Inc/ommx/blob/main/PYTHON_SDK_MIGRATION_GUIDE.md) にまとめてあります。
```

## Unreleased

直近のリリース以降にマージされた変更を、このセクションに順次追記していきます。次のリリース時に新しいバージョンのセクションへ昇格します。

### ⚠ Artifact API: archive は交換用フォーマットに ([#872](https://github.com/Jij-Inc/ommx/pull/872))

v3 では SQLite Local Registry を唯一の正規ストアと位置づけ、`.ommx` アーカイブは純粋に交換用フォーマットとして扱う形に再整理しました。{class}`~ommx.artifact.ArtifactBuilder` と {class}`~ommx.artifact.Artifact` に破壊的変更が入っています:

- {func}`ArtifactBuilder.new_archive <ommx.artifact.ArtifactBuilder.new_archive>` → {func}`ArtifactBuilder.new <ommx.artifact.ArtifactBuilder.new>` + 新メソッド {func}`Artifact.save <ommx.artifact.Artifact.save>`。
- {func}`ArtifactBuilder.new_archive_unnamed <ommx.artifact.ArtifactBuilder.new_archive_unnamed>` → {func}`ArtifactBuilder.new_anonymous <ommx.artifact.ArtifactBuilder.new_anonymous>` + `Artifact.save(path)`。匿名アーティファクトは `None` の代わりに `<registry-id8>.ommx.local/anonymous:<timestamp>-<nonce>` 形式の自動生成された image_name を持ちます。
- {func}`Artifact.load_archive <ommx.artifact.Artifact.load_archive>` は移行エラーを投げるようになり、2 つの置換メソッドへ誘導します: {func}`Artifact.import_archive <ommx.artifact.Artifact.import_archive>` (アーカイブを永続 SQLite Local Registry に import する v3 の後継、書き込み副作用あり) と {func}`Artifact.inspect_archive <ommx.artifact.Artifact.inspect_archive>` (registry に書き込まずに manifest + layer descriptors を読む、{class}`ArchiveManifest <ommx.artifact.ArchiveManifest>` を返却)。v2 の `load_archive` は registry 副作用無しで in-place 読み込みする API でした。リネームによって、アップグレード時に静かに registry に書き込まれることを防ぎ、意味論変更を明示します。`ArtifactBuilder.new_archive_unnamed` が生成していた `org.opencontainers.image.ref.name` 注釈のない v2 アーカイブは、`import_archive` が import 時に匿名名を合成して受け入れます (`inspect_archive` は read-only のため synthesis 用の registry が無く、`ArchiveManifest.image_name = None` でそのまま返却します)。
- CLI `ommx push <archive>` / `ommx push <oci-dir>` は廃止 — レジストリに load してから image name で push する 2 段階フローへ移行してください。
- 新 CLI `ommx artifact prune-anonymous [--dry-run]` で蓄積した匿名 build エントリを一括削除可能。

before / after コード例と移行チェックリストは [Python SDK v2→v3 Migration Guide §13](https://github.com/Jij-Inc/ommx/blob/main/PYTHON_SDK_MIGRATION_GUIDE.md#13-artifact-api-archive-becomes-an-exchange-format) を参照してください。

## 3.0.0 Alpha 3

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_3.0.0a3-orange?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-3.0.0a3)

詳細な変更点は上のGitHub Releaseをご覧ください。以下に主な変更点をまとめます。これはプレリリースバージョンです。APIは最終的なリリースまでに変更される可能性があります。

### ⚠ `*_df` アクセサがメソッドに変更 + `include=` 追加 + Sidecar DataFrame ([#846](https://github.com/Jij-Inc/ommx/pull/846))

`Instance` / `ParametricInstance` / `Solution` / `SampleSet` のすべての `*_df` アクセサを `#[getter]` プロパティから通常のメソッドに変更しました。プロパティアクセスからメソッド呼び出しに移行する必要があります:

```python
# Before
df = solution.constraints_df

# After
df = solution.constraints_df()
```

ワイドな `*_df` メソッドには `include` 引数が追加され、メタデータ系・パラメータ系のカラムをそれぞれ ON/OFF できます。デフォルトの `include=("metadata", "parameters")` は v2 互換のワイド形を維持します:

```python
solution.decision_variables_df()                       # core + metadata + parameters
solution.decision_variables_df(include=[])             # core only
solution.decision_variables_df(include=["metadata"])   # core + metadata
solution.decision_variables_df(include=["parameters"]) # core + parameters
```

加えて、SoA メタデータストアを直接読む 6 種類の long-format / id-indexed sidecar アクセサが追加されました。`kind=` で対象の制約ファミリーを切り替えます (`"regular"` / `"indicator"` / `"one_hot"` / `"sos1"`、デフォルト `"regular"`):

- `constraint_metadata_df(kind=...)` — id-indexed (`name` / `subscripts` / `description`)
- `constraint_parameters_df(kind=...)` — long format (`{kind}_constraint_id` / `key` / `value`)
- `constraint_provenance_df(kind=...)` — long format (`{kind}_constraint_id` / `step` / `source_kind` / `source_id`)
- `constraint_removed_reasons_df(kind=...)` — long format (`{kind}_constraint_id` / `reason` / `key` / `value`)
- `variable_metadata_df()` — id-indexed
- `variable_parameters_df()` — long format

Sidecar の index 名はファミリーごとに qualified (`regular_constraint_id` / `indicator_constraint_id` / `one_hot_constraint_id` / `sos1_constraint_id` / `variable_id`) になっており、別 ID 空間どうしを誤って `df.join()` した場合に `df.head()` 等で気づきやすくなっています。`*_parameters_df` / `*_removed_reasons_df` の行は `(id, key)` 順にソート済み、空の long-format DataFrame もスキーマ列だけ持つ形で返ります。

### ⚠ `removed_reason` カラムを `include=` でゲート ([#796](https://github.com/Jij-Inc/ommx/pull/796), [#847](https://github.com/Jij-Inc/ommx/pull/847))

v2.5.1 までは {meth}`Solution.constraints_df <ommx.v1.Solution.constraints_df>` に `removed_reason` カラムが常に含まれていました。`include=` による初期のゲート化は 3.0.0a2 (#796) で導入され、3.0.0a3 では上記の `kind=` / `include=` / `removed=` dispatch 形に整理されています (#847)。`include=` の `"removed_reason"` フラグでカラムを有効化する形で、これは reason 名と `removed_reason.{key}` パラメータカラムをまとめて制御するユニットフラグです。評価前に削除されていなかった行はそれらのカラムが NA になります。

```python
# Before (2.5.1)
df = solution.constraints_df  # 'removed_reason' カラムを含む

# After (3.0.0a3 — `*_df` はメソッドになりました)
df = solution.constraints_df()  # removed_reason カラムなし
df = solution.constraints_df(include=("metadata", "parameters", "removed_reason"))
# ↳ removed_reason / removed_reason.{key} が追加（active 行は NA）
```

`kind=` / `include=` の形は {class}`~ommx.v1.SampleSet` でも同じです。{class}`~ommx.v1.Instance` / {class}`~ommx.v1.ParametricInstance` では、`removed=True` を渡すと active と removed の両方が同じ DataFrame に並び、`"removed_reason"` が自動的に有効化されるので、active 行と removed 行を見分けることができます。

### ⚠ 部品型から `to_bytes` / `from_bytes` を削除 ([#845](https://github.com/Jij-Inc/ommx/pull/845))

以下の部品型からバイト列シリアライズを削除しました:

- {class}`~ommx.v1.Function`, {class}`~ommx.v1.Linear`, {class}`~ommx.v1.Quadratic`, {class}`~ommx.v1.Polynomial`
- {class}`~ommx.v1.Parameter`
- {class}`~ommx.v1.NamedFunction`, {class}`~ommx.v1.EvaluatedNamedFunction`, {class}`~ommx.v1.SampledNamedFunction`
- {class}`~ommx.v1.DecisionVariable`, {class}`~ommx.v1.EvaluatedDecisionVariable`, {class}`~ommx.v1.SampledDecisionVariable`

これらのメソッドは元々、Python SDK が独自の protobuf ベースのラッパー層を持っていた時代に Python ↔ Rust 境界を跨ぐたびにシリアライズが必要だったために用意されていたものでした。v3 で全型を PyO3 から直接再エクスポートする方針に切り替わったことでこの境界自体が消え、要素単位のバイト列ラウンドトリップは役目を終えています。今後予定しているメタデータ管理方式の見直しに合わせて維持し続けるコストも見合わなくなったため、ここで廃止します。永続化やプロセス間でのデータ交換が必要な場合は、これまで通りコンテナ型（{class}`~ommx.v1.Instance` / {class}`~ommx.v1.ParametricInstance` / {class}`~ommx.v1.Solution` / {class}`~ommx.v1.SampleSet`）と evaluate 用の DTO（{class}`~ommx.v1.State` / {class}`~ommx.v1.Samples` / {class}`~ommx.v1.Parameters`）の `to_bytes` / `from_bytes` をご利用ください。

### 🆕 メタデータ書き込みスルーラッパー: `AttachedConstraint` / `AttachedDecisionVariable` ([#849](https://github.com/Jij-Inc/ommx/pull/849), [#850](https://github.com/Jij-Inc/ommx/pull/850), [#852](https://github.com/Jij-Inc/ommx/pull/852))

`Instance.add_constraint` / `instance.constraints[id]` と `ParametricInstance` 側の対応するアクセサが、snapshot のコピーではなく親ホストに紐付いた書き込みスルーハンドルを返すようになりました。読み出しはホストから live に取得し、メタデータの setter はホスト側 SoA メタデータストアに直接書き込まれるため、同じ id を指す 2 つのハンドルは常に同じ状態を観測します。

```python
c = instance.add_constraint(x + y == 0)         # AttachedConstraint が返る
c.set_name("budget")                             # instance に書き込まれる
assert instance.constraints[c.constraint_id].name == "budget"
```

書き込みスルー型は 5 種類: {class}`~ommx.v1.AttachedConstraint`, {class}`~ommx.v1.AttachedIndicatorConstraint`, {class}`~ommx.v1.AttachedOneHotConstraint`, {class}`~ommx.v1.AttachedSos1Constraint`, {class}`~ommx.v1.AttachedDecisionVariable`。{class}`~ommx.v1.Constraint` / {class}`~ommx.v1.DecisionVariable` の構造はこれまでと変わらず、モデリング入力（演算子オーバーロードや `Instance.from_components`）に使う snapshot ラッパーとして引き続き利用します。各 `AttachedX` には、ホストへの back-reference を切り離して等価な snapshot を取り出すための `.detach()` が用意されています。

同じ変更の一環として、`instance.decision_variables` の戻り値が `list[DecisionVariable]` (snapshot) から `list[AttachedDecisionVariable]` に変更され、`instance.constraints` や特殊制約アクセサと整合的になりました。

### 🆕 OpenTelemetryベースのトレーシング/プロファイリング ([#816](https://github.com/Jij-Inc/ommx/pull/816), [#823](https://github.com/Jij-Inc/ommx/pull/823), [#826](https://github.com/Jij-Inc/ommx/pull/826), [#828](https://github.com/Jij-Inc/ommx/pull/828), [#829](https://github.com/Jij-Inc/ommx/pull/829))

従来の `log` + `pyo3-log` 経由のPython `logging` ブリッジを廃止し、Rustコアを `tracing` + `pyo3-tracing-opentelemetry` ベースに切り替えて、Python OTel SDKを通じて可視化できるようになりました。

`ommx.tracing` モジュールに2つの入口を用意しています:

- **`%%ommx_trace`** — Jupyterセル単位でスパンツリーとChrome Trace JSONダウンロードリンクを表示するセルマジック
- **`capture_trace` / `@traced`** — 通常のPythonスクリプト／テスト／CIから同じ機能を使うためのコンテキストマネージャとデコレータ

詳しい使い方、独自 `TracerProvider` の設定方法、トラブルシューティングは [トレースとプロファイリング](../user_guide/tracing.md) を参照してください。

### 🆕 Solver / Sampler Adapter のトレーシング対応 ([#833](https://github.com/Jij-Inc/ommx/pull/833))

OMMX の各 Adapter が solve / sample 1回につき3本の OpenTelemetry スパンを出すようになりました。上記のトレーシングパイプラインから、Adapter が実際に時間を使う3つのフェーズそれぞれの経過時間を計測できます。

- **`convert`** — OMMX の `Instance` からソルバーネイティブな問題への変換
- **`solve`** / **`sample`** — ソルバー／サンプラーへの呼び出し自体
- **`decode`** — 戻ってきた解を `Solution` / `SampleSet` に変換する処理（内部では Rust 側 `evaluate` のスパンがネストされます）

Adapter ごとに異なる tracer 名を使っているので、ツリービューで solver ごとの実行を識別しやすくなっています:

| Adapter | Tracer | Spans |
|---|---|---|
| `ommx-pyscipopt-adapter` | `ommx.adapter.pyscipopt` | `convert` / `solve` / `decode` |
| `ommx-highs-adapter` | `ommx.adapter.highs` | `convert` / `solve` / `decode` |
| `ommx-python-mip-adapter` | `ommx.adapter.python_mip` | `convert` / `solve` / `decode` |
| `ommx-openjij-adapter` | `ommx.adapter.openjij` | `convert` / `sample` / `decode` |

```python
from ommx.tracing import capture_trace
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

with capture_trace() as trace:
    solution = OMMXPySCIPOptAdapter.solve(instance)

print(trace.text_tree())  # convert / solve / decode が所要時間付きで表示される
```

スパンは標準の OpenTelemetry API 経由で発行されるため、`TracerProvider` が設定されていなければ no-op となり、トレーシングを使わないユーザーには実行コストがかかりません。

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

通常制約に加えて以下の3種類の特殊制約を、すべて第一級の制約型として `Instance.from_components` に `indicator_constraints=` / `one_hot_constraints=` / `sos1_constraints=` として渡せるようになりました。{class}`~ommx.v1.Solution` / {class}`~ommx.v1.SampleSet` でも、{meth}`~ommx.v1.Solution.constraints_df` を `kind=` で切り替えるだけで参照できます。

- {class}`~ommx.v1.IndicatorConstraint` — バイナリ変数による条件付き制約 (新規追加)
- {class}`~ommx.v1.OneHotConstraint` — 従来 `ConstraintHints.OneHot` として扱われていた one-hot 制約
- {class}`~ommx.v1.Sos1Constraint` — 従来 `ConstraintHints.Sos1` として扱われていた SOS1 制約

具体的な使い方、評価結果の参照、Indicator 制約の relax / restore ワークフローについては [特殊制約型](../user_guide/special_constraints.md) を参照してください。

これに伴い旧 API である `ConstraintHints` / `OneHot` / `Sos1` クラス、`Instance.constraint_hints` プロパティ、PySCIPOpt Adapter の `use_sos1` フラグは削除されています。

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
