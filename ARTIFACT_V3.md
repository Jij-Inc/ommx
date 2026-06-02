# OMMX Experiment / Artifact v3 提案

OMMX v3 における Experiment / Run / Solve / Attachment / OTel span/event schema / Lineage / GC の未実装領域をまとめる提案。

本ファイルは開発中の一時文書である。実装済みの API 仕様は通常の Sphinx documentation / API reference / module rustdoc を正本にし、本書には残さない。既に実装済みの Artifact manifest format、Local Registry、archive / remote transport の移行ログは本書では扱わない。必要な前提は `rust/ommx/doc/artifact_design.md` と `ommx::artifact::local_registry` の rustdoc を参照する。

本書の主眼は、MINTO が提供していた実験管理 UX を OMMX-owned な機能として再設計し、記録データ、実行 telemetry、Artifact version、lineage を一貫したモデルに落とすことである。

## 0. 用語集

本書では、データモデル上の状態 / 不変条件と、高レベルの操作を分けて扱う。操作名は「どの状態遷移を起こすか」によって定義し、状態名の代用にはしない。

### 0.1 データモデル上の用語

| 用語 | 意味 |
|---|---|
| Descriptor | OCI descriptor。digest / size / media type / annotations を主張するが、その bytes が OMMX Local Registry に存在することは保証しない。 |
| StoredDescriptor | 特定の Local Registry の BlobStore に、descriptor が指す bytes が存在することを保証する OMMX 側の型。保証するのは存在であり、その呼び出しが新規に書いたことではない。 |
| Unsealed | 複数 blob からなるデータ構造の一部または全部の component blob は保存済みだが、全体を指す root manifest blob はまだ保存されていない状態。データモデル上の mutable state。 |
| SealedArtifact | root manifest blob が BlobStore に保存され、全体を指す root descriptor が存在する状態を表す OMMX 側の型。inner descriptor は `StoredDescriptor` だが、component blob ではなく root manifest であることを型で区別する。 |
| Published | sealed root descriptor が Local Registry の IndexStore で ref に対応づけられた状態。publish は名前解決の更新であり、payload blob の保存ではない。 |

### 0.2 API lifecycle 上の用語

| 用語 | 意味 |
|---|---|
| Draft | ユーザーまたは SDK が変更中の mutable object。`ArtifactDraft` や `Experiment` は unsealed state を所有するが、`Draft` 自体は storage state ではない。 |
| LocalArtifact | sealed / published artifact を読むための immutable view。mutable session に戻す API は持たない。 |

### 0.3 操作に関する用語

| 操作 | レイヤ | 状態遷移 / 責務 |
|---|---|---|
| Store | Local Registry / BlobStore | bytes を content-addressed blob として保存し、検証後に `StoredDescriptor` を得る。 |
| Seal | データモデル | unsealed state から root manifest blob を作り、`SealedArtifact` を得る。ref は更新しない。 |
| Publish | Local Registry / IndexStore | `SealedArtifact` の root descriptor を ref に対応づけ、Published state にする。payload blob は保存しない。 |
| Commit | SDK lifecycle | Draft / Experiment を immutable Artifact として確定する高レベル操作。内部で必要な Store、Seal、通常は Publish を行う。 |
| Import | SDK / Registry boundary | Local Registry 外の source（OCI directory / archive / remote registry など）から identity を保って bytes と descriptors を取り込む高レベル操作。内部で source の読み取り、Store、必要に応じた Publish を行う。 |

`Staged` は data model の用語として使わない。以前 `Draft` / `Staged` が混在して表していた「blob は逐次保存されるが、全体としてはまだ root manifest を持たない」状態は `Unsealed` と呼ぶ。

## 1. 実装済み API の正本

実装済みの Experiment / Run / Solve API、context manager の挙動、`log_solve`、`fork`、`rename`、archive / push 連携、trace storage / renderer API は本書では列挙しない。Python API は `ommx.experiment` / `ommx.tracing` の API Reference、Rust core は `ommx::experiment` の module rustdoc を正本にする。Tracing のユーザー向け説明は `docs/en/user_guide/tracing.ipynb` / `docs/ja/user_guide/tracing.ipynb` を参照する。

ユーザー向けの既存チュートリアルとして `docs/ja/tutorial/experiment_management.md` がある。これは別途改稿方針を決めるまで本書の整理では書き換えない。

本書に残すのは、API Reference に落とし込む前の未実装設計、または実装済み API から明確に外れている残作業だけである。

## 2. 参照する既存 UX

MINTO 互換 API は維持しない。ただし、以下のユーザー体験は OMMX 側に移す。

### 2.1 MINTO から維持する体験

MINTO の中心 UX は、実験全体に属するデータと run ごとに属するデータを分ける 2 つの保存空間モデルである。OMMX ではこの区分を Experiment space / Run space として維持する。

| 保存空間 | 目的 | 例 |
|---|---|---|
| Experiment space | 全 run で共有される context | dataset name, source problem, baseline config |
| Run space | 各 run の定式化、条件、実行環境、結果 | run parameters, run environment, candidate instance, disabled constraints, solution, sample set |

MINTO の archive / registry sharing UX も維持する。ただし OMMX では共有の正本は Artifact である。`save_as_ommx_archive()` / `load_from_ommx_archive()` のような名前は compatibility layer として置いてもよいが、中心概念は `commit()`、`Experiment.load(...)`、`Artifact.load(...)` に寄せる。

MINTO の `Experiment` context manager は `auto_saving` が有効なら保存 directory へ flush していたが、archive commit までは行っていなかった。OMMX では BlobStore-backed autosave を常に持つため、`with Experiment(...)` の意味を「保存 directory への flush」ではなく「Experiment session の正常終了と Artifact commit」に寄せる。

### 2.2 MLflow / W&B との差分

MLflow や W&B は主に Run lifecycle を context manager で閉じる。OMMX でも `with exp.run()` は Run を close / finalize する。

一方、OMMX の実験管理では Experiment が単なる Run の namespace ではなく、共有入力、複数の定式化、比較 table、Artifact sharing をまとめる単位になる。数理最適化では、同じ source problem から定式化を変える、一部の制約を無効化する、relaxation を変える、solver parameters を変える、といった複数 run を比較する作業が多い。このため OMMX では `with Experiment(...)` も lifecycle を持ち、正常終了時に experiment bundle を Artifact として seal する。

## 3. Lifecycle

Experiment / Run の context manager、`commit()`、`fork()`、`restore_from_checkpoint()`、Run close autosave checkpoint、Run status (`finished|failed|interrupted`) は実装済みである。ユーザー向け挙動は Python API Reference の `ommx.experiment` と Rust SDK の `ommx::experiment` module rustdoc を正本にする。

本書に残す lifecycle 上の設計境界は以下だけである。

- 成功 Artifact は requested image ref を進める immutable Experiment Artifact である。
- checkpoint は requested success ref を進めない reserved local ref であり、ユーザーに Artifact handle として直接見せない。
- restore は original image name から checkpoint ref を再計算し、元の image name を持つ unsealed Experiment session として再開する。
- `log_*` 時点で BlobStore に保存済みだが、成功 commit や checkpoint manifest から到達できない blob は orphan blob として扱う。
- active Run の途中状態を process kill 後に復元する journal metadata は未設計である。
- checkpoint discovery / retention / GC policy は未設計である。

## 4. Experiment state model

Experiment state は以下からなる。

| 要素 | 内容 |
|---|---|
| Attachment list | Experiment / Run に添付された payload の順序付き list |
| Run parameter table | `run_id` と parameter name を key にした scalar table |
| Solve list | Run 内で発生した solver 呼び出し。input / output / solve parameter を持つ |

汎用保存抽象は domain object ではなく「payload を保存できる添付物」に近いため `Attachment` と呼ぶ。`Run` と `Solve` は domain object として扱い、Attachment はそれらに添付される payload descriptor への参照である。

```text
Experiment
  attachments        # Experiment scoped attachments
  runs
    parameters       # Run scoped scalar parameters
    attachments      # Run scoped opaque / auxiliary payloads
    solves
      input          # Instance
      output         # Solution
      parameters     # SolverAdapter に実際に渡された solve scoped parameters
```

`Run` は分割戦略、定式化戦略、アルゴリズム設定などの試行条件を表す。列生成法のように 1 つの run 内で master problem / pricing problem / subproblem を複数回 solve するケースがあるため、`Run` と `Solve` は 1:1 ではない。`Solve` は Run 内で発生した solver 呼び出しを表し、adapter 名、solver kwargs、backend status、elapsed time などは Run parameter ではなく Solve scoped metadata / parameters として扱う。

`Run` / `Solve` は Artifact sub manifest ではない。Artifact boundary は引き続き Experiment のみであり、Run と Solve は Experiment config 内の logical entity とする。

### 4.0 LayerRef と Descriptor の扱い

OCI では Descriptor は blob そのものではなく、blob を指す JSON metadata である。Descriptor 単体を保存する専用 blob はない。OMMX Artifact では payload blob を指す Descriptor は root OCI Image Manifest の `layers[]` に列挙される。

Experiment config が payload を参照する場合、同じ Descriptor を config 内に inline で複製すると、manifest `layers[]` 側の Descriptor と config 側の Descriptor が不一致になり得る。これを避けるため、目標モデルでは config 内に payload Descriptor を inline しない。代わりに zero-based `LayerRef` を使って、この config を所有する OCI Image Manifest の `layers[]` を参照する。

```text
LayerRef = u32  # index into the owning OCI Image Manifest's layers[]

ImageManifest.layers
  [0] Descriptor for experiment attachment
  [1] Descriptor for run 0 solve 0 input
  [2] Descriptor for run 0 solve 0 output

ExperimentConfig
  attachments: [LayerRef(0)]
  runs:
    - run_id: 0
      solves:
        - solve_id: 0
          input: LayerRef(1)
          output: LayerRef(2)
```

`LayerRef` は単独では意味を持たず、その config blob を `config` として参照している exact OCI Image Manifest と一緒にだけ解釈できる。`layers[]` の順序は manifest JSON の内容であり、manifest digest の一部である。順序を書き換えた場合は別 manifest / 別 Artifact になるため、OMMX Artifact としては layer order を semantic な descriptor table として扱ってよい。

Validation rules:

- `LayerRef.index < manifest.layers.len()`。
- `Solve.input` が参照する layer は OMMX Instance media type を持つ。
- `Solve.output` が参照する layer は OMMX Solution media type を持つ。
- Experiment / Run attachments は任意 media type を許す。
- `LayerRef` の重複参照は許してよい。同一 payload を複数 logical entity が参照することは合法である。
- loader は config と、それを所有する manifest の `layers[]` を合わせて解釈し、`layers[]` scan だけで Experiment / Run / Solve を推測しない。

Descriptor annotations は source of truth ではない。Config の構造と `LayerRef` が所属関係の正である。Descriptor annotation を持たせる場合も、generic inspector / compatibility / debug 用の補助情報に限定し、`run_id` / `solve_id` のような階層情報を重複保持しない。

### 4.1 Experiment space / Run space

Experiment object に対する `log_*` と Run object に対する `log_*` の API 仕様は実装済みであり、正本は API Reference の `ommx.experiment.Experiment` / `ommx.experiment.Run` と Rust SDK の `AttachmentLogger` rustdoc とする。

設計上の境界は、Experiment space と Run space を receiver で決め、暗黙の global context では決めないことである。Run scalar parameter は Attachment ではなく Run parameter table の cell として扱う。

### 4.2 Attachment

Attachment API、既知 media type の helper、descriptor access、blob read helper は実装済みであり、API Reference を正本にする。

本書では、Attachment は API / loader から見える名前付き payload であって、実装上の blob 所有単位ではない、という境界だけを残す。Attachment の分類軸は独立した enum ではなく OCI descriptor の `mediaType` に統一する。user-defined media type は OMMX core が schema を知らない payload の escape hatch とし、OMMX core は unknown media type を decode しない。

OMMX core は `jijmodeling` を import しない。domain-specific problem storage は external package が `media_type` と codec を登録して提供する。例えば `jijmodeling` の model payload は、`jijmodeling` package が media type / codec を所有し、OMMX には user-defined media type の Attachment として渡す。OMMX は descriptor を保持するだけで、parse / validation / round-trip guarantee はその media type owner の責務にする。

### 4.3 Run parameter table

Run parameter API と `run_parameters_df()` は実装済みであり、API Reference を正本にする。

設計上の境界は、Run parameter table を Attachment とは別に `run_id` と parameter name を key にした scalar table として持つことである。欠損は `null` value ではなく、その `(run_id, parameter_name)` cell が存在しないことで表す。

commit 時に materialize する Run parameter table JSON は column-oriented とする。

```json
{
  "columns": {
    "timelimit": {
      "type": "float64",
      "values": {
        "0": 1.0,
        "1": 10.0
      }
    },
    "solver": {
      "type": "string",
      "values": {
        "0": "scip",
        "1": "highs"
      }
    }
  }
}
```

`values` の key は `run_id` であり、存在しない key は missing cell を意味する。

物理化候補:

| 対象 | 候補 | 備考 |
|---|---|---|
| run parameter | Run ごとの parameter aggregate JSON | `run_parameters_df()` の入力になる |
| solve parameter | Solve ごとの string metadata map として Experiment config に含める | adapter 名、`json.dumps` した kwargs など。Run parameter には入れない |

この方針では、Run `parameter` は API / analysis 上は key 単位で扱えるが、Attachment ではなく Artifact layer の最小単位でもない。`Instance` や `Solution` のような大きな typed payload は Attachment / Solve input / Solve output として即 blob 化しやすいが、Run scalar parameter は commit 時に Run table payload として materialize する方が manifest / blob 数を抑えられる。Solve parameter は string metadata として Solve entry 内に保存し、kwargs は Python の `json.dumps` 結果を再解釈せず文字列のまま保持する。

### 4.4 Run metadata

Run status は実装済みであり、Rust SDK では enum、Python SDK では `SealedRun.status` の string property として公開される。Experiment config では `runs[].status` に `finished|failed|interrupted` として保存する。

例外要約は、実験に使ったコード、入力、実行環境と結びつかない限り原因究明に足る情報になりにくいため、Run metadata としては保存しない。elapsed time、実行環境属性、backend solver version などは引き続き未決定であり、Run attributes という aggregate payload は導入していない。

### 4.5 Build phase と Seal phase

Build / Seal / View の API lifecycle は実装済みであり、API Reference / rustdoc を正本にする。

本書に残す data-model 境界は、Seal / commit phase では最終 Experiment state を復元できる aggregate payload と `LayerRef` に固定する、という点である。Committed config では Attachment list と Solve list の保存順と重複を保持し、logical entry の dedup / normalization は行わない。

## 5. Adapter execution and diagnostics

### 5.1 Adapter 実行

`run.log_solve(...)` は実装済みであり、API Reference を正本にする。任意 callable を包む generic solver logging API は提供しない。

未設計として残すのは Sampler-oriented execution と diagnostics である。

- `log_sample` は `SamplerAdapter.sample(...)` を呼び、Run 内に SampleSet-oriented Solve / Sample entry を追加する。詳細な型分けは後続設計で決める。
- 全 run で共有される巨大な Instance を Solve input から参照する API は後続設計で検討する。
- 実行時間は OTel span から得ることを基本とし、Artifact に保存する結果 payload の正本にはしない。
- adapter が返す diagnostics を Artifact-backed evidence として記録する。

### 5.2 Diagnostics

ソルバーの native log / report は、構造化された結果の正本ではない。しかし、モデルが解けない、timeout する、infeasible になる、または想定外の挙動をするときの解析には不可欠な診断 evidence である。OMMX はこの evidence を Artifact に保持できる必要がある。

どれだけの診断情報を保存すべきかはソルバー依存である。OMMX core が stdout / stderr を盲目的に capture し、すべての backend に対する policy を決めるべきではない。代わりに、各 OMMX Adapter が対象 backend solver の診断 policy を持つ。

Adapter は Experiment / Run を直接操作しない。Adapter の直接利用は従来通り `Solution` / `SampleSet` だけを返し、Experiment 管理の有無によって暗黙に挙動を変えない。

```python
solution = OMMXPySCIPOptAdapter.solve(instance)
```

Diagnostics は optional sink protocol として opt-in にする。`run.log_solve(...)` / `run.log_sample(...)` は Run-scoped `DiagnosticsSink` を作り、対応している Adapter に明示的に渡す。未対応 Adapter は単に diagnostics を返さないだけで、solve / sample 自体は従来通り動く。

```python
with exp.run() as run:
    solution = run.log_solve(
        "scip",
        OMMXPySCIPOptAdapter,
        instance,
        time_limit=1.0,
    )

# log_solve 内部の概念:
sink = run.diagnostics_sink(adapter="scip")
solution = OMMXPySCIPOptAdapter.solve(
    instance,
    time_limit=1.0,
    diagnostics=sink,  # Adapter が対応している場合だけ使われる
)
```

直接利用でも user が diagnostics を欲しい場合は、`DiagnosticCollector` を明示的に渡せるようにする。

```python
collector = DiagnosticCollector()
solution = OMMXPySCIPOptAdapter.solve(instance, diagnostics=collector)
diagnostics = collector.entries
```

Adapter 側の責務:

- debug に有用な native log / report / summary を選ぶ。
- presolve log、node log、gap history、termination report、warning、backend status、backend version、sampler schedule など、ソルバー固有の evidence を capture する。
- truncate、compression、redaction、summary の policy を決める。
- optional `diagnostics` sink protocol に対応する。未対応なら何もしない。
- `Run` / `Experiment` は import しない。

OMMX core 側の責務:

- `run.log_solve(...)` / `run.log_sample(...)` で Solve-scoped `DiagnosticsSink` を作る。
- Adapter が `diagnostics` kwarg または同等の optional protocol に対応している場合だけ sink を渡す。
- diagnostics を media type と annotations を持つ Solve-scoped Attachment として保存する。
- diagnostics Attachment を committed Artifact に含める。
- diagnostics Attachment を要約し参照する OTel span attribute / event を出す。
- Adapter が例外を投げた場合でも、sink に書かれた diagnostics は Solve-scoped Attachment として残す。

diagnostics は 2 層構造にする。

| 層 | 役割 | 例 |
|---|---|---|
| Attachment / Artifact | diagnostic payload の正本 | raw solver log, compressed log, JSON termination report, gap timeline |
| OTel trace | lifecycle、summary、reference | diagnostic recorded event, size, truncation flag, Attachment name, solver status |

diagnostic Attachment の例:

```python
DiagnosticEntry(
    name="solver/scip/log",
    media_type="text/plain; charset=utf-8",
    data=log_bytes,
    annotations={
        "org.ommx.solver.name": "scip",
        "org.ommx.solver.diagnostic.kind": "log",
        "org.ommx.solver.log.truncated": "false",
    },
)
```

対応する OTel event:

```text
event: ommx.solver.diagnostic.recorded
attributes:
  ommx.attachment.name = "solver/scip/log"
  ommx.attachment.media_type = "text/plain; charset=utf-8"
  ommx.solver.name = "scip"
  ommx.solver.diagnostic.kind = "log"
  ommx.solver.diagnostic.size = ...
  ommx.solver.diagnostic.truncated = false
```

Phase 1 は diagnostics payload を Attachment / Artifact に保存し、OTel は summary と reference のみを持つ。OTel Logs signal への本格統合は Phase 2 以降で扱う。

## 6. OTel / Trace / Renderer

Experiment v3 は、記録データ、実行 telemetry、表示、Artifact version を明確に分ける。

| 領域 | 正本 | 例 |
|---|---|---|
| 記録データ | Artifact manifest / layers / aggregate payload | run parameters, metadata, objects, Instance, Solution, SampleSet |
| 実行 telemetry | OTel trace | lifecycle, duration, solver execution, IO, error, attachment / solve event |
| console / notebook 表示 | Trace renderer | text tree, live view, Chrome trace export |
| version / sharing | Artifact manifest | digest, tag, subject lineage, layer descriptors |

`run.log_parameter(...)` の一次効果は Run table の parameter cell 更新である。`run.log_solution(...)` や `run.log_instance(...)` の一次効果は Attachment の追加であり、`run.log_solve(...)` の一次効果は Solve entry の追加である。同時に OTel span event を出すことはできるが、それは「この run で何が記録されたか」を可視化する telemetry であり、データ本体ではない。

### 6.1 Span 階層

通常の Experiment / Run / builder は global `TracerProvider` を暗黙に設定しない。active provider がある場合にそれを使い、ない場合は通常の OTel no-op として扱う。

一方で `store_trace=True` と `ommx.tracing.capture_trace(...)` は trace 保存の明示要求であるため、保存用 collector を構成する目的で、未設定時に in-process SDK `TracerProvider` を設定してよい。この provider は外部 exporter や network 送信を設定しない。独自 provider を使いたい場合は capture 開始前に設定する。

`ommx.experiment` span は `with Experiment(...)` の scope に対応する。context manager を使わない手動 `commit()` workflow では、人間の思考時間や notebook cell 間の待ち時間が Experiment object の lifetime に混ざるため、Experiment scope の trace は作らない。

Span の基本構造:

| 操作 | Span 名 | 親 |
|---|---|---|
| Experiment context 開始 | `ommx.experiment` | active span があれば child、なければ root |
| Run 開始 | `ommx.run` | 通常 tracing では `ommx.experiment`。`store_trace=True` で保存する Run trace では保存単位の root |
| Adapter solve 実行 | `ommx.solver.solve` | `ommx.run` |
| Adapter sample 実行 | `ommx.solver.sample` | `ommx.run` |
| Attachment / Solve 追加 / Run parameter 更新 | span event | current run / experiment span |
| Artifact commit/build | `ommx.artifact.build` | 明示 commit / build 操作の active span。`store_trace=True` で同じ Artifact に trace を保存する場合、trace の生成と final publish は自己参照を避けるため保存対象 trace の外に置く |
| Artifact load | `ommx.artifact.load` | active span |
| Artifact push | `ommx.artifact.push` | active span |

Trace ID は OTel が発行する。OMMX は独自 Trace ID を採番しない。

### 6.2 Attachment / Solve / parameter event

各 `log_*` は Attachment / Solve 追加または Run parameter table 更新の後、可能なら current span に event を追加する。

Event 名:

- `ommx.attachment.added`
- `ommx.solve.recorded`
- `ommx.run.parameter.recorded`

Attachment event attributes:

| Attribute | 内容 |
|---|---|
| `ommx.attachment.owner` | `experiment` / `run` / `solve` |
| `ommx.run.id` | run scope の場合 |
| `ommx.solve.id` | solve scope の場合 |
| `ommx.attachment.name` | Attachment name |
| `ommx.attachment.media_type` | payload media type |
| `ommx.attachment.digest` | commit 後に分かる場合。Build 中は absent でもよい |

Run parameter event attributes:

| Attribute | 内容 |
|---|---|
| `ommx.run.id` | run id |
| `ommx.run.parameter.name` | parameter name |
| `ommx.run.parameter.scalar_type` | `int`, `float`, `string`, `bool`, `null` 等 |

Event は Attachment、Solve entry、または Run parameter cell への reference であり、payload 本体を OTel event attribute に入れない。parameter の small scalar を display 用に入れるかは renderer policy として扱い、正本にはしない。

### 6.3 Trace storage mode

Trace storage の具体的な API 仕様は実装済みであり、API Reference / tracing user guide を正本にする。本書には、今後の span/event schema 設計に影響する境界だけを残す。

- Trace storage は OTel を有効化する機能ではなく、明示的に保存を要求した Run scope の trace を Artifact layer として保存する機能である。default は保存しない。
- 保存境界は Run context manager である。Experiment object の lifetime、手動 `commit()` workflow、notebook cell 間の思考時間を保存対象 trace に混ぜない。
- Trace は Run に紐づく概念であり、Artifact aggregate accessor ではなく sealed Run view から読む。
- `trace="auto"` / `trace="required"` / `with_trace()` のような別 API は導入しない。
- trace を保存しない場合でも、active provider があれば通常の OTel span / event は外部 exporter に流れる。

### 6.4 Run trace

Run trace は Artifact / Experiment data model 上では、Run config が `LayerRef` で参照する optional payload である。1 Run に対する trace は 0 または 1 個とする。Fork は Run を logical entity としてコピーするため、Run parameter、Attachment、Solve と同じく、その Run に紐づく trace ref も子 Experiment に引き継ぐ。子で追加された Run には、その Run の trace が追加される。

Rust SDK は Trace payload の中身を知らない。`Trace` は media type 付きの opaque bytes として扱い、OTLP の decode / encode / validation は行わない。Python SDK が OTel SDK から export 済み payload を作り、読み出し時に `TraceResult` として解釈する。

Trace は Attachment / Run parameter table / Solve entry の代替ではない。parameter / solution / sample set などの本体は Experiment state の物理化戦略に従って保存し、trace は実行時系列と logical entry reference を保存する。

### 6.5 Renderer

`MintoLogger` 相当の独立 logger class は作らない。console output は OTel span / event の renderer として実装する。

実装済みの基本 renderer は API Reference / tracing user guide を正本にする。設計上の境界は、`TraceResult` を完了後の trace data / serialization 型に限定し、表示や保存は renderer 関数が `TraceResult` を受け取る形にすることである。`TraceResult` は live `ReadableSpan` を持たず、renderer は export 済み OTLP protobuf span を読む。

Phase 2:

- scoped streaming renderer を追加する。
- `Experiment(..., live=True)` 相当で opt-in。
- 対象 trace_id だけを購読する scoped processor を、呼び出し側が設定した SDK `TracerProvider` に attach する。
- span end / event を逐次 render する。

Phase 2 は span / event schema を変更せず、同じ OTel signal を読む renderer を増やす形にする。

## 7. Artifact への写像

### 7.1 Manifest identity と Experiment profile

`1 Experiment = 1 Artifact` とするが、Experiment 専用の top-level Artifact media type は初期設計では作らない。既存 Artifact 仕様と同じく、OCI manifest descriptor / transport 上の media type は `application/vnd.oci.image.manifest.v1+json`、manifest の `artifactType` は `application/org.ommx.v1.artifact` のままとする。

Experiment であることは、OMMX Artifact の profile / kind として表す。

| 場所 | 値 |
|---|---|
| OCI manifest descriptor media type | `application/vnd.oci.image.manifest.v1+json` |
| OCI manifest `artifactType` | `application/org.ommx.v1.artifact` |
| OCI manifest `config.mediaType` | `application/org.ommx.v1.experiment.config+json` |
| Experiment config JSON | `status=finished|draft|failed|interrupted`, Run / Solve structure, Attachment `LayerRef` |
`Artifact.load()` は従来通り OMMX Artifact として読み、`Experiment.load()` は OCI config descriptor の media type で Experiment profile を確認した上で、config blob の Experiment config JSON から immutable Experiment view を復元する。`config.mediaType` が `application/vnd.oci.empty.v1+json` なら v1 互換の通常 Artifact、`application/org.ommx.v1.experiment.config+json` なら Experiment と判定する。Layer annotations は inspector / compatibility 用の補助情報であり、loader が全 layer を scan して意味を推測する設計にはしない。これにより、Experiment は OMMX Artifact family の一種として扱え、既存の Local Registry / archive / remote transport / generic Artifact inspector と互換にできる。

成功 commit、Run close checkpoint、例外終了時 checkpoint、`Experiment.load(...)`、`Experiment.restore_from_checkpoint(...)` の user-facing semantics は実装済みであり、API Reference を正本にする。wire model としては、成功 commit は `status=finished` の Experiment Artifact、checkpoint は `status=draft|failed|interrupted` の Experiment config と checkpoint marker annotation を持つ reserved local ref として扱う。checkpoint Artifact handle は user-facing API には出さない。

`OMMX Artifact v3` という media type は導入しない。v3 は SDK / 設計フェーズの名前であり、wire format の互換性境界とは分ける。将来、registry の referrers API などで Experiment だけを `artifactType` で filter したい要求が強くなった場合は、`application/org.ommx.v1.experiment` を追加で許容する余地を残す。ただし初期設計では、top-level は `application/org.ommx.v1.artifact` に統一する。

### 7.2 完全な descriptor list としての manifest

各 committed Artifact manifest は、blob bytes の保存タイミングを表すものではない。`layers[]` には、その時点の Experiment view を復元するために必要な typed payload と aggregate JSON などの descriptor を載せる。

Run / Solve は manifest の子 manifest ではなく、OCI config blob に保存した Experiment config の `runs[]` / `solves[]` から復元する。Manifest `layers[]` は payload descriptor table であり、Experiment config は `LayerRef` でその table を参照する。構造復元の source of truth は config であり、payload descriptor の source of truth は manifest `layers[]` である。

初期設計では、少なくとも以下の aggregate payload を通常の Artifact layer として載せる。

| Layer | 目的 | 備考 |
|---|---|---|
| Run parameter table JSON | run ごとの scalar parameter table | 1 cell = 1 layer にはしない |

これらは manifest annotation で表現しない。Experiment profile / schema は OCI config descriptor の media type で表し、status は config JSON で表す。Run parameter table の本体は layer payload として保存する。

Experiment / Run attachments、Solve input / output、diagnostics は個別 layer または aggregate layer として保存し、Experiment config はそれらを `LayerRef` として参照する。`Experiment.load(...)` は layer annotations を scan せず、Experiment config JSON に保存された `LayerRef` と manifest `layers[]` を合わせて読む。Annotation は generic Artifact inspector や migration compatibility のための redundant metadata として扱う。

Instance / Solution / diagnostics などの payload blob は `log_*` 時点で Local Registry の BlobStore に逐次保存される。commit が行うのは、それらの blob と、Run parameter table JSON など commit 時に materialize する payload を含む最終 Experiment state を seal し、復元に必要な descriptor list を immutable manifest として IndexStore に publish することである。

既存 blob は同じ digest の descriptor として再利用できる。Local Registry では CAS として共有され、remote registry では dedup / mount され得る。一方、archive export では、その Artifact 単体で読めるよう参照 blob を含める。

したがって、複数の Run / Solve が同一 bytes の Instance を参照する場合、logical entity は複数存在してよいが、BlobStore 上の実体は同じ digest の 1 blob に共有される。重複排除の前提は serialized bytes が一致することであり、論理的に同じ Instance でも serialization に timestamp や非決定的 ordering が混ざる場合は別 digest になる。

### 7.3 Layer annotations

Attachment layer は Artifact layer descriptor annotations に以下を持てる。ただしこれらは source of truth ではなく、generic inspector / compatibility / debug 用の補助情報である。Experiment / Run / Solve の所属関係は Experiment config の構造と `LayerRef` で表す。

| Annotation | 必須 | 内容 |
|---|---|---|
| `org.ommx.attachment.name` | optional | user-facing attachment name |
| `org.ommx.codec` | optional | external codec identifier, 必要な場合 |

Attachment の media type は descriptor の `mediaType` field にあるため、annotation として重複保持しない。`run_id` / `solve_id` / `solve role` は config の位置と field 名で分かるため、annotation として重複保持しない。

Trace、Run parameter table layer、Instance / Solution layer など、layer payload の種別は descriptor の `mediaType` で判定する。`org.ommx.experiment.layer=trace` / `run-parameters` のような layer kind annotation は持たせない。

Run parameter table は、必ずしも 1 cell = 1 layer descriptor にならない。run parameter の key-level metadata は aggregate payload の内部 schema に持たせてよい。Experiment metadata を Attachment として復元するときは media type + name に写像する。

user-defined media type の Attachment は caller / external package が指定した `mediaType` をそのまま使う。OMMX core は unknown media type を拒否せず、必要なら `org.ommx.attachment.name` / `org.ommx.codec` を保持して opaque bytes として扱う。

Experiment の識別子は Artifact の Image Name と同一にする。Experiment name を別メタデータとして持たない。created time、OMMX version などの experiment-level metadata は Experiment config または JSON Attachment に保存する。巨大な metadata は config に載せず Attachment にする。

MINTO 由来の `org.minto.*` annotation は新規書き込みでは使わない。既存 MINTO artifact の import compatibility が必要なら、compat loader が `org.minto.*` を読んで `org.ommx.*` Attachment / Solve model に変換する。

### 7.4 Artifact からの復元

`Experiment.load(...)` は Artifact を読み、root manifest の config descriptor が指す Experiment config JSON から immutable Experiment view を復元する。`layers[]` 全体を scan して Run / Solve / Attachment の意味を推測しない。

復元に必要な invariants:

- config media type は `application/org.ommx.v1.experiment.config+json` とする。
- config は Experiment attachments、Run list、Run ごとの attachments / solves、Run parameter table layer への `LayerRef` を持つ。各 `LayerRef` は、この config blob を所有する OCI Image Manifest の `layers[]` index として解決する。
- run id は 0-based integer とする。ただし復元対象の Run 集合は config の `runs[]` が source of truth であり、欠番を layer scan から補完しない。
- solve id は Run 内の 0-based integer とする。ただし復元対象の Solve 集合は config の `runs[].solves[]` が source of truth であり、欠番を layer scan から補完しない。
- `LayerRef` は manifest `layers[]` の範囲内でなければならない。
- `Solve.input` が参照する layer は OMMX Instance media type、`Solve.output` が参照する layer は OMMX Solution media type を持たなければならない。
- 同一 Experiment / Run 内で同一 `LayerRef`、同一 digest、同一 media type の Attachment が複数箇所から参照されても、committed config では重複を禁止しない。loader は config に書かれた順序と重複を保持する。
- Run parameter table で同一 `(run_id, parameter_name)` が複数値を持つ場合も、committed manifest では重複を禁止し、loader は error にする。
- Run parameter table が config に存在しない run id を参照する場合、loader は error にする。

## 8. Lineage

Artifact lineage は OCI v1.1 `subject` で表す。初期設計では各 Artifact が 0/1 個の parent を持つ single-parent history のみを扱う。複数 child は自然に発生してよい。

| API | 方針 |
|---|---|
| `parent()` | `subject` を読む。0/1 件 |
| `history()` | `subject` chain を root 方向に辿る |
| `diff(other)` | Attachment / Solve list、Run parameter table、layer descriptor を比較する |

`subject` は provenance / lineage 用リンクであり、Artifact 復元に必須の dependency ではない。各 manifest は復元に必要な descriptor list を持つので、単一 Artifact archive は parent chain なしで読める。

保存済み Experiment に run を追加する場合は、loaded `Experiment` をその場で mutable にせず、`with exp.fork(tag=...) as forked:` のように parent Artifact から派生した新しい session / builder を作る。re-enter はこの派生 session を開く操作であり、元 Artifact を変更する操作ではない。正常終了時の自動 commit では新しい manifest を作り、`subject` に parent manifest descriptor を記録する。既存 run の descriptor / blob は再利用し、新しい run で追加された Instance / Solution / diagnostics / aggregate JSON だけが BlobStore に追加される。

同じ仕組みで、派生した Experiment version から run を削除する操作も表現できる。削除は既存 blob を消す操作ではなく、新しい manifest からその run に対応する attachments / solves、Run parameter table row、index entry を省く操作である。元 Artifact は immutable な parent として残り、削除された run の blob は parent からは引き続き参照され得る。物理的な blob 削除は GC の到達可能性解析と retention policy に委ねる。

複数 experiment の統合は lineage merge としては扱わない。必要なら新規 Artifact の Attachment として入力 Artifact digest を列挙する。これは parent ではなく data reference である。

Referrers API を使った child listing は初期必須 API にしない。remote registry compatibility に依存するため、manifest と `subject` だけで完結する parent 方向の走査を先に安定させる。

## 9. Garbage Collection

`ommx artifact gc` 相当の command と、到達可能性解析に必要な API hook を提供する。

GC roots:

- Local Registry refs
- checkpoint manifest refs
- user-specified protected digests
- publish 中の in-flight manifest / ref update
- protected root から辿れる `subject` chain

Local Registry GC:

- IndexStore の manifest / blob records から到達可能性を解析する。
- BlobStore に存在するが IndexStore から参照されない blob は orphan blob として扱う。
- `log_*` 時点で BlobStore に書かれたが、成功 commit または checkpoint manifest に到達しなかった orphan blob を削除候補にする。
- checkpoint manifest は retention policy が許す間 GC root として扱い、期限後に削除候補にする。
- 派生 Experiment version で run が削除されても、parent Artifact が root または protected subject chain から到達可能なら、その run の blob は保持される。storage reclaim は parent lineage の retention / pruning policy と組み合わせて行う。
- publish 途中の blob を誤削除しないよう grace period を置く。
- IndexStore record があるが BlobStore に bytes がない場合は corruption として report する。

Archive / exported OCI directory:

- manifest / index / explicit root digest から到達可能 blob を辿る。
- 未到達 blob を削除候補にする。

Remote registry:

- registry 実装ごとに deletion / retention policy が異なるため、初期設計では到達可能性解析と削除候補の列挙を優先する。
- 実削除は registry capability を検出できる場合だけ行う。

GC は data model を変えない。完全な descriptor list を持つ manifest、digest primary、single-parent lineage、Attachment / Solve model と Run parameter table とは独立した maintenance operation とする。

## 10. 残り設計事項

### 10.1 Checkpoint discovery / retention

Run close checkpoint、例外終了時 checkpoint、checkpoint からの `Experiment.restore_from_checkpoint(original_image_name)` は実装済みであり、API Reference を正本にする。

process crash など Python context manager が走らない場合でも、最後に close された Run までの checkpoint は復元できる。active Run の途中で process kill された場合に、どの未 close Run local state がどの blob に対応していたかを復元する journal metadata は未設計である。

残作業:

- checkpoint の discovery command / inspector API。
- active Run 途中状態を復元する journal metadata format。
- orphan blob と checkpoint manifest の retention policy。

### 10.2 Lineage / run deletion

保存済み Experiment を mutable に戻さず、child Artifact として派生 version を作る方針を前提に、lineage の読み出し、比較、削除 projection、retention policy を決める。

残作業:

- parent lineage の読み出し API。
- history / diff API。
- run deletion を「child manifest から run を省く操作」として扱う API。
- parent lineage と GC retention の関係。

### 10.3 Adapter execution / diagnostics

Sampler-oriented execution、diagnostics sink protocol、adapter diagnostics Attachment は未設計である。

残作業:

- `log_sample` の SampleSet-oriented Solve / Sample entry。
- backend solver 名、solver status、elapsed time など Solve scoped metadata の schema。
- `DiagnosticsSink` / `DiagnosticCollector` / `DiagnosticEntry` の型と media type validation。
- Adapter が diagnostics protocol に対応しているかの検出方法。
- solver native log、termination report、gap timeline などをどの media type と Attachment 名で保存するか。
- OTel event には diagnostics payload 本体ではなく Attachment / Solve reference と summary だけを持たせる実装。

### 10.4 Run attributes / environment

Run status は Experiment config の Run entry に保存する。elapsed time、実行環境 OS / package versions / backend solver version などを Artifact data model に持つかどうかは未設計である。

残作業:

- 実行環境情報を Run の属性として保存する場合の schema。
- 環境情報を Attachment として持たせるか、config / aggregate payload として持たせるか。
- Python / Rust で取得できる情報の差分。

### 10.5 OTel trace / renderer

Experiment / Run / Artifact operation の詳細な trace schema は未設計である。Run trace 保存、Run からの trace 読み出し、基本的な text tree / Chrome trace export renderer は実装済み API Reference / tracing user guide を正本にする。

残作業:

- Experiment / Run / solver / artifact build / load / push span schema。
- `ommx.attachment.added` / `ommx.solve.recorded` / `ommx.run.parameter.recorded` events。
- default の Experiment / Artifact build path が global `TracerProvider` を暗黙に install しないこと、および `store_trace=True` / `capture_trace(...)` では保存用 collector のための provider 初期化に限定されることの tests。
- live / streaming renderer を追加する場合の scoped collector と表示更新 policy。

### 10.6 GC

Local Registry GC の reachability model、dry-run report、削除 policy は未設計である。

残作業:

- Local Registry refs、checkpoint refs、protected digests、subject chain からの到達可能性解析。
- orphan blob / unreferenced manifest / stale checkpoint refs の dry-run report。
- retention / grace period。
- archive / OCI directory の到達可能性解析。
- remote registry は deletion capability を検出できる範囲で dry-run を優先する。

### 10.7 Legacy MINTO import

`org.minto.*` annotation を読む compatibility loader は未実装である。必要なら、OMMX core に持つか migration tool として外に出すかを決める。

## 11. 未実装項目

| 項目 | 残作業 |
|---|---|
| Checkpoint discovery / retention | discovery command、inspector、retention policy |
| Active Run journal | active Run 途中状態を復元する metadata format |
| Lineage / run deletion | `parent()`、`history()`、`diff(other)`、run deletion、lineage retention / GC |
| Adapter sample execution | `log_sample` の SampleSet-oriented Solve / Sample entry |
| Solve scoped metadata | backend solver 名、solver status、elapsed time などの schema |
| Diagnostics sink protocol | `DiagnosticsSink` / `DiagnosticCollector` / `DiagnosticEntry`、media type、truncation / compression policy |
| Run attributes / environment | elapsed time、OS / package / solver version の schema |
| OTel span / event integration | Experiment / Run / solver / artifact operation spans、attachment / solve / parameter events、live renderer policy |
| GC | Local Registry dry-run / report / delete、orphan blob retention、archive / remote registry handling |
| Legacy MINTO import | compatibility loader を core に持つか migration tool にするか |
| Documentation integration | 既存の `docs/ja/tutorial/experiment_management.md` は保持しつつ、必要に応じて Sphinx guide、Rust API docs、wire format docs へ分割し、本ファイルを削除する |
