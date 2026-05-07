# OMMX Artifact v3 Proposal

OMMX Artifact レイヤの最終形を記述する Proposal。本ドキュメントは決定事項ではなく **議論のたたき台** であり、未決論点を明示する。実装順序や移行計画は含まず、**「最終的にどうあるべきか」** に絞る。

## 1. 背景と動機

OMMX Artifact は OCI Image / Artifact 仕様に乗せて最適化問題・解・実験メタデータを配布する仕組みで、現在は外部クレート [`ocipkg`](https://github.com/termoshtt/ocipkg) に薄く依存している。一方で実験トラッキング層である [`minto`](https://github.com/Jij-Inc/minto) は `ommx>=2.0.0` の Artifact API に依存しつつ、Experiment / Run / DataStore 階層・provenance 収集・DataFrame 集計といった「Artifact に居場所があるべき機能」を Python 側で抱えている。

v3 における Artifact の最終形は以下:

1. **`ocipkg` 依存を撤去** し、OCI 取り扱いは整備された外部 OCI 関連クレート (`oci-spec`, `oci-distribution` / `oci-client` 等) + 最小限の自前コードに置き換える。**自前実装は最小限、外部ライブラリは最大限活用** が v3 の方針。
2. 実験管理 (Experiment / Run / DataStore) は OMMX が直接提供する。
3. observability は `ommx.tracing` (OTel) に一元化される。`MintoLogger` 相当の階層出力は OTel span のレンダラとして実装され、独立ログ系統は持たない。**ジョブが生成した Artifact は build 時のトレース本体を内蔵し**、Cloud Run / バッチ系のような Artifact 入出力で完結する実行環境でも単体で実行履歴を再構成できる。
4. artifact 同士の派生関係は OCI v1.1 `subject` で表現され、Git の commit DAG に近い履歴セマンティクスを持つ。

### 1.1 `ocipkg` を外す理由

- ocipkg は @termoshtt の実験プロジェクトとして始まり、現在ほぼメンテナンスされていない
- 元々は OCI Artifact 仕様成立前に「静的ライブラリを OCI Image として配布する」目的で作られ、後から OCI Artifact ベースに付け替えたため設計に歪みが残っている
- ocipkg は現在すでに `oci-spec` を利用しているが、Distribution client、archive / dir / remote 間の copy abstraction、静的ライブラリ配布向けユーティリティなど、OMMX Artifact の要件とは別の自前抽象が残っている。v3 では OCI 標準型は直接 `oci-spec` 等に寄せ、OMMX 固有部分だけを自前に残す

### 1.2 実装戦略 (try-existing-first)

- まず既存クレート (`oci-spec`, `oci-distribution` / `oci-client` 等) で実装することを **基本** とし、外部ライブラリでまかなえるかを評価する
- 評価の結果、機能不足・設計上の不整合・メンテ状況等の問題が判明した部分だけを自前実装に切り替える
- 「最初から全部自前」「最初から全部外部依存」のいずれも取らず、外部依存は段階的に必要最小限へ削っていく
- Remote 機能 (push / pull) もこの戦略の特殊例: 既存の OCI Distribution クライアントクレートを試し、ダメなら自前

## 2. 現状把握

### 2.1 Rust 側 (`rust/ommx/src/artifact.rs`, `rust/ommx/src/artifact/`)

親 module + 4 サブモジュール構成:

| モジュール | 役割 |
|---|---|
| `artifact.rs` | レジストリ管理、image ロード/プル、レイヤ取得、Solution/Instance/SampleSet 抽出 |
| `builder.rs` | archive/dir バックエンドに対する Builder トレイト |
| `annotations.rs` | Instance / ParametricInstance / Solution / SampleSet 用メタデータ注釈 |
| `media_types.rs` | OMMX 固有 MIME タイプ定義 |
| `config.rs` | 設定構造体 (現状ほぼ空) |

`Cargo.toml`:

```toml
ocipkg = { version = "0.4.0", default-features = false }

[features]
default = ["remote-artifact"]
remote-artifact = ["ocipkg/remote", "built"]
```

### 2.2 ocipkg 利用面

| ocipkg API | 用途 |
|---|---|
| `ImageName` | イメージ参照のパース、ローカル保存パス算出 |
| `Image` trait | archive / dir / remote を抽象化する基盤 |
| `OciArtifact<Base>` | manifest / config / layer / blob 読み出しの汎用ラッパ |
| `OciArchive(Builder)` | tar.gz 形式の `.ommx` ファイル |
| `OciDir(Builder)` | ローカルレジストリのディレクトリ配置 |
| `Remote(Builder)` | OCI Distribution API クライアント (HTTP)、`remote-artifact` でゲート |
| `ImageManifest` / `Descriptor` / `Digest` / `MediaType` | OCI 標準型 |
| `image::copy()` | バックエンド間で artifact を転送するコア関数 |

実装上の依存は artifact 周辺にほぼ局所化されている。一方で public surface には漏れている:

- Rust SDK は `ommx::ocipkg` を re-export している
- Rust の artifact API は `Descriptor` / `Digest` / `MediaType` を `ocipkg::oci_spec` 経由で公開している
- Python の `Descriptor` は `oci-spec` の JSON shape を公開 API として見せている

v3 では `ocipkg` 型を public API から消し、OCI 標準型は `oci-spec` 直参照または OMMX wrapper に整理する。

### 2.3 Python 側 (`python/ommx/src/artifact.rs`)

- PyO3 ラッパ: `PyArtifact` / `PyArtifactBuilder` (内部に archive/dir variant の enum)
- 公開 API:
  - `Artifact.load_archive(path)` / `Artifact.load(image_name)` / `Artifact.push()`
  - `ArtifactBuilder.new_archive_unnamed(path)` / `new_archive(path, name)` / `new(name)` / `temp()` / `for_github(org, repo, name, tag)`
  - `add_*` / `get_*` 系: instance / solution / parametric_instance / sample_set / ndarray / dataframe / json / layer

### 2.4 テスト現況

- Rust: `examples/create_artifact.rs`, `examples/pull_artifact.rs` — アドホックのみ、専用統合テストなし
- Python: `python/ommx-tests/tests/test_descriptor.py` のみ
- **Artifact 自体のテストカバレッジは薄い**

### 2.5 minto 側で吸収候補

`minto` の Python 実装のうち、Artifact レイヤに上げるのが自然な部分:

| minto モジュール | 役割 | OMMX 側候補 |
|---|---|---|
| `DataStore` (`minto/datastore.py`) | 型別ストレージ戦略を pluggable に束ねる。現 minto には JSON / JijModeling Problem / OMMX Instance/Solution/SampleSet がある | `ommx.artifact.datastore` 相当。ただし OMMX core に入れるのは JSON / scalar / generic bytes / OMMX Instance/Solution/SampleSet 等に限り、JijModeling 固有 storage は除外する |
| `ExperimentDataSpace` (`minto/exp_dataspace.py`) | experiment / runs/{i}/ の階層構造、layer annotation で experiment データと run データを区別 | `ommx.artifact.experiment` 相当 |
| `table.create_table_from_stores` (`minto/table.py`) | DataStore 群から pandas DataFrame を生成 | `ommx.artifact.export` または同等 |
| `EnvironmentInfo` (`minto/environment.py`) | 実験プロベナンス (OS / CPU / Python / パッケージバージョン) | `ommx.artifact.environment` 相当 |
| `MintoLogger` (`minto/logger.py`, `minto/logging_config.py`) | experiment / run イベントの階層的コンソール出力 (インデント・アイコン) | クラスとしては吸収しない。4.5 節で 3 信号 (Span / DataStore+Event / OTel Logs) に分解、コンソール表示は streaming exporter に集約 |

**取り込まない**:
- `minto.problems.*` (TSP/Knapsack/CVRP ジェネレータ) — ドメイン固有の問題ジェネレータは OMMX のスコープ外
- `minto.datastore.ProblemStorage` — `jijmodeling` への依存は OMMX core には入れない。必要なら外部 package が generic media-type storage として登録する

### 2.6 OMMX の OTel 連携現状

OMMX v3 では Rust 側 `tracing` を Python 側 OpenTelemetry に橋渡しする方針が既に走っている。Artifact v3 はこの基盤の上に載る。

**Rust 側**:
- `tracing` クレートで span / event を発行 (例: `instance.rs` の評価系)
- `python/ommx/Cargo.toml` の feature `tracing-bridge` (default 有効) → `pyo3-tracing-opentelemetry` で Python OTel `TracerProvider` に流す

**Python 側 (`python/ommx/ommx/tracing/`)**:
- 公開 API:
  - `capture_trace()` — context manager。終了時に `TraceResult` に span ツリーを格納 (例外時も保持)
  - `@traced(output=...)` — デコレータ糖衣、Chrome Trace JSON をディスク出力可
  - `%%ommx_trace` — Jupyter cell magic、セル単位で span ツリーをテキスト + Chrome Trace 描画
  - `load_ipython_extension` / `unload_ipython_extension`
- 内部:
  - `_collector.py` — `SpanProcessor` 実装 (明示的に capture したトレースのみ収集)
  - `_render.py` — テキスト木 + Chrome Trace Event Format への変換
  - `_setup.py` — OTel pipeline の遅延初期化
  - `_magic.py` / `_capture.py`

**ハード依存**: `opentelemetry-sdk>=1.20.0`, `ipython` (`python/ommx/pyproject.toml`)

**テスト**:
- `python/ommx-tests/tests/test_tracing.py`, `test_tracing_capture.py`, `test_tracing_magic.py`
- 各 adapter (`ommx-openjij-adapter`, `-pyscipopt-adapter`, `-python-mip-adapter`, `-highs-adapter`) に `test_tracing.py` — adapter span も同経路で橋渡し

**示唆**: トレース管理の本流は既に OTel である。`MintoLogger` を **OTel と並列の独立ログ系統として持ち込むのは避け**、span / event / attribute に変換する方向で設計するのが整合的。

## 3. 目的とスコープ

### 3.1 In Scope

- `Cargo.toml` から `ocipkg` を削除し、OCI 取り扱いを既存クレート + 最小限の自前コードに置き換える (1.2 の戦略)
- minto の汎用機能 (DataStore の構造 / ExperimentDataSpace / table / EnvironmentInfo / Logger) を **再設計しつつ** OMMX に取り込む。ただし `jijmodeling` 固有の storage / API は取り込まない
- Artifact ↔ OTel の双方向統合 (4 章)
- OCI v1.1 `subject` を用いた artifact lineage 機構 (5 章)
- Artifact 層の Rust 統合テスト + Python ラウンドトリップテストを最終形が備える

### 3.2 Out of Scope

- 既存 `.ommx` ファイル形式の変更 (OCI Image Layout 互換は維持)
- `ommx.v1` の Instance/Solution/SampleSet 等のスキーマ変更
- minto のドメイン固有機能 (`problems.*` の問題ジェネレータ) の取り込み
- `jijmodeling` への OMMX core 依存、および `minto.datastore.ProblemStorage` の取り込み

### 3.3 非目的

- パフォーマンス改善は副次的目標であり、主目標ではない (ベンチマークなしの性能主張は禁止: CLAUDE.md 参照)

### 3.4 互換性スタンス

- **minto との互換性は API レベルでは維持しない**。`minto.Experiment` / `minto.Run` のクラス階層やメソッドシグネチャをそのまま OMMX に持ち込む必要はない。**ユーザ体験 (実験を作って run を回し記録、artifact として配布、再ロードして解析、というフロー)** が同等に成立すれば十分
- この自由度を活かし、minto が暗黙に妥協していた点 (DataStore の dual nature、aggregate dict の更新、Logger の信号混在) を v3 では正面から再設計する (5 章で詳述)
- **OMMX 自身の Python / Rust SDK の破壊的変更も許容する**: v2 → v3 で `Artifact.load_archive` / `ArtifactBuilder.new_archive` 等のシグネチャや構造を変更して構わない。Rust 側 v3 Stage Pattern (`rust/ommx/doc/migration_guide.md`) との整合は通常の v3 マイグレーションプロセスで吸収する
- **OMMX の公式ドキュメント・API リファレンスでは minto に一切言及しない**: OMMX は実験管理機構 (Experiment / Run / DataStore / 環境メタデータ収集) を OMMX 自身の機能として提供・解説する。minto への参照、移行ガイド、互換性ノート等を OMMX 側の公開ドキュメントには載せない。本 Proposal (`ARTIFACT_V3.md`) は内部設計文書であり、設計の出処として minto に言及しているが、実装完了後に削除されて通常ドキュメント / API リファレンスに統合されるため外部には残らない

## 4. Logger / OTel 統合設計

`MintoLogger` 相当の責務と Artifact 永続化を、既存 `ommx.tracing` (OTel) 基盤の上にどう載せるかの設計。本節は方向性のたたき台で、最終決定は別途。

### 4.1 設計原則

- **source of truth を分離する**: parameter / solution / sample set / environment などの記録データ本体は DataStore / Artifact が真実の源となる。OTel は experiment / run のライフサイクル、duration、I/O、エラー、record reference などの実行テレメトリの真実の源となる
- **`MintoLogger` 相当のコンソール出力は OTel span のレンダラとして実装**: `ommx.tracing._render` の延長。並列のログ系統を作らない
- **Artifact 永続化と OTel は双方向に紐付く**:
  - 書き出し時: Artifact build を span として記録、layer annotations にトレース ID を埋める
  - 読み込み時: `Artifact.load` で記録された build-time のトレース情報を取得可能に

### 4.2 現状とのマッピング

ここでは概略のみ示す。`MintoLogger` を構成する個々のメソッドは 4.5 節で詳しく分解する。

| minto の機能 | OTel での表現 |
|---|---|
| `Experiment` 開始/終了 | ルート span (`ommx.experiment`) |
| `Run` 開始/終了 | 子 span (`ommx.run`)、Experiment span の子 |
| `log_parameter(name, value)` | DataStore への書き込み (一次) + run span の event/attribute (二次) |
| `log_instance` / `log_solution` / `log_sampleset` / generic media entry | DataStore への書き込み (一次) + event (record reference)、重い処理は子 span |
| `EnvironmentInfo` 収集 | DataStore / Artifact への first-class record として永続化 (一次) + OTel `Resource` 属性 (`os.type`, `host.arch`, `process.runtime.version` 等の semantic conventions に寄せる) |
| `MintoLogger` のインデント出力 | streaming SpanProcessor / exporter (`_render.py` のリアルタイム版)、experiment/run カテゴリで色分け / アイコン |
| `log_warning` / `log_error` / `log_debug` | OTel Logs 信号、または Rust `tracing::{warn,error,debug}!` (4.5 分類 C) |

`minto` の `log_problem` は `jijmodeling` 固有なので OMMX core API としては提供しない。外部 package が必要なら generic media entry として拡張する。

### 4.3 Artifact ↔ OTel の接続点

接続には 3 つの方向がある。

#### 4.3.1 トレース ID の埋め込み (provenance ID 化)

`ArtifactBuilder.build()` を span に包み、生成された Artifact の manifest annotations に `ommx.trace_id` / `ommx.span_id` を記録 (再現性とプロベナンスのため)。これは ID のみで、span 本体は外部 (OTel collector / backend) に存在する前提。

#### 4.3.2 トレース本体の Artifact 内埋め込み (self-contained trace)

Cloud Run / バッチ系のような **「Artifact 入出力で完結する実行環境」** で必須となる機能。ジョブが Artifact を生成する際の OTel span / event / log 一式を、Artifact の **専用 layer** として埋め込む:

- 配布された Artifact を受け取った側は OTel backend に接続せず、その Artifact 単体で実行履歴 (timing, parameter 記録, error) を再構成できる
- 取り出し側 API: `artifact.get_trace() -> TraceResult` (現行 `ommx.tracing.TraceResult` と互換)
- 形式・media type・粒度 (span のみ / span + log) は 6 章 Q4

この trace layer は DataStore の代替ではない。記録データ本体と EnvironmentInfo は通常の artifact layer / DataStore entry として永続化し、trace layer にはそれらへの参照と実行時系列を入れる。

4.3.1 (ID) と 4.3.2 (本体) は両立する: ID は外部 OTel backend との cross-reference 用、本体は Artifact 単体で完結する用。

#### 4.3.3 Load / Push 操作の計装

`Artifact.load*` / `push` 自体も span 化 (この artifact を **使う** 側のトレース)。Push / Pull の HTTP I/O は OTel semantic conventions の `http.*` 属性で計装する。読み込んだ artifact 由来の build-time trace (4.3.2) は `TraceResult` として load span に link 関係を張る。

### 4.4 階層的コンソールレンダラの位置付け

`MintoLogger` の見た目 (インデント + アイコン + 色) を、`ommx.tracing._render.text_tree()` のオプション付き拡張として再実装する。具体的には:

- 既存の `text_tree()`: 汎用 span ツリー (durations + attributes)
- 追加: `experiment_tree()` あるいは `text_tree(style="experiment")` — `ommx.experiment` / `ommx.run` 名前空間の span を実験フォーマットでレンダリング、その他の span を集約表示

これにより minto Logger の出力は「OTel span ツリーの特定スタイル描画」となり、独立した print 系統を持たない。

### 4.5 MintoLogger は本当に Logger か — 信号の分解

`MintoLogger` は名前に反して **3 つの異なる信号を 1 つのインタフェースに混在させている**。OTel は Traces / Metrics / Logs を別信号として分離する設計なので、最終形では分解して各信号へ振り分けるのが正しい。

呼び出し点 (minto `experiment.py` / `run.py`) を網羅的に分類すると以下になる。

#### 分類 A: ライフサイクル (= Span)

開始/終了がペアで、**`duration` を持つ**。

| メソッド | データ | OTel 表現 |
|---|---|---|
| `log_experiment_start(name)` / `log_experiment_end(name, duration, run_count)` | Experiment 開始/終了 | ルート span `ommx.experiment` |
| `log_run_start(run_id)` / `log_run_end(run_id, duration)` | Run 開始/終了 | 子 span `ommx.run` |
| `log_solver(solver_name, execution_time)` | Solver 実行 | 子 span `ommx.solver` |

これらは **Span に変換するべき**。現状でも `experiment.py:143` の `log_experiment_end` は計算済み duration を引数で受け取っており、span のセマンティクスそのもの。

#### 分類 B: データ登録 (= NOT Log; データ記録の副次的エコー)

`duration` がなく、ユーザが **値を記録する** 呼び出し。

| メソッド | 一次効果 (本来の役目) | 二次効果 |
|---|---|---|
| `Run.log_parameter(name, value)` | DataStore に書き込み → Artifact 永続化 | コンソールに表示 |
| `Run.log_instance` / `log_solution` / `log_sampleset` / generic media entry | DataStore に書き込み → Artifact 永続化 | コンソールに表示 |
| `Run.log_object` / `Experiment.log_global_*` | DataStore に書き込み | コンソールに表示 |

**重要**: これらは「ログを出すための呼び出し」ではなく **データを Artifact に記録するための呼び出し**で、コンソール出力は副次効果に過ぎない。Logger の責務ではなく **DataStore / Artifact の責務**。

OTel への対応:
- 一次: Artifact / DataStore への永続化 (Logger 経由しない)
- 二次: 現在 active な span の **Span Event** (`add_event(name, attributes)`) として記録 — トレース可視化時に「この run で何が記録されたか」が見える
- 三次: コンソール出力は span event を購読する **streaming renderer / exporter** が担当

#### 分類 C: 純粋なログメッセージ (= Log)

タイミングや値ではなく **テキストメッセージ** を出力する。

| メソッド | OTel 表現 |
|---|---|
| `log_warning(message)` | OTel Logs (severity=WARN) または `tracing::warn!` |
| `log_error(message)` | OTel Logs (severity=ERROR) または `tracing::error!` |
| `log_debug(message)` | OTel Logs (severity=DEBUG) または `tracing::debug!` |
| `log_environment_info(info)` | 情報本体は DataStore / Artifact の EnvironmentInfo entry。OTel `Resource` 属性にも写し、起動時のレンダラ出力は見た目だけを担当 |

これらは OTel **Logs 信号** または Rust 側 `tracing::{warn,error,debug}!` に直結させるのが素直。Python 側は `logging` モジュール + `LoggingHandler` (OTel SDK 提供) でブリッジできる。`log_environment_info` は情報収集そのものではなく、収集済み EnvironmentInfo の表示フックとして扱う。

#### 結論: MintoLogger 相当のクラスは存在しない

最終形では `ommx.artifact.logger` のような単一クラスは作らず、**MintoLogger を解体する**:

| 元の機能 | 新しい所属先 |
|---|---|
| 分類 A (ライフサイクル) | `Experiment` / `Run` / `Solver` のスコープ entry/exit が OTel span を発行 (Rust `tracing::info_span!` か Python `tracer.start_as_current_span`) |
| 分類 B (データ登録) | `DataStore.add()` / `ArtifactBuilder.add_*()` の責務。副次的に span event を発火 |
| 分類 C (純粋ログ) | Python `logging` + OTel `LoggingHandler`、または Rust `tracing::{warn,error,debug}!` |
| インデント/絵文字つき表示 | OTel span の **streaming exporter / SpanProcessor** が担当 (`_render.py` のリアルタイム版) |

ユーザ視点の API:
- `with experiment.run() as run:` のようなスコープ → 分類 A
- `run.log_parameter(...)`, `run.log_solution(...)` → 分類 B (DataStore 呼び出しを行い、span event を発火)
- 警告・エラー → 分類 C (Logger 経由でなく標準ロガー経由)

最終形に存在するのは:
- Experiment/Run のスコープ管理クラス (5 章の DataStore/ExperimentDataSpace と統合)
- OTel streaming exporter (現行 `ommx.tracing._render` のリアルタイム拡張)
- 通常の Python `logging` 設定

→ 4.4 節「階層レンダラを `_render.py` 拡張として」と整合。`MintoLogger` の見た目は失われない (リアルタイム exporter が同じ出力を生成する) が、内部表現は OTel の 3 信号に正規化される。

### 4.6 リアルタイム vs ポストホック

`ommx.tracing` 現行は **ポストホック** (capture 終了後に span ツリーを取得) で動く。`MintoLogger` 相当の体験は **リアルタイム** (各 log_* 呼び出し時に行を出力) なので、最終形が両者をどう備えるかが論点:

- (a) `BatchSpanProcessor` ではなく即時フラッシュする `SimpleSpanProcessor` 系のリアルタイムレンダラを追加
- (b) ポストホック方式に統一し、ユーザコードでは `with experiment(...) as exp:` のスコープ完了後にツリー描画
- (c) 両方提供 (capture API はポストホック、`%%ommx_experiment` cell magic はリアルタイム描画)

選択は 6 章 Q2 を参照。なお 4.5 節で MintoLogger を分解する場合、リアルタイム描画は streaming SpanProcessor として実装される (これが分類 A/B のコンソール出力の唯一の経路)。

## 5. DataStore と系譜 (lineage) モデル

DataStore の最終形と artifact 同士の関係 (lineage) をどう表現するかは、Artifact v3 全体の設計骨格。本節でまとめる。

### 5.1 DataStore の構造再分析

minto `DataStore` (`minto/datastore.py:362-`) の実体は **「名前付きの型別 dict」を 7 つ束ねたもの** で、構造的に 2 カテゴリに分かれている。ただし OMMX core の取り込み対象から `problems` は除く (`jijmodeling` 依存を持ち込まないため)。

| カテゴリ | 種別 | OCI Artifact マッピング |
|---|---|---|
| **エントリ単位 (per-entry)** | `instances`, `solutions`, `samplesets`, `objects`, generic media entries | 名前ごとに 1 layer (1 blob、digest で内容アドレス) |
| **集約 dict (aggregate)** | `parameters`, `meta_data` | dict 全体で 1 layer |

エントリ単位型は OCI 不変性と整合する (1 名前 = 1 blob、digest 一致で deduplicate)。集約 dict 型は **構造上 update を要求**する (キー追加で layer 全体を再エンコード)。これが現 minto の設計で最も歪んでいる箇所。

**v3 では集約 dict を per-entry に正規化** する: `parameters["alpha"] = 0.1` は `("alpha", 0.1)` という独立 layer になる。これで OCI 内のすべての layer が append-only に統一される。

現 minto の `problems` 相当は、OMMX core では組み込み storage として持たない。`jijmodeling` など外部ドメインのデータは、外部 package が media type と codec を登録して generic media entry として扱う。

### 5.2 不変性と "更新" のセマンティクス

minto の `add()` (`datastore.py:446`) は事実上 **upsert (同名再呼び出しで黙って上書き)** だが、OCI Artifact は content-addressable で **layer の in-place 更新は仕様上存在しない** (内容を変えると digest が変わる = 別 blob)。「更新」を扱う方針は 3 つの相のどこで起こるかで分かれる:

- **Build 相 (in-memory, 可変)**: Builder のメソッド呼び出し中。upsert を許容
- **Seal 相 (build/save の瞬間)**: スナップショットを取って immutable な artifact を生成
- **Read 相 (load 後)**: 永続化 artifact のビュー、変更不可

「update」というプリミティブは **存在しない**。in-memory dict 操作は実装詳細、永続層では **新しい artifact を作る** のが唯一の "更新" 経路。

### 5.3 Git ↔ OCI v1.1 ↔ OMMX の三者対応

OCI Image Spec v1.1 で導入された manifest の `subject` フィールドは、Git の `parent` ポインタと同型のセマンティクスを OCI 側で正規に持つ機構である。これによって artifact の系譜が **annotation の俺ルールではなく標準仕様で表現できる**。

| 層 | Git | OCI v1.1 | OMMX Artifact |
|---|---|---|---|
| 内容アドレス層 | blob (`sha1`) | descriptor → blob (`sha256`) | Instance / Solution / SampleSet 等の実体 |
| スナップショット層 | tree | manifest (`layers[]`) | 1 つの experiment 状態 |
| 履歴ノード | commit (parents, author, message) | manifest + `subject` | 派生関係を持つ artifact |
| 可変参照 | branch / tag (refs) | tag | `experiment:latest`, `experiment:v2` |
| 履歴グラフ走査 | `git log` | `subject` chain + Referrers API | `experiment.history()` |

`subject` は SBOM・署名・provenance attestation 用に v1.1 で追加された、manifest 内に「別 manifest を指す descriptor」を書けるフィールド。OMMX experiment の系譜表現に流用するのは仕様意図とも整合する。

OMMX Artifact v3 では、各 manifest は **full snapshot** とする。`subject` は lineage / provenance のためのリンクであり、子 artifact を読むための必須 dependency ではない。派生 artifact の `layers[]` には、その時点の DataStore view を復元するために必要な全 descriptor を載せる。既存 blob は同じ digest の descriptor として再利用でき、remote registry では dedup / mount され得るが、archive / dir 形式ではその artifact 単体で読めるよう参照 blob を含める。

```jsonc
// experiment v2 の manifest
{
  "schemaVersion": 2,
  "artifactType": "application/org.ommx.experiment.v1",
  "config":  { ... },
  "layers":  [ ... v2 の完全な DataStore snapshot ... ],
  "subject": {                              // v1 を指す
    "mediaType": "application/vnd.oci.image.manifest.v1+json",
    "digest":    "sha256:...v1...",
    "size":      1234
  }
}
```

履歴を過去に辿る場合は、現在の manifest の `subject` を再帰的に読む。Referrers API (`/v2/<name>/referrers/<digest>`) は「この artifact を subject にしている子 manifest 一覧」を取得するために使い、branch heads の探索や派生一覧表示に対応する。

### 5.4 三層 (Snapshot 内 / Build vs View / 系譜) は直交

DataStore と lineage の設計は、Git アナロジーで見ると **層が異なる 3 つの軸が直交**している:

- **A: "1 つの commit (= manifest) の中" の局所ルール**: tree 内で同名上書きを許すか。Git でいう「同じ commit で同じファイルパスを 2 回書けるか」レベル
- **B: working tree と HEAD の分離**: Git の working tree (mutable) と HEAD が指す commit (immutable) の関係そのもの。Builder = staging + working tree、View = 特定 commit のチェックアウト
- **C: commit DAG**: `subject` リンクで複数 manifest の関係を表現

```
─ 個々の build ─────  ← A (スナップショット内の add 規則)
│
├ Builder (mutable) ─┐
│                    ├ B (相の分離)
└ View    (read-only)┘
│
└ history (subject chain) ─ C (DAG)
```

最終形では **三層すべてを採用** する:

- A: aggregate dict を per-entry 化、Build 相での同名上書きは内部 dict 操作として許容、build 時にスナップショット
- B: `ArtifactBuilder` / `Artifact` (View) を型レベルで分離、Read 相の View に `add` メソッドは生やさない
- C: OCI v1.1 `subject` を使った lineage、`Builder.from_parent(view)` で派生関係を明示的に張る。ただし子 artifact は full snapshot であり、parent は復元の必須依存ではない

### 5.5 派生のユースケース

| ケース | 最終形での扱い |
|---|---|
| ループ内で進捗を逐次記録 | Build 相で各 iteration を独立 entry として add |
| post-hoc メトリクス追加 | 旧 artifact を parent とする新 artifact。既存 descriptor を再掲し、追加メトリクス layer を加えた full snapshot |
| 既存 solution への "best" タグ | 既存 descriptor を再掲し、tag annotation layer を加えた full snapshot。parent は元 experiment |
| パラメータの誤りを訂正 | 訂正版 full snapshot を派生 artifact として作る (履歴は保持) |
| 複数 experiment の統合 | 単一 `subject` では表現不能、複数 parent 規約は 6 章 Q7 |

未解決の設計点は 6 章 Q6〜Q10 (commit 粒度、parent 数、tag 可視性、GC、diff API)。

## 6. 未決の論点 (Open Questions)

以下は本 Proposal で **決定しない** 事項。別途議論する。

### Q1. ~~minto リポジトリの今後~~ [決定済み: 3.4 参照]

OMMX 側では minto の今後について立場を取らない。OMMX のドキュメント・API リファレンスは minto に一切言及しないため、minto の位置付け・責務分担・移行ガイドはすべて OMMX のスコープ外となる。

### Q2. Logger / OTel 統合のリアルタイム性

4.6 節の選択。最終形がリアルタイムレンダラを提供するか、ポストホックに統一するか、両方提供か。

### Q3. Artifact manifest annotation への trace_id 埋め込み

4.3.1 節。Artifact の manifest annotations に build 時の `trace_id` / `span_id` を残すか。残す場合のキー命名規約 (`org.ommx.trace.*` 等) と、再ロード時の取り扱い (新しいトレースの `links` として張る? 単なる属性として表示?)。

### Q4. トレース本体の Artifact 内埋め込み

4.3.2 節。「埋め込む」自体は決定済み (Cloud Run / バッチ系での要件、1 章方針)。残る論点:

- **シリアライズ形式**: OTLP (Protobuf / JSON) / Chrome Trace Event Format / 両方提供
- **media type 命名**: `application/vnd.opentelemetry.trace.otlp+json` / 独自 `application/org.ommx.trace+json` / 他
- **対象信号**: span のみか、span + OTel Logs か、将来的に metric も含めるか
- **always-on vs opt-in**: デフォルトで埋め込むか、`ArtifactBuilder.with_trace()` のような明示 API か

### Q5. OTel semantic conventions の採用範囲

`EnvironmentInfo` は DataStore / Artifact entry として永続化する。そのうえで OTel `Resource` semantic conventions (`os.*`, `host.*`, `process.*`) に寄せるか、OMMX 独自の `ommx.*` namespace に閉じるか、両方併記か。

### Q6. commit 単位の粒度

「1 manifest = 1 commit」とする時、commit を打つ自然な単位は?

- (i) 各 `Run` 終了で 1 manifest — Git 風で履歴が密
- (ii) 各 `Experiment` 終了で 1 manifest — 現 minto の運用に近い
- (iii) ユーザが明示的に `build()` / `commit()` を呼んだ時点 — 制御は明示的だが粒度判断をユーザに押しつける

基本 (iii) を採用しつつ、`with experiment.run() as run:` の終了で自動 commit する糖衣を提供するかが論点。

### Q7. parent ポインタの単数 / 複数

OCI v1.1 `subject` は単一フィールド。複数 experiment を統合する merge 系の操作を最終形で扱うか:

- 当面は単一 parent で十分とする (linear history のみ)
- 多 parent を annotation 規約 (例: `org.ommx.parents` に複数 digest を列挙) で表現する独自拡張を入れる

### Q8. tag (mutable ref) のユーザ可視性

`experiment:latest`, `experiment:v2` のような tag (Git の branch 相当) を OMMX が API として前面に出すか、内部実装にとどめるか。前面に出すと "branch を切る" 概念をユーザに教える必要がある。

### Q9. Garbage collection

長期運用で古い manifest や未参照 blob が registry に滞留する。最終形が GC 機能を含むか:

- `ommx artifact gc` 相当のサブコマンドを最終 API に含める
- 設計上のフックだけ用意し、実 GC は外部ツール (registry の管理機能) に委ねる
- スコープ外とする

### Q10. diff / lineage 走査 API

`experiment.history()`, `experiment.diff(other)`, `experiment.parent()` を最終 API に含めるか。manifest の layer descriptor 列を比較する diff は実装的には軽量。

## 7. リスクと懸念

- **OCI Distribution の HTTP 周りが最重量**: auth (Bearer / Basic), manifest PUT/GET, blob upload session (cross-repo mount, chunked upload) を網羅すると相応の実装量
- **既存 `.ommx` ファイルの後方互換**: OCI Image Layout 標準を逸脱していなければ影響なしだが、annotations のキー命名等で OMMX 固有の慣習があれば要確認
- **`ocipkg` の public surface 撤去**: `ommx::ocipkg` re-export、Rust/Python の `Descriptor` / `Digest` / `MediaType` 露出をどう置き換えるかは migration note が必要
- **minto ユーザへの影響**: 取り込みのタイミング・API 変更の予告期間
- **OTel TracerProvider の二重登録**: 既存ユーザコードや外部計装が独自に provider を立てている場合、`ommx.tracing` の lazy 初期化と衝突しないか要検証 (OTel は provider 差し替えを拒否するため、テスト初期化順が壊れやすい — `test_tracing_magic.py` の冒頭参照)
- **Logger の出力先**: stdout 直書きから OTel 経由に変えると、OTel exporter 未設定環境ではコンソールに何も出ない事故が起こり得る。デフォルトで NoOp ConsoleExporter を載せるか、明示的に opt-in させるかの判断が必要
- **OCI v1.1 `subject` フィールドのレジストリ対応**: 全レジストリが v1.1 manifest と Referrers API を実装しているわけではない (古い ghcr.io 設定や self-hosted の一部)。archive / dir 形式は完全制御できるので影響なし。remote 経由で `subject` push が拒否される registry への fallback 戦略 (annotation でのポリフィル / 系譜情報を諦めて push 続行) は要検討

## 8. 参考: 関連ファイル

### OMMX Rust
- `rust/ommx/src/artifact.rs`
- `rust/ommx/src/artifact/{builder,annotations,media_types,config}.rs`
- `rust/ommx/Cargo.toml` (ocipkg 依存)
- `rust/ommx/examples/{create,pull}_artifact.rs`

### OMMX Python
- `python/ommx/src/artifact.rs` (PyO3 バインディング)
- `python/ommx/Cargo.toml`
- `python/ommx-tests/tests/test_descriptor.py`

### OMMX Tracing / OTel
- `python/ommx/ommx/tracing/{__init__,_capture,_collector,_render,_setup,_magic}.py`
- `python/ommx/Cargo.toml` の `tracing-bridge` feature と `pyo3-tracing-opentelemetry` 依存
- `python/ommx-tests/tests/{test_tracing,test_tracing_capture,test_tracing_magic}.py`
- 各 adapter の `tests/test_tracing.py` (4 アダプタ共通)

### ocipkg (置換対象)
- `/Users/termoshtt/github.com/termoshtt/ocipkg/ocipkg/src/lib.rs`
- 主要モジュール: `image`, `media_types`, `local`, `distribution` (feature-gated)

### minto (吸収候補)
- `minto/{experiment,run,datastore,exp_dataspace,table,environment}.py`
- `minto/pyproject.toml`
