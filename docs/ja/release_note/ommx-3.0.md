# OMMX Python SDK 3.0.x

```{note}
Python SDK 3.0.0にはAPIの破壊的な変更が含まれます。マイグレーションガイドを [Python SDK v2 to v3 Migration Guide](https://github.com/Jij-Inc/ommx/blob/main/PYTHON_SDK_MIGRATION_GUIDE.md) にまとめてあります。
```

## Unreleased

直近のリリース以降にマージされた変更を、このセクションに順次追記していきます。次のリリース時に新しいバージョンのセクションへ昇格します。

### 🆕 Adapter 固有の solve diagnostics ([#913](https://github.com/Jij-Inc/ommx/pull/913))

Solver Adapter に、共通の {class}`~ommx.v1.Solution` 結果には入らない backend solver 側の情報を保持するための adapter 固有 diagnostics channel を追加しました。adapter を直接呼ぶ場合は、予約済みの `diagnostics` keyword から {class}`~ommx.adapter.DiagnosticCollector` を {meth}`~ommx.adapter.SolverAdapter.solve` に渡せます。一方、{meth}`~ommx.experiment.Run.log_solve` はこの keyword を内部で管理し、記録された diagnostics を Experiment の各 {class}`~ommx.experiment.Solve` に保存します。

PySCIPOpt Adapter は、SCIP の `BESTSOLFOUND` と `DUALBOUNDIMPROVED` callback から {class}`~ommx_pyscipopt_adapter.SCIPProgressSnapshot` diagnostics を出力し、`model.optimize()` の後に {class}`~ommx_pyscipopt_adapter.SCIPTerminationReport` を出力するようになりました。termination report には SCIP の status、primal / dual bound、gap、incumbent objective value、node 数、LP / cut / solution counter、primal-dual integral、求解時間、SCIP / PySCIPOpt version metadata が含まれます。typed collector の中身や Experiment から読み出した dictionary は {class}`~ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer` で records または pandas DataFrame に後処理できます。これらの report は OMMX Solution へ decode する前に記録されるため、termination report は infeasible や unbounded の検出などで decode が adapter exception を投げる場合でも確認できます。

詳しい API の使い方と PySCIPOpt report の各 field については [Adapter 固有 diagnostics](../user_guide/adapter_diagnostics.md) を参照してください。

## 3.0.0 Alpha 5

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_3.0.0a5-orange?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-3.0.0a5)

詳細な変更点は上のGitHub Releaseをご覧ください。以下に主な変更点をまとめます。これはプレリリースバージョンです。APIは最終的なリリースまでに変更される可能性があります。

### 🆕 Run 単位の Experiment trace 保存 ([#910](https://github.com/Jij-Inc/ommx/pull/910), [#916](https://github.com/Jij-Inc/ommx/pull/916))

{class}`~ommx.experiment.Experiment`、{meth}`~ommx.experiment.Experiment.with_temp_local_registry`、{meth}`~ommx.experiment.Experiment.fork` が `store_trace=True` を受け取れるようになりました。有効化すると、各 `with experiment.run()` context 内で発生した OpenTelemetry span を capture し、close 済みの {class}`~ommx.experiment.SealedRun` に trace を 1 つ保存します。保存された trace は {attr}`~ommx.experiment.SealedRun.trace` から {class}`~ommx.tracing.TraceResult` として取得でき、commit、load、fork をまたいで保持されます。

詳しい trace workflow、renderer、OpenTelemetry の設定については [トレースとプロファイリング](../user_guide/tracing.ipynb) を参照してください。

```python
from ommx.experiment import Experiment
from ommx.tracing import render_text_tree
from ommx_highs_adapter import OMMXHighsAdapter

with Experiment.with_temp_local_registry(store_trace=True) as experiment:
    with experiment.run() as run:
        run.log_solve(OMMXHighsAdapter, instance)

loaded = Experiment.from_artifact(experiment.artifact)
trace = loaded.runs[0].trace
if trace is not None:
    print(render_text_tree(trace))
```

保存される payload は OTLP protobuf です。{class}`~ommx.tracing.TraceResult` は exported request を保持し、flatten された `spans` を公開し、`otlp_protobuf()` / `from_otlp_protobuf()` で往復変換できます。text / Chrome trace renderer も `Run`、`solve`、`convert`、`call`、`decode` など domain-oriented な span 名を使い、debug 用の source attribute を隠しつつ instrumentation scope を表示するようになりました。

### ⚠ Experiment attachment は name-indexed API に整理 ([#924](https://github.com/Jij-Inc/ommx/pull/924))

Experiment / Run の attachment は、Experiment config 内の name-indexed table として保存されるようになりました。公開 Python API は名前ベースです: `attachment_names`、`attachment_media_type(name)`、`get_attachment(name)`、`get_json(name)` や `get_instance(name)` などの型付き getter、`get_blob(name)`、`get_with_codec(...)`、`write_attachment(...)` を使います。

```python
loaded = Experiment.from_artifact(experiment.artifact)

for name in loaded.attachment_names:
    print(name, loaded.attachment_media_type(name))
    value = loaded.get_attachment(name)
```

以前の 3.0 alpha で提供していた descriptor-oriented な attachment view は削除しました。これには `Experiment.experiment_attachments` と `SealedRun.attachments` が含まれます。registry-backed descriptor は内部実装に留め、attachment 名、media type、file export name、checkpoint metadata は descriptor annotation ではなく Experiment config に保持します。

### 🆕 Experiment checkpoint と中断 session からの復帰 ([#917](https://github.com/Jij-Inc/ommx/pull/917))

{class}`~ommx.experiment.Experiment` が途中状態を Local Registry の checkpoint として保存するようになりました。{class}`~ommx.experiment.Run` を close すると best-effort に draft checkpoint を書き、Experiment が例外で終了した場合は成功用の Experiment image reference を進めず、failed または interrupted checkpoint を書きます。close 済みの Run は attachment、solve、trace、run parameter を保持し、`KeyboardInterrupt` などで中断された Run も `"failed"` または `"interrupted"` の status として残ります。

Run close の境界、checkpoint からの復帰、Local Registry cleanup の挙動については [Experiment の復帰と cleanup](../user_guide/experiment.md) を参照してください。

最新の checkpoint から再開するには、元の Experiment image name を {meth}`~ommx.experiment.Experiment.restore_from_checkpoint` に渡します:

```python
from ommx.experiment import Experiment

image_name = "ghcr.io/example/team/experiment:notebook"

try:
    with Experiment(image_name) as experiment:
        with experiment.run() as run:
            run.log_parameter("solver", "highs")
            raise KeyboardInterrupt
except KeyboardInterrupt:
    pass

experiment = Experiment.restore_from_checkpoint(image_name)
assert experiment.image_name == image_name
```

正常に `commit()` された場合は、これまで通り requested image reference だけが publish され、残っている local checkpoint は削除されます。checkpoint Artifact handle や checkpoint image name は Python API には公開せず、ユーザーは元の Experiment image name を覚えておいて復帰します。

### 🆕 Local Registry cleanup ([#919](https://github.com/Jij-Inc/ommx/pull/919))

SQLite-backed Artifact registry をメンテナンスするための Local Registry cleanup command を `ommx` CLI に追加しました。`ommx gc` は Experiment checkpoint refs を含む SQLite refs から到達できない blob を report します。active Experiment write を誤って削除しないよう、grace period より新しい unreachable blob は保護されます。

破壊的な cleanup command はデフォルトでは report のみを行い、`--delete` 指定時だけ registry を変更します:

```bash
ommx prune-anonymous
ommx gc
ommx prune-anonymous --delete
ommx gc --delete
```

通常の report は raw digest ではなく件数とサイズを表示します。低レベルの診断が必要な場合は `--show-digests` を指定してください。

同じ cleanup 操作は Python SDK からも
{func}`ommx.artifact.prune_anonymous` と {func}`ommx.artifact.gc` として
呼べます。どちらもデフォルトでは report-only で、`delete=True` 指定時だけ
registry を変更し、notebook や script で扱いやすい structured report object を返します。

### 🆕 Experiment Attachment の型付き Codec ([#921](https://github.com/Jij-Inc/ommx/pull/921))

新しい {class}`ommx.experiment.attachments.AttachmentCodec` protocol により、Python payload 型を所有するパッケージ側で、その値を Experiment attachment として保存・復元する方法を定義できるようになりました。Codec class は media type と `encode` / `decode` を提供し、OMMX は Experiment-level / Run-level の `log_with_codec` と `get_with_codec` からそれを呼び出します。

JijModeling `Problem` 用の codec 例は、Experiment management tutorial の {ref}`添付できるデータ形式 <experiment-management-attachable-data-formats>` を参照してください。

```python
from ommx.experiment import Experiment


class TextCodec:
    media_type = "text/plain"

    @staticmethod
    def encode(value: str) -> bytes:
        return value.encode()

    @staticmethod
    def decode(data: bytes) -> str:
        return data.decode()


with Experiment.with_temp_local_registry() as experiment:
    experiment.log_with_codec(TextCodec, "note", "created outside OMMX")

loaded = Experiment.from_artifact(experiment.artifact)
assert loaded.get_with_codec(TextCodec, "note") == "created outside OMMX"
```

decode の前に保存済み attachment の media type を検証するため、attachment に対して誤った Codec を使った場合は、その Codec の `decode` が呼ばれる前にエラーになります。

### 🆕 Experiment へのファイル添付 ([#922](https://github.com/Jij-Inc/ommx/pull/922))

{class}`~ommx.experiment.Experiment` と {class}`~ommx.experiment.Run` に、OMMX の外で作られた既存ファイルを添付できるようになりました。`log_file` は指定されたファイルを Experiment Artifact の attachment blob としてコピーします。後から復元できるよう元ファイルの basename を metadata として保存し、media type は明示指定された値、または Rust SDK の content-based inference による推定値を使います。推定できない場合は `application/octet-stream` に fallback します。

commit 済み Experiment / Run の読み取りビューには、attachment blob を実ファイルとして書き戻す `write_attachment` も追加しました。binary file-like object を受け取るライブラリに渡したい場合は、既存の `get_blob` の戻り値を `io.BytesIO` で包んで使えます。

```python
import io
from pathlib import Path

from ommx.experiment import Experiment

with Experiment.with_temp_local_registry() as experiment:
    experiment.log_file("input-spreadsheet", "input.xlsx")

loaded = Experiment.from_artifact(experiment.artifact)
spreadsheet_file = io.BytesIO(loaded.get_blob("input-spreadsheet"))
Path("restored").mkdir(parents=True, exist_ok=True)
loaded.write_attachment("input-spreadsheet", "restored/input.xlsx")
```

## 3.0.0 Alpha 4

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_3.0.0a4-orange?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-3.0.0a4)

詳細な変更点は上のGitHub Releaseをご覧ください。以下に主な変更点をまとめます。これはプレリリースバージョンです。APIは最終的なリリースまでに変更される可能性があります。

### ⚠ SQLite-based Local Registry の導入 ([#871](https://github.com/Jij-Inc/ommx/pull/871), [#872](https://github.com/Jij-Inc/ommx/pull/872))

v3 では Artifact のローカル保存実体を SQLite-based Local Registry に整理しました。Artifact の blob は content-addressed storage に保存され、image name から manifest への参照や registry metadata は SQLite で管理されます。従来の disk OCI dir cache を前提にした API は廃止し、Local Registry 上に commit された Artifact を `save` / `push` / `load` する形に統一しています。

この変更と `Experiment` の導入に合わせて、旧 `ArtifactBuilder` は {class}`~ommx.artifact.ArtifactDraft` として整理しました。`ArtifactDraft` は「Local Registry に commit される前の下書き」を表し、commit 後の {class}`~ommx.artifact.Artifact` を `save` / `push` する、という意味論に揃えています。`.ommx` アーカイブは Local Registry へ import / export するための交換用フォーマットです。主な破壊的変更は次の通りです:

- `ArtifactBuilder.new_archive` → {func}`ArtifactDraft.new <ommx.artifact.ArtifactDraft.new>` + 新メソッド {func}`Artifact.save <ommx.artifact.Artifact.save>`。
- `ArtifactBuilder.new_archive_unnamed` → {func}`ArtifactDraft.new_anonymous <ommx.artifact.ArtifactDraft.new_anonymous>` + `Artifact.save(path)`。v2 の unnamed archive は文字通り image name を持たず、読み込み後も `None` として扱われていました。v3 の anonymous Artifact は Local Registry が `<registry-id8>.ommx.local/anonymous:<timestamp>-<nonce>` 形式の image name を自動生成するため、保存・再読込・cleanup の対象として扱えます。
- {func}`Artifact.load_archive <ommx.artifact.Artifact.load_archive>` は移行エラーを投げるようになり、2 つの置換メソッドへ誘導します: {func}`Artifact.import_archive <ommx.artifact.Artifact.import_archive>` (アーカイブを永続 SQLite Local Registry に import する v3 の後継、書き込み副作用あり) と {func}`Artifact.inspect_archive <ommx.artifact.Artifact.inspect_archive>` (registry に書き込まずに manifest + layer descriptors を読む、{class}`ArchiveManifest <ommx.artifact.ArchiveManifest>` を返却)。v2 の `load_archive` は registry 副作用無しで in-place 読み込みする API でした。リネームによって、アップグレード時に静かに registry に書き込まれることを防ぎ、意味論変更を明示します。`ArtifactBuilder.new_archive_unnamed` が生成していた `org.opencontainers.image.ref.name` 注釈のない v2 アーカイブは、`import_archive` が import 時に匿名名を合成して受け入れます (`inspect_archive` は read-only のため synthesis 用の registry が無く、`ArchiveManifest.image_name = None` でそのまま返却します)。
- CLI `ommx push <archive>` / `ommx push <oci-dir>` は廃止 — Local Registry に load してから image name で push する 2 段階フローへ移行してください。
- 新 CLI `ommx prune-anonymous [--delete]` はデフォルトで蓄積した匿名 commit エントリを report し、`--delete` 指定時だけ削除します。
- `ommx.get_image_dir(...)` と CLI `ommx image-dir <name>` を廃止しました。戻り値は v2 disk-cache の `<root>/<image_name>/<tag>/` パスで、v3 SQLite Local Registry の実際の保存先 (blob は content-addressed、ref は SQLite) とは無関係になっており、ユーザーをミスリードしていたため。既存の v2 cache は引き続き `ommx import-legacy` で移行できます。

before / after コード例と移行チェックリストは [Python SDK v2→v3 Migration Guide §13](https://github.com/Jij-Inc/ommx/blob/main/PYTHON_SDK_MIGRATION_GUIDE.md#13-artifact-api-archive-becomes-an-exchange-format) を参照してください。

### 🆕 Artifact ベースの実験管理 API: `ommx.experiment` ([#882](https://github.com/Jij-Inc/ommx/pull/882), [#885](https://github.com/Jij-Inc/ommx/pull/885), [#886](https://github.com/Jij-Inc/ommx/pull/886), [#903](https://github.com/Jij-Inc/ommx/pull/903))

実験の入力データ、実行条件、Solver/Sampler の結果を 1 つの OMMX Artifact として記録する `ommx.experiment` モジュールを追加しました。{class}`~ommx.experiment.Experiment`、{class}`~ommx.experiment.Run`、{class}`~ommx.experiment.Solve` を使って、Run ごとの比較パラメータ、attachment、solve 入出力を Local Registry に保存できます。

基本的な使い方、Experiment の共有、保存済み Experiment の読み込み、fork による派生実験の作り方は [実験管理チュートリアル](../tutorial/experiment_management.md) を参照してください。

### 🆕 `Run.log_solve` で solve 入出力と adapter options を記録 ([#902](https://github.com/Jij-Inc/ommx/pull/902))

{meth}`~ommx.experiment.Run.log_solve` を追加しました。`ommx.adapter.SolverAdapter` のサブクラスと {class}`~ommx.v1.Instance` を渡すと、adapter の `solve` を呼び出し、入力 Instance、出力 Solution、adapter クラス名、JSON-serializable な keyword arguments を {class}`~ommx.experiment.Solve` として保存します。

```python
from ommx.experiment import Experiment
from ommx_highs_adapter import OMMXHighsAdapter
from ommx.v1 import Instance, Solution

with Experiment() as experiment:
    with experiment.run() as run:
        solution = run.log_solve(OMMXHighsAdapter, instance, verbose=False)
        run.log_parameter("objective", solution.objective)

solve = experiment.runs[0].solves[0]
assert solve.adapter.endswith("OMMXHighsAdapter")
assert isinstance(solve.input, Instance)
assert isinstance(solve.output, Solution)
assert solve.output.feasible
assert solve.adapter_options == {"verbose": False}
```

adapter options は solve 単位のメタデータなので、Run の比較軸である {meth}`~ommx.experiment.Experiment.run_parameters_df` には入りません。DataFrame に出したい値は、これまで通り {meth}`~ommx.experiment.Run.log_parameter` で明示的に記録してください。

### 🆕 Experiment の fork と lineage ([#905](https://github.com/Jij-Inc/ommx/pull/905))

commit 済みの Experiment から新しい未 commit の Experiment を開始する {meth}`~ommx.experiment.Experiment.fork` を追加しました。fork 先は元の Experiment の attachments、Runs、Solves、Run parameters を引き継ぎますが、親 Experiment は変更されません。fork 先で新しい Run や attachment を追加して commit すると、親の manifest descriptor が OCI `subject` として記録されます。

```python
from ommx.experiment import Experiment
from ommx_highs_adapter import OMMXHighsAdapter

loaded = Experiment.load("ghcr.io/jij-inc/ommx/tutorial/experiment:baseline")

with loaded.fork("ghcr.io/jij-inc/ommx/tutorial/experiment:capacity-64") as child:
    with child.run() as run:
        run.log_parameter("capacity", 64)
        run.log_solve(OMMXHighsAdapter, instance, verbose=False)
```

fork は Artifact Manifest を新しく作りますが、Instance / Solution / attachment payload は Local Registry の content-addressed blob を参照するため、同じデータ本体を重複保存しません。fork した Experiment を `save` / `push` すると、親由来の Run や Solve も含む fork 後の Experiment 全体を共有できます。

### 🆕 `Instance.substitute` / `ParametricInstance.substitute` を追加 ([#891](https://github.com/Jij-Inc/ommx/pull/891), [#897](https://github.com/Jij-Inc/ommx/pull/897))

{meth}`~ommx.v1.Instance.substitute` と {meth}`~ommx.v1.ParametricInstance.substitute` を Python から使えるようにしました。決定変数 ID から置換後の {class}`~ommx.v1.Function` への辞書を渡すと、目的関数と有効な制約に現れる決定変数を in-place で代数的に書き換えます。`log_encode` の背後にある一般的な置換機構を直接使えるようになったため、unary encoding や one-hot encoding など独自の変数変換を書けます。

```python
from ommx.v1 import DecisionVariable, Instance

x = DecisionVariable.integer(0, lower=0, upper=3)
b = [DecisionVariable.binary(i) for i in (1, 2)]
instance = Instance.from_components(
    decision_variables=[x, *b],
    objective=x,
    constraints={},
    sense=Instance.MAXIMIZE,
)

instance.substitute({0: b[0] + 2 * b[1]})
assert str(instance.objective) == "Function(x1 + 2*x2)"
```

この API はあくまで代数的な書き換えです。置換元変数の `kind` / `lower` / `upper` を、置換後の式に対する制約へ自動変換しません。最適化問題として同値な変換にしたい場合は、domain を保つ encoding を使うか、必要な linking / bound 制約を呼び出し側で追加してください。`ParametricInstance.substitute` では置換後の式に parameter を残せるため、`with_parameters` で具体値を入れる前に記号的な変数変換を適用できます。

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

詳しい使い方、独自 `TracerProvider` の設定方法、トラブルシューティングは [トレースとプロファイリング](../user_guide/tracing.ipynb) を参照してください。

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
from ommx.tracing import capture_trace, render_text_tree
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

with capture_trace() as trace:
    solution = OMMXPySCIPOptAdapter.solve(instance)

print(render_text_tree(trace))  # convert / solve / decode が所要時間付きで表示される
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
