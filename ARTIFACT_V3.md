# OMMX Artifact v3 Design

OMMX Artifact v3 の設計決定をまとめる内部文書。議論ログではなく、実装に入るための合意済み方針を記録する。実装完了後は本ファイルを削除し、内容を通常の Sphinx documentation / API reference に統合する。

`ocipkg` 置換、minto 由来機能の取り込み範囲、OTel 統合、trace layer、lineage、tag / digest、Garbage Collection の扱いは本書の方針で固定する。

## 1. 最終方針

v3 における Artifact の最終形は以下とする。

1. `ocipkg` 依存を撤去し、OCI 標準型と Distribution 処理は既存の OCI 関連 crate (`oci-spec`, `oci-distribution` / `oci-client` 等) を優先して使う。OMMX 固有部分だけを最小限自前実装にする。
2. 実験管理機構 (`Experiment`, `Run`, `DataStore`, `EnvironmentInfo`, table/export) は OMMX が直接提供する。ただし `jijmodeling` 依存は OMMX core に入れない。
3. DataStore / Artifact は記録データの source of truth、OTel は実行テレメトリの source of truth とする。
4. `MintoLogger` 相当の出力は OTel span / event の renderer として実装する。独立した logger class は作らない。
5. Artifact は build 時の trace body を self-contained layer として持てる。Phase 1 は OTLP JSON trace layer、Phase 2 以降で Logs / Metrics や scoped streaming renderer を拡張する。
6. OMMX は global `TracerProvider` を暗黙に設定しない。trace capture は `trace="auto"` を既定とし、明示要求の `trace="required"` / `with_trace()` だけを fail fast にする。
7. Artifact lineage は OCI v1.1 `subject` で表す。v3 初期は単一 parent の linear history に限定し、merge commit 相当は後続拡張に回す。
8. 各 manifest は full snapshot とする。`subject` は provenance / lineage 用リンクであり、artifact 復元に必須の dependency ではない。
9. Artifact の primary identifier は digest とする。tag は registry transport 上の mutable alias に限定する。
10. `history()`, `parent()`, `diff(other)` 相当の lineage 走査 API は提供する。子一覧取得は Referrers API 依存が強いため初期必須 API にしない。
11. `git gc` 相当の `ommx artifact gc` command を提供する。到達可能性解析に必要な hook も Artifact API 側に用意する。

## 2. 背景

OMMX Artifact は OCI Image / Artifact 仕様に乗せて最適化問題・解・実験メタデータを配布する仕組みである。現在は外部 crate `ocipkg` に依存している。一方、実験トラッキング層である `minto` は `ommx>=2.0.0` の Artifact API に依存しながら、Experiment / Run / DataStore 階層、provenance 収集、DataFrame 集計、階層的 console output といった「Artifact に居場所があるべき機能」を Python 側で抱えている。

Artifact v3 では、この構造を整理する。

- Artifact の OCI 実装を OMMX 側の所有に戻す。
- minto の汎用実験管理機構を OMMX に吸収する。
- domain-specific な problem generator や `jijmodeling` storage は OMMX core から除外する。
- 実験の可視化と実行履歴は OTel の span / event / resource に正規化する。

## 3. 現状

### 3.1 Rust Artifact 実装

現在の Rust 実装は `rust/ommx/src/artifact.rs` と `rust/ommx/src/artifact/` にある。

| モジュール | 役割 |
|---|---|
| `artifact.rs` | registry 管理、image load / pull、layer 取得、Solution / Instance / SampleSet 抽出 |
| `builder.rs` | archive / dir backend に対する Builder trait |
| `annotations.rs` | Instance / ParametricInstance / Solution / SampleSet 用 metadata annotation |
| `media_types.rs` | OMMX 固有 media type 定義 |
| `config.rs` | config 構造体 |

`rust/ommx/Cargo.toml` は `ocipkg` に依存している。

```toml
ocipkg = { version = "0.4.0", default-features = false }

[features]
default = ["remote-artifact"]
remote-artifact = ["ocipkg/remote", "built"]
```

### 3.2 `ocipkg` 利用面

| `ocipkg` API | 用途 |
|---|---|
| `ImageName` | image reference の parse、local save path 算出 |
| `Image` trait | archive / dir / remote backend の抽象化 |
| `OciArtifact<Base>` | manifest / config / layer / blob 読み出し |
| `OciArchive(Builder)` | tar.gz 形式の `.ommx` file |
| `OciDir(Builder)` | local registry directory |
| `Remote(Builder)` | OCI Distribution API client |
| `ImageManifest` / `Descriptor` / `Digest` / `MediaType` | OCI 標準型 |
| `image::copy()` | backend 間の artifact copy |

実装上の依存は artifact 周辺に寄っているが、public surface には漏れている。

- Rust SDK は `ommx::ocipkg` を re-export している。
- Rust artifact API は `Descriptor` / `Digest` / `MediaType` を public signature に含む。
- Python `Descriptor` は `oci-spec` の JSON shape を public API として見せている。

したがって `ocipkg` 削除は内部差し替えだけでは終わらない。v3 では OMMX-owned wrapper を用意するか、`oci-spec` 由来型を直接 public API として採用するかを実装前に決め、migration note を用意する。

### 3.3 Python Artifact 実装

`python/ommx/src/artifact.rs` は PyO3 wrapper として `PyArtifact` / `PyArtifactBuilder` を提供している。

現在の主要 API:

- `Artifact.load_archive(path)`
- `Artifact.load(image_name)`
- `Artifact.push()`
- `ArtifactBuilder.new_archive_unnamed(path)`
- `ArtifactBuilder.new_archive(path, name)`
- `ArtifactBuilder.new(name)`
- `ArtifactBuilder.temp()`
- `ArtifactBuilder.for_github(org, repo, name, tag)`
- `add_*` / `get_*`: instance, solution, parametric_instance, sample_set, ndarray, dataframe, json, generic layer

v3 では Rust / Python ともに破壊的変更を許容する。既存 API を維持するより、Builder と read-only View の分離、digest primary の参照、DataStore / Experiment の一貫性を優先する。

### 3.4 テスト状況

Artifact 自体の test coverage は薄い。

- Rust: `examples/create_artifact.rs`, `examples/pull_artifact.rs` の ad-hoc coverage が中心。
- Python: `python/ommx-tests/tests/test_descriptor.py` が中心。

v3 実装では Rust integration test と Python round-trip test を最初に整備する。

## 4. スコープ

### 4.1 対象

- `Cargo.toml` から `ocipkg` を削除し、OCI handling を既存 crate と OMMX-owned glue に置き換える。
- `ocipkg` / OCI public type の migration 方針を明示する。
- OMMX core に `Experiment`, `Run`, `DataStore`, `EnvironmentInfo`, table/export を設計し直して取り込む。
- Artifact と OTel の双方向接続を実装する。
- build trace の self-contained trace layer を Artifact に埋め込む。
- OCI v1.1 `subject` による single-parent linear lineage を実装する。
- `history()`, `parent()`, `diff(other)` 相当の lineage 走査 API を提供する。
- `ommx artifact gc` 相当の Garbage Collection command と、到達可能性解析に必要な API hook を提供する。
- Artifact の Rust / Python tests を追加する。

### 4.2 対象外

- `ommx.v1` の Instance / Solution / SampleSet schema 変更。
- 既存 `.ommx` file の OCI Image Layout compatibility を壊す変更。
- `minto.problems.*` の problem generator 取り込み。
- `jijmodeling` への OMMX core dependency。
- `minto.datastore.ProblemStorage` の OMMX core 取り込み。
- v3 初期での multi-parent merge lineage。
- v3 初期での Referrers API based child listing。
- v3 初期での OTel Logs / Metrics の Artifact 埋め込み。

### 4.3 互換性スタンス

`minto` API compatibility は維持しない。`minto.Experiment` / `minto.Run` の class hierarchy や method signature をそのまま OMMX に持ち込む必要はない。

維持するのは user experience である。つまり、ユーザが実験を作り、run を回し、parameters / solutions / artifacts / environment を記録し、Artifact として配布し、再ロードして解析できることを保証する。

OMMX の公式 documentation / API reference では minto に言及しない。minto は設計上の参考元であり、公開 API の前提ではない。

## 5. OCI 実装方針

### 5.1 既存実装優先

OCI 実装は既存 crate を優先する。

1. まず `oci-spec`, `oci-distribution` / `oci-client` 等で必要機能を実装できるか評価する。
2. 足りない部分だけ OMMX-owned glue として実装する。
3. archive / dir / remote の共通抽象は OMMX の Artifact use case に合わせて最小化する。
4. remote push / pull も同じ方針で、Distribution client crate を試したうえで必要なら自前実装する。

`ocipkg` は現在すでに `oci-spec` を利用しているため、「OCI 標準型が自前実装だから置き換える」という整理ではない。置き換え対象は Distribution client、layout / copy abstraction、package distribution 用 utility、そして OMMX の public API に漏れた `ocipkg` 型である。

### 5.2 公開 API surface

v3 では `ommx::ocipkg` re-export を削除する。`Descriptor` / `Digest` / `MediaType` は次のいずれかに整理する。

- OMMX-owned wrapper として公開する。
- `oci-spec` 型を直接 public API として採用する。

どちらを選んでも、Python の `Descriptor` JSON shape と Rust の public signature は migration note の対象にする。

### 5.3 Registry compatibility

OCI v1.1 `subject` と Referrers API は全 registry で同じように動くとは限らない。

v3 初期では fallback 仕様を先に固定しない。archive / dir backend は完全に制御できるため `subject` をそのまま扱う。remote registry が `subject` push を拒否した場合は、annotation fallback で曖昧に継続せず、明示 error とする。実際の非対応 registry に遭遇した時点で fallback を設計する。

## 6. DataStore / Experiment model

### 6.1 DataStore の構造

minto `DataStore` は名前付きの型別 dict を束ねた構造である。v3 では OMMX core の DataStore を次のように整理する。

| カテゴリ | 種別 | OCI Artifact mapping |
|---|---|---|
| per-entry storage | instances, solutions, samplesets, objects, generic media entries | 名前ごとに 1 layer |
| normalized scalar storage | parameters, metadata | key/value ごとに 1 layer |
| environment storage | EnvironmentInfo | first-class artifact entry |

現 minto の aggregate dict (`parameters`, `meta_data`) は OCI の content-addressable model と相性が悪い。key 追加のたびに dict 全体を再 encode する必要があり、append-only ではなくなるためである。

v3 では aggregate dict を per-entry に正規化する。例えば `parameters["alpha"] = 0.1` は `("alpha", 0.1)` の独立 entry になる。

### 6.2 Pluggable storage

OMMX core は generic media type storage を持つ。外部 package は media type と codec を登録することで domain-specific data を保存できる。

ただし OMMX core は `jijmodeling` を import しない。`jijmodeling` problem storage が必要なら、OMMX core ではなく optional adapter / external package が提供する。

### 6.3 EnvironmentInfo

`EnvironmentInfo` は Artifact / DataStore の first-class entry として永続化する。OTel `Resource` はその投影であり、情報本体ではない。

OTel `Resource` へ写す属性は standard semantic conventions を優先する。OS / host / process / runtime / container などは `os.*`, `host.*`, `process.*`, `process.runtime.*`, `container.*` 等に寄せる。標準で表現できない OMMX 固有情報だけを `ommx.*` namespace に置く。

同じ意味の値を標準属性と `ommx.*` に二重記録しない。

### 6.4 Build / Seal / View

Artifact の mutation semantics は 3 相に分ける。

| 相 | 性質 | API |
|---|---|---|
| Build | in-memory mutable | `ArtifactBuilder`, `Experiment`, `Run` |
| Seal | snapshot を作る | `build()` / `commit()` |
| View | immutable read-only | `Artifact` |

Build 相では同名 key の upsert を許容してよい。Seal 相で最終 DataStore view を snapshot として manifest に固定する。View 相には `add` / `update` を生やさない。

永続層に update primitive は存在しない。永続化済み Artifact を変える唯一の方法は、新しい full-snapshot Artifact を作ることである。

### 6.5 Commit granularity

`1 manifest = 1 commit` とする。

- Core primitive は明示 `build()` / `commit()`。
- High-level `Experiment` は experiment 終了時に自動 commit する。
- `Run` 終了ごとに manifest を切る挙動は `commit_per_run=True` 相当の opt-in にする。
- Default では run ごとに commit しない。

## 7. OTel / Trace model

### 7.1 Source of truth

DataStore / Artifact と OTel の責務を分ける。

| 領域 | Source of truth |
|---|---|
| parameter / solution / sample set / object / environment | DataStore / Artifact |
| lifecycle / duration / IO / error / record reference | OTel trace |
| console rendering | OTel renderer |

`run.log_parameter(...)` や `run.log_solution(...)` は logger 呼び出しではない。一次効果は DataStore への記録であり、OTel span event は「この run で何が記録されたか」を可視化する副次的 telemetry である。

### 7.2 MintoLogger の解体

`MintoLogger` 相当の単一 class は作らない。minto の logger が混ぜていた信号を分解する。

| 元の機能 | v3 の所属 |
|---|---|
| Experiment / Run / Solver の開始終了 | OTel span |
| parameter / solution / sample set / object の記録 | DataStore entry + span event |
| warning / error / debug | OTel Logs または Rust `tracing::{warn,error,debug}!` |
| EnvironmentInfo 表示 | EnvironmentInfo entry + Resource projection + renderer |
| indent 付き console output | post-hoc / streaming renderer |

Phase 1 では OTel Logs を Artifact に埋め込まない。warning / error は span event と span status に寄せる。OTel Logs / Metrics の Artifact 埋め込みは Phase 2 以降で扱う。

### 7.3 Span hierarchy

Trace ID は OMMX が独自採番しない。OTel の root span 作成時に発行される。

Span 開始ルール:

- `Experiment` 開始時:
  - active span がなければ `ommx.experiment` root span を開始し、新しい `trace_id` が発行される。
  - active span があれば `ommx.experiment` はその child span となり、既存 `trace_id` を継承する。
- `Run` 開始時:
  - `ommx.run` は `ommx.experiment` の child span。
  - 新しい `trace_id` は発行せず、`span_id` だけ新しくなる。
- `ArtifactBuilder.build()`:
  - `ommx.artifact.build` span を開始する。
  - active span があれば child span となり、既存 `trace_id` を継承する。
  - active span がなければ root span となり、新しい `trace_id` が発行される。
- `Artifact.load*` / `push`:
  - artifact を使う側の trace として現在の active span に接続する。
  - build-time trace と同じ trace に無理に接続しない。
  - artifact 内の build-time trace は load / push span から OTel Link として参照する。
- 派生 Artifact:
  - lineage は OCI `subject` で表す。
  - trace は新しい `ommx.artifact.build` span を持つ。
  - parent artifact の build trace は、存在すれば OTel Link として張る。

Manifest annotation key:

- `org.ommx.trace.build.trace_id`
- `org.ommx.trace.build.span_id`

### 7.4 Trace layer

Artifact は build 時の trace body を dedicated layer として埋め込める。これは Cloud Run / batch job のように Artifact 入出力だけで完結する実行環境で重要である。

Phase 1 の trace layer:

| 項目 | 方針 |
|---|---|
| encoding | OTLP JSON |
| media type | `application/vnd.ommx.trace.otlp+json` |
| payload | OTLP JSON mapping の `ExportTraceServiceRequest` 互換 (`resourceSpans`) |
| 対象 signal | span / span event |
| derived format | Chrome Trace Event Format は読み出し時に生成 |
| API | `artifact.get_trace() -> TraceResult` |

Trace layer は DataStore の代替ではない。parameter / solution / sample set / environment の本体は通常の Artifact entry に保存し、trace layer は実行時系列と record reference を保存する。

### 7.5 Trace capture mode

OMMX は global `TracerProvider` を暗黙に設定しない。import、`Experiment` 開始、`ArtifactBuilder.build()` のいずれでも `trace.set_tracer_provider()` を勝手に呼ばない。

Trace capture mode:

| mode | 動作 |
|---|---|
| `trace="auto"` | default。provider / collector が設定済みなら trace layer を埋め込む。未設定なら trace layer を省略し、status annotation を残す |
| `trace="required"` | 明示要求。provider / collector が未設定なら setup error |
| `with_trace()` | 低レベル builder の明示要求。provider / collector が未設定なら setup error |
| `trace=False` | 常に trace layer を生成しない |

`trace="auto"` で trace layer を省略した場合の manifest annotations:

- `org.ommx.trace.status=not_recorded`
- `org.ommx.trace.reason=no_tracer_provider`

この設計により、通常の Experiment 利用は OTel setup を必須にせず、trace を成果物として要求する利用では欠落を fail fast で検知できる。

### 7.6 Renderer

Phase 1 は post-hoc renderer のみを提供する。

- `Experiment` / `Run` / `ArtifactBuilder.build()` が span / event を発行する。
- `capture_trace()` または Experiment 内部 collector が span を収集する。
- Experiment 終了後または Artifact load 後に `TraceResult` を作る。
- `trace_result.text_tree(style="experiment")` 相当で描画する。

Phase 2 で scoped streaming renderer を追加する。

- `Experiment(..., live=True)` 相当で opt-in。
- 対象 `trace_id` だけを購読する scoped `SpanProcessor` を、呼び出し側が設定した SDK `TracerProvider` に attach する。
- span end / event を逐次 render する。
- Experiment 終了時に processor を deactivate / detach する。

Phase 2 は span / event schema を変更せず、同じ OTel signal を読む renderer を増やす形で実装する。

## 8. Lineage model

### 8.1 Full snapshot manifest

各 manifest は full snapshot とする。派生 Artifact の `layers[]` には、その時点の DataStore view を復元するために必要なすべての descriptor を載せる。

既存 blob は同じ digest の descriptor として再利用できる。remote registry では dedup / mount され得るが、archive / dir 形式ではその Artifact 単体で読めるよう参照 blob を含める。

`subject` は lineage / provenance のためのリンクであり、子 Artifact を読むための必須 dependency ではない。

```jsonc
{
  "schemaVersion": 2,
  "artifactType": "application/org.ommx.experiment.v1",
  "config": { ... },
  "layers": [ ... full DataStore snapshot ... ],
  "subject": {
    "mediaType": "application/vnd.oci.image.manifest.v1+json",
    "digest": "sha256:...",
    "size": 1234
  }
}
```

### 8.2 Linear history

v3 初期は OCI v1.1 `subject` の単一 parent に寄せ、linear history のみを扱う。

| 概念 | Git | OCI v1.1 | OMMX |
|---|---|---|---|
| content address | blob | descriptor -> blob | Instance / Solution / SampleSet 等 |
| snapshot | tree | manifest (`layers[]`) | 1 つの experiment state |
| history node | commit with parent | manifest + `subject` | 派生 Artifact |
| mutable ref | tag / branch | tag | digest primary の alias |
| history traversal | `git log` | `subject` chain | `history()` |

複数 experiment の統合は lineage merge としては扱わない。必要なら新規 Artifact の DataStore entry として入力 Artifact digest を列挙する。これは parent ではなく data reference である。

多 parent が必要になった場合は、後続 version で annotation 規約または OCI 側の標準機能を再検討する。

### 8.3 Digest and tag

Artifact の primary identifier は digest とする。API / metadata / provenance で再現性が必要な場所には digest を保存する。

`experiment:latest`, `experiment:v2` のような tag は registry transport 上の mutable alias として扱う。OMMX の Experiment API では branch concept を前面に出さない。

### 8.4 Lineage API

v3 初期で提供する lineage 走査 API:

| API | 方針 |
|---|---|
| `parent()` | `subject` を読む。0/1 件 |
| `history()` | `subject` chain を root 方向に辿る |
| `diff(other)` | manifest の layer descriptor 列と DataStore entry metadata を比較する |

Referrers API を使った「この Artifact を parent に持つ子一覧」は初期必須 API にしない。remote registry compatibility に依存するため、archive / dir backend で完結する parent 方向の走査を先に安定させる。

## 9. Garbage Collection

長期運用では古い manifest や未参照 blob が registry / local layout に残る。v3 では `git gc` 相当の `ommx artifact gc` command を提供する。

GC の責務は以下とする。

- local archive / dir backend では、manifest / tag / explicit root digest から到達可能な blob を辿り、未到達 blob を削除対象にする。
- remote registry では registry 実装ごとに deletion / retention policy が異なるため、v3 初期は到達可能性解析と削除候補の列挙を優先する。実削除は registry capability を検出できる場合だけ行う。
- `subject` chain、tag alias、user-specified protected digest を GC root として扱う。
- Artifact API 側に到達可能性解析 hook を用意し、CLI command と将来の storage-specific GC が同じ解析を使えるようにする。

GC は data model を変えない。full snapshot、digest primary、single-parent lineage の方針は GC 実装と独立している。

## 10. リスク

- **OCI Distribution の実装量**: auth、manifest PUT / GET、blob upload session、cross-repo mount、chunked upload まで含めると実装量が大きい。
- **既存 `.ommx` file の後方互換**: OCI Image Layout compatibility は維持するが、annotation key や public descriptor shape の差分は migration note が必要。
- **`ocipkg` public surface の撤去**: `ommx::ocipkg` re-export、Rust / Python の `Descriptor` / `Digest` / `MediaType` 露出を置き換える必要がある。
- **minto user への影響**: API compatibility は維持しないため、取り込み時期と migration messaging が必要。
- **TracerProvider 所有権**: 現行 `ommx.tracing` の lazy setup が provider を install する挙動は v3 方針と衝突するため見直す必要がある。
- **Logger output の期待差分**: Phase 1 は post-hoc 表示なので、従来の live console output を期待する環境では明示的な説明が必要。
- **Registry の OCI v1.1 対応差**: `subject` 非対応 registry に当たる可能性がある。初期方針は explicit error とし、fallback は実ケースが出てから設計する。

## 11. 参考

### OMMX Rust

- `rust/ommx/src/artifact.rs`
- `rust/ommx/src/artifact/{builder,annotations,media_types,config}.rs`
- `rust/ommx/Cargo.toml`
- `rust/ommx/examples/{create,pull}_artifact.rs`

### OMMX Python

- `python/ommx/src/artifact.rs`
- `python/ommx/Cargo.toml`
- `python/ommx-tests/tests/test_descriptor.py`

### OMMX Tracing / OTel

- `python/ommx/ommx/tracing/{__init__,_capture,_collector,_render,_setup,_magic}.py`
- `python/ommx/Cargo.toml` の `tracing-bridge` feature と `pyo3-tracing-opentelemetry` dependency
- `python/ommx-tests/tests/{test_tracing,test_tracing_capture,test_tracing_magic}.py`
- 各 adapter の `tests/test_tracing.py`

### ocipkg

- `/Users/termoshtt/github.com/termoshtt/ocipkg/ocipkg/src/lib.rs`
- 主要 module: `image`, `media_types`, `local`, `distribution`

### minto

- `/Users/termoshtt/github.com/Jij-Inc/minto/minto/datastore.py`
- `/Users/termoshtt/github.com/Jij-Inc/minto/minto/experiment.py`
- `/Users/termoshtt/github.com/Jij-Inc/minto/minto/run.py`
- `/Users/termoshtt/github.com/Jij-Inc/minto/minto/exp_dataspace.py`
- `/Users/termoshtt/github.com/Jij-Inc/minto/minto/environment.py`
- `/Users/termoshtt/github.com/Jij-Inc/minto/minto/logger.py`
- `/Users/termoshtt/github.com/Jij-Inc/minto/minto/table.py`
