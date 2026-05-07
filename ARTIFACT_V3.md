# OMMX Artifact v3 Proposal

OMMX Artifact レイヤの刷新計画。本ドキュメントは決定事項ではなく **議論のたたき台** であり、未決論点を明示する。

## 1. 背景と動機

OMMX Artifact は OCI Image / Artifact 仕様に乗せて最適化問題・解・実験メタデータを配布する仕組みで、現在は外部クレート [`ocipkg`](https://github.com/termoshtt/ocipkg) に薄く依存している。一方で実験トラッキング層である [`minto`](https://github.com/Jij-Inc/minto) は `ommx>=2.0.0` の Artifact API に依存しつつ、Experiment / Run / DataStore 階層・provenance 収集・DataFrame 集計といった「Artifact に居場所があるべき機能」を Python 側で抱えている。

v3 では以下を狙う:

1. **`ocipkg` 依存を撤去** し、Artifact の OCI 取り扱いを `ommx` クレート内の自前実装に置き換える。
2. **`minto` のフロントエンド機能のうち汎用部分を吸収** し、minto は薄いラッパまたはドメイン固有層に絞る。
3. Artifact 層のテストカバレッジを底上げし、上記変更の回帰検出を可能にする。

## 2. 現状把握

### 2.1 Rust 側 (`rust/ommx/src/artifact/`)

5 モジュール構成:

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

依存は **artifact モジュールに局所化**されており、他モジュールへの漏れはほぼゼロ。

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
| `DataStore` (`minto/datastore.py`) | 型別ストレージ戦略 (JSON / JijModeling Problem / OMMX Instance/Solution/SampleSet) を pluggable に束ねる | `ommx.artifact.datastore` 相当 |
| `ExperimentDataSpace` (`minto/exp_dataspace.py`) | experiment / runs/{i}/ の階層構造、layer annotation で experiment データと run データを区別 | `ommx.artifact.experiment` 相当 |
| `table.create_table_from_stores` (`minto/table.py`) | DataStore 群から pandas DataFrame を生成 | `ommx.artifact.export` または同等 |
| `EnvironmentInfo` (`minto/environment.py`) | 実験プロベナンス (OS / CPU / Python / パッケージバージョン) | `ommx.artifact.environment` 相当 |
| `MintoLogger` (`minto/logger.py`, `minto/logging_config.py`) | experiment / run イベントの階層的コンソール出力 (インデント・アイコン) | クラスとしては吸収しない。5.5 節で 3 信号 (Span / DataStore+Event / OTel Logs) に分解、コンソール表示は streaming exporter に集約 |

**残す**: `minto.problems.*` (TSP/Knapsack/CVRP ジェネレータ) — ドメイン固有の問題ジェネレータは minto 側に残す

### 2.6 OMMX の OTel 連携現状

OMMX v3 では Rust 側 `tracing` を Python 側 OpenTelemetry に橋渡しする方針が既に走っている。Artifact v3 / Logger 吸収はこの基盤の上に載せる必要がある。

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

**示唆**: トレース管理の本流は既に OTel である。minto の `MintoLogger` を **OTel と並列の独立ログ系統として持ち込むのは避け**、span / event / attribute に変換する方向で設計するのが整合的。

## 3. 目的とスコープ

### 3.1 In Scope

- `rust/ommx/src/artifact/` 内の OCI 取り扱いコードの自前化
- `Cargo.toml` から `ocipkg` の削除
- minto の汎用機能 (上記 5 つ: DataStore / ExperimentDataSpace / table / EnvironmentInfo / Logger) の **再設計を含む** OMMX への取り込み
- Artifact 層の Rust 統合テスト + Python ラウンドトリップテストの追加

### 3.2 Out of Scope

- 既存 `.ommx` ファイル形式の変更 (OCI Image Layout 互換は維持)
- `ommx.v1` の Instance/Solution/SampleSet 等のスキーマ変更
- minto のドメイン固有機能 (`problems.*` の問題ジェネレータ) の取り込み

### 3.3 非目的

- パフォーマンス改善は副次的目標であり、主目標ではない (ベンチマークなしの性能主張は禁止: CLAUDE.md 参照)

### 3.4 互換性スタンス

- **minto との互換性は API レベルでは維持しない**。`minto.Experiment` / `minto.Run` のクラス階層やメソッドシグネチャをそのまま OMMX に持ち込む必要はない。**ユーザ体験 (実験を作って run を回し記録、artifact として配布、再ロードして解析、というフロー)** が同等に成立すれば十分
- この自由度を活かし、minto が暗黙に妥協していた点 (DataStore の dual nature、aggregate dict の更新、Logger の信号混在) を v3 では正面から再設計する (6 章で詳述)
- OMMX 自身の v2 → v3 の Python API 互換性は別論点 (7 章 Q3 参照)

## 4. 移行計画案 (Phased Plan)

各フェーズは独立した PR セットとして切る想定。フェーズ間の順序は **未決** (5 章 Open Question 参照)。

### Phase A: テスト整備 (前提)

- `rust/ommx/tests/artifact/` を新設し、archive 作成→読み出しのラウンドトリップ、annotations 永続化、media type 検証をカバー
- Python 側 `python/ommx-tests/tests/test_artifact.py` を新設し、`ArtifactBuilder` → `Artifact` のラウンドトリップ、`add_instance`/`get_instance` 等の往復をテスト
- 既存 `examples/` を統合テストに昇格、または削除

### Phase B: ocipkg 撤去 (Core)

OCI Image Layout / OCI Artifact 仕様に準拠した薄いコアを `rust/ommx/src/artifact/oci/` に新設:

- `oci::layout` — OCI Image Layout (`oci-layout`, `index.json`, `blobs/sha256/`) の読み書き
- `oci::manifest` — OCI Image Manifest v1 (artifactType 対応) の serde
- `oci::descriptor` / `oci::digest` / `oci::media_type` — 基本型
- `oci::archive` — tar / tar.gz の薄いラッパ
- `oci::image` — `Image` トレイトに相当する内部抽象 (archive / dir 共通)
- 既存 `artifact.rs` / `builder.rs` を新コアに繋ぎ替え、外向き API は維持

依存追加候補: `tar`, `flate2`, `oci-spec` (型定義のみ利用), `sha2`

### Phase C: Remote 機能の処遇

`remote-artifact` feature で提供している pull/push を、Phase B のコアの上にどう載せるか。複数案あり (5 章 Open Question 参照)。

### Phase D: minto 機能の再設計と取り込み

3.4 の通り API 互換は不要なので、minto の以下 5 機能を **再設計しつつ** OMMX に取り込む。配置は Rust / Python のどちらが妥当かは要検討:

- `EnvironmentInfo`: Python 側で十分 (sys / platform に依存)。OTel `Resource` 属性として埋め込む方針も並行検討 (5 章)
- `DataStore` / `ExperimentDataSpace`: 6 章 (Builder/View 分離、aggregate dict の per-entry 化、`subject` による系譜) を踏まえて再設計。Rust コア + 薄い Python ラッパが基本線
- `table` 集計: pandas 依存なので Python 側
- `Logger`: 単一クラスとしては取り込まず、5.5 節の分解に従って Span (分類 A) / DataStore + Span Event (分類 B) / OTel Logs (分類 C) の 3 経路に振り分ける。`MintoLogger` クラスは v3 で消失

minto リポジトリの今後の位置付けは 7 章 Q4 を参照。

### Phase E: 後片付け

- ドキュメント更新 (`PYTHON_SDK_MIGRATION_GUIDE.md`, `rust/ommx/doc/migration_guide.md`)
- minto 側の deprecation アナウンス

## 5. Logger / OTel 統合設計

minto Logger 吸収と Artifact 永続化を、既存 `ommx.tracing` (OTel) 基盤の上にどう載せるかの設計。本節は方向性のたたき台で、最終決定は別途。

### 5.1 設計原則

- **OTel を単一の真実の源 (single source of truth) とする**: experiment / run のライフサイクル、parameter / solution の記録、Artifact の load / push などは全て span / event / attribute として OTel に流す
- **`MintoLogger` 相当のコンソール出力は OTel span のレンダラとして実装**: `ommx.tracing._render` の延長。並列のログ系統を作らない
- **Artifact 永続化と OTel は双方向に紐付く**:
  - 書き出し時: Artifact build を span として記録、layer annotations にトレース ID を埋める
  - 読み込み時: `Artifact.load` で記録された build-time のトレース情報を取得可能に

### 5.2 現状とのマッピング

ここでは概略のみ示す。`MintoLogger` を構成する個々のメソッドは 5.5 節で詳しく分解する。

| minto の機能 | OTel での表現案 |
|---|---|
| `Experiment` 開始/終了 | ルート span (`ommx.experiment`) |
| `Run` 開始/終了 | 子 span (`ommx.run`)、Experiment span の子 |
| `log_parameter(name, value)` | DataStore への書き込み (一次) + run span の event/attribute (二次) |
| `log_problem` / `log_instance` / `log_solution` / `log_sampleset` | DataStore への書き込み (一次) + event (record reference)、重い処理は子 span |
| `EnvironmentInfo` 収集 | OTel `Resource` 属性 (`os.type`, `host.arch`, `process.runtime.version` 等の semantic conventions に寄せる) + 独自 namespace |
| `MintoLogger` のインデント出力 | streaming SpanProcessor / exporter (`_render.py` のリアルタイム版)、experiment/run カテゴリで色分け / アイコン |
| `log_warning` / `log_error` / `log_debug` | OTel Logs 信号、または Rust `tracing::{warn,error,debug}!` (5.5 分類 C) |

### 5.3 Artifact ↔ OTel の接続点

- **Build 時**: `ArtifactBuilder.build()` を span に包み、生成された Artifact の manifest annotations に `ommx.trace_id` / `ommx.span_id` を記録 (再現性とプロベナンスのため)
- **Load 時**: `Artifact.load*` も span 化、レイヤ取得を子 span に。読み込んだ artifact 由来の trace 情報を `TraceResult` 経由で参照できる API を検討
- **Push / Pull**: `remote-artifact` 経由のネットワーク I/O は OTel semantic conventions の `http.*` 属性で計装する

### 5.4 階層的コンソールレンダラの位置付け

`MintoLogger` の見た目 (インデント + アイコン + 色) を、`ommx.tracing._render.text_tree()` のオプション付き拡張として再実装する。具体的には:

- 既存の `text_tree()`: 汎用 span ツリー (durations + attributes)
- 追加: `experiment_tree()` あるいは `text_tree(style="experiment")` — `ommx.experiment` / `ommx.run` 名前空間の span を実験フォーマットでレンダリング、その他の span を集約表示

これにより minto Logger の出力は「OTel span ツリーの特定スタイル描画」となり、独立した print 系統を持たない。

### 5.5 MintoLogger は本当に Logger か — 信号の分解

`MintoLogger` は名前に反して **3 つの異なる信号を 1 つのインタフェースに混在させている**。OTel は Traces / Metrics / Logs を別信号として分離する設計なので、吸収にあたっては分解して各信号へ振り分けるのが正しい。

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
| `Run.log_problem` / `log_instance` / `log_solution` / `log_sampleset` | DataStore に書き込み → Artifact 永続化 | コンソールに表示 |
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
| `log_environment_info(info)` | OTel `Resource` 属性 (情報そのもの) + 起動時のレンダラ出力 (見た目) |

これらは OTel **Logs 信号** または Rust 側 `tracing::{warn,error,debug}!` に直結させるのが素直。Python 側は `logging` モジュール + `LoggingHandler` (OTel SDK 提供) でブリッジできる。

#### 結論: MintoLogger は丸ごと吸収しない、責務を分解する

吸収方針は「`ommx.artifact.logger` モジュールを作る」ではなく、**MintoLogger を解体する**:

| 元の機能 | 新しい所属先 |
|---|---|
| 分類 A (ライフサイクル) | `Experiment` / `Run` / `Solver` のスコープ entry/exit が OTel span を発行 (Rust `tracing::info_span!` か Python `tracer.start_as_current_span`) |
| 分類 B (データ登録) | `DataStore.add()` / `ArtifactBuilder.add_*()` の責務。副次的に span event を発火 |
| 分類 C (純粋ログ) | Python `logging` + OTel `LoggingHandler`、または Rust `tracing::{warn,error,debug}!` |
| インデント/絵文字つき表示 | OTel span の **streaming exporter / SpanProcessor** が担当 (`_render.py` のリアルタイム版) |

ユーザ視点の API:
- `with experiment.run() as run:` のようなスコープ → 分類 A
- `run.log_parameter(...)`, `run.log_solution(...)` → 分類 B (現状の DataStore 呼び出しを維持、span event を追加)
- 警告・エラー → 分類 C (Logger 経由でなく標準ロガー経由)

この分解を経由すると **`MintoLogger` クラスそのものは v3 で消える**。代わりに登場するのは:
- Experiment/Run のスコープ管理クラス (Phase D の DataStore/ExperimentDataSpace と統合)
- OTel streaming exporter (現行 `ommx.tracing._render` のリアルタイム拡張)
- 通常の Python `logging` 設定

→ 5.4 節「階層レンダラを `_render.py` 拡張として」と整合。`MintoLogger` の見た目は失われない (リアルタイム exporter が同じ出力を生成する) が、内部表現は OTel の 3 信号に正規化される。

### 5.6 リアルタイム vs ポストホック

`ommx.tracing` 現行は **ポストホック** (capture 終了後に span ツリーを取得) で動く。minto Logger は **リアルタイム** (各 log_* 呼び出し時に行を出力) なので、UX 上の差を埋めるには:

- (a) `BatchSpanProcessor` ではなく即時フラッシュする `SimpleSpanProcessor` 系のリアルタイムレンダラを追加
- (b) 既存ポストホック方式に統一し、ユーザコードでは `with experiment(...) as exp:` のスコープ完了後にツリー描画
- (c) 両方提供 (capture API はポストホック、`%%ommx_experiment` cell magic はリアルタイム描画)

選択は 7 章 Q6 を参照。なお 5.5 節で MintoLogger を分解する場合、リアルタイム描画は streaming SpanProcessor として実装される (これが分類 A/B のコンソール出力の唯一の経路)。

## 6. DataStore と系譜 (lineage) モデル

DataStore の刷新と artifact 同士の関係 (lineage) をどう表現するかは、ocipkg 撤去 (Phase B) と minto 機能の再設計 (Phase D) の両方を貫く設計骨格。本節でまとめる。

### 6.1 DataStore の構造再分析

minto `DataStore` (`minto/datastore.py:362-`) の実体は **「名前付きの型別 dict」を 7 つ束ねたもの** で、構造的に 2 カテゴリに分かれている。

| カテゴリ | 種別 | OCI Artifact マッピング |
|---|---|---|
| **エントリ単位 (per-entry)** | `problems`, `instances`, `solutions`, `samplesets`, `objects` | 名前ごとに 1 layer (1 blob、digest で内容アドレス) |
| **集約 dict (aggregate)** | `parameters`, `meta_data` | dict 全体で 1 layer |

エントリ単位型は OCI 不変性と整合する (1 名前 = 1 blob、digest 一致で deduplicate)。集約 dict 型は **構造上 update を要求**する (キー追加で layer 全体を再エンコード)。これが現 minto の設計で最も歪んでいる箇所。

**v3 では集約 dict を per-entry に正規化** する: `parameters["alpha"] = 0.1` は `("alpha", 0.1)` という独立 layer になる。これで OCI 内のすべての layer が append-only に統一される。

### 6.2 不変性と "更新" のセマンティクス

minto の `add()` (`datastore.py:446`) は事実上 **upsert (同名再呼び出しで黙って上書き)** だが、OCI Artifact は content-addressable で **layer の in-place 更新は仕様上存在しない** (内容を変えると digest が変わる = 別 blob)。「更新」を扱う方針は 3 つの相のどこで起こるかで分かれる:

- **Build 相 (in-memory, 可変)**: Builder のメソッド呼び出し中。upsert を許容
- **Seal 相 (build/save の瞬間)**: スナップショットを取って immutable な artifact を生成
- **Read 相 (load 後)**: 永続化 artifact のビュー、変更不可

「update」というプリミティブは **存在しない**。in-memory dict 操作は実装詳細、永続層では **新しい artifact を作る** のが唯一の "更新" 経路。

### 6.3 Git ↔ OCI v1.1 ↔ OMMX の三者対応

OCI Image Spec v1.1 で導入された manifest の `subject` フィールドは、Git の `parent` ポインタと同型のセマンティクスを OCI 側で正規に持つ機構である。これによって artifact の系譜が **annotation の俺ルールではなく標準仕様で表現できる**。

| 層 | Git | OCI v1.1 | OMMX Artifact |
|---|---|---|---|
| 内容アドレス層 | blob (`sha1`) | descriptor → blob (`sha256`) | Instance / Solution / SampleSet 等の実体 |
| スナップショット層 | tree | manifest (`layers[]`) | 1 つの experiment 状態 |
| 履歴ノード | commit (parents, author, message) | manifest + `subject` | 派生関係を持つ artifact |
| 可変参照 | branch / tag (refs) | tag | `experiment:latest`, `experiment:v2` |
| 履歴グラフ走査 | `git log` | Referrers API | `experiment.history()` |

`subject` は SBOM・署名・provenance attestation 用に v1.1 で追加された、manifest 内に「別 manifest を指す descriptor」を書けるフィールド。OMMX experiment の系譜表現に流用するのは仕様意図とも整合する。

```jsonc
// experiment v2 の manifest
{
  "schemaVersion": 2,
  "artifactType": "application/org.ommx.experiment.v1",
  "config":  { ... },
  "layers":  [ ... 追加された layer のみ ... ],
  "subject": {                              // v1 を指す
    "mediaType": "application/vnd.oci.image.manifest.v1+json",
    "digest":    "sha256:...v1...",
    "size":      1234
  }
}
```

Referrers API (`/v2/<name>/referrers/<digest>`) で「この artifact を subject にしている manifest 一覧」を取得できるため、registry 側の標準機能として `git log` 相当の走査が成立する。

### 6.4 設計案 A / B / C は層が違う (直交)

前段の議論で挙げた A (append-only) / B (Builder/View 分離) / C (系譜) は当初並列の選択肢として整理したが、Git アナロジーで見ると **層が異なり競合しない**:

- **A は "1 つの commit の中" の局所ルール**: tree 内で同名上書きを許すか。Git でいう「同じ commit で同じファイルパスを 2 回書けるか」レベル
- **B は working tree と HEAD の分離**: Git の working tree (mutable) と HEAD が指す commit (immutable) の関係そのもの。Builder = staging + working tree、View = 特定 commit のチェックアウト
- **C は commit DAG**: `subject` リンクで複数 manifest の関係を表現

```
─ 個々の build ─────  ← A (スナップショット内の add 規則)
│
├ Builder (mutable) ─┐
│                    ├ B (相の分離)
└ View    (read-only)┘
│
└ history (subject chain) ─ C (DAG)
```

v3 では **三層すべてを採用** する:

- A: aggregate dict を per-entry 化、Build 相での同名上書きは内部 dict 操作として許容、build 時にスナップショット
- B: `ArtifactBuilder` / `Artifact` (View) を型レベルで分離、Read 相の View に `add` メソッドは生やさない
- C: OCI v1.1 `subject` を使った lineage、`Builder.from_parent(view)` で派生関係を明示的に張る

### 6.5 派生のユースケース

| ケース | v3 での扱い |
|---|---|
| ループ内で進捗を逐次記録 | Build 相で各 iteration を独立 entry として add |
| post-hoc メトリクス追加 | 旧 artifact を parent とする新 artifact、追加メトリクスのみを layer に |
| 既存 solution への "best" タグ | annotation 専用 layer を含む派生 artifact、parent は元 experiment |
| パラメータの誤りを訂正 | 訂正版を派生 artifact として作る (履歴は保持) |
| 複数 experiment の統合 | 単一 `subject` では表現不能、複数 parent 規約は 7 章 Q10 |

未解決の設計点は 7 章 Q9〜Q13 (commit 粒度、parent 数、tag 可視性、GC、diff API)。

## 7. 未決の論点 (Open Questions)

以下は本 Proposal で **決定しない** 事項。別途議論する。

### Q1. 進行順序

- (a) Phase B (ocipkg 撤去) を先に完了 → Phase D (minto 吸収)
- (b) Phase D を先に完了 (ocipkg 依存は維持したまま吸収) → Phase B
- (c) 並行ブランチで両方を進めて後でマージ

### Q2. Remote 機能の扱い

- (a) OCI Distribution Spec を `reqwest` ベースで自前実装
- (b) 既存クレート (`oci-distribution`, `oci-client` 等) に切り替え (ocipkg 排除のみ達成、外部依存は残る)
- (c) v3 では archive/dir のみサポートし、remote は別タスクとして後続マイルストーンに送る

### Q3. 後方互換性ポリシー

- 既存 Python API (`Artifact.load_archive` / `ArtifactBuilder.new_archive` 等) のシグネチャを v3 で維持するか
- minto 吸収に伴う `ommx.artifact` の API 拡張は破壊的変更とせず additive にできるか
- Rust 側 v3 Stage Pattern (`rust/ommx/doc/migration_guide.md`) との整合

### Q4. minto リポジトリの今後

API 互換は不要 (3.4) と決定済み。残る論点:

- minto を独立ライブラリとして維持し、OMMX の Artifact API を利用するユーザ層に位置付けるか
- 主要機能が再設計・移管された後の minto の責務 (`problems.*` の問題ジェネレータ + ドメイン UI のみ?)
- 既存 minto ユーザの v3 への移行ガイド

### Q5. ocipkg 内製化の動機の言語化

- バージョン整合の管理コスト?
- OMMX 固有の機能ギャップ?
- ライセンス / 配布の都合?

理由を明文化することで Phase B/C の設計判断 (どこまで薄くするか) の指針となる。

### Q6. Logger / OTel 統合のリアルタイム性

5.5 節の選択。リアルタイムレンダラを追加するか、ポストホック統一か、両方提供か。

### Q7. Artifact annotation へのトレース情報埋め込み

5.3 節の通り、Artifact の manifest annotations に build 時の `trace_id` / `span_id` を残すか。残す場合のキー命名規約 (`org.ommx.trace.*` 等) と、再ロード時の取り扱い (新しいトレースの `links` として張る? 単なる属性として表示?)。

### Q8. OTel semantic conventions の採用範囲

`EnvironmentInfo` を OTel `Resource` semantic conventions (`os.*`, `host.*`, `process.*`) に寄せるか、OMMX 独自の `ommx.*` namespace に閉じるか、両方併記か。

### Q9. commit 単位の粒度

「1 manifest = 1 commit」とする時、commit を打つ自然な単位は?

- (i) 各 `Run` 終了で 1 manifest — Git 風で履歴が密
- (ii) 各 `Experiment` 終了で 1 manifest — 現 minto の運用に近い
- (iii) ユーザが明示的に `build()` / `commit()` を呼んだ時点 — 制御は明示的だが粒度判断をユーザに押しつける

基本 (iii) を採用しつつ、`with experiment.run() as run:` の終了で自動 commit する糖衣を提供するかが論点。

### Q10. parent ポインタの単数 / 複数

OCI v1.1 `subject` は単一フィールド。複数 experiment を統合する merge 系の操作を v3 で扱うか:

- 当面は単一 parent で十分とする (linear history のみ)
- 多 parent を annotation 規約 (例: `org.ommx.parents` に複数 digest を列挙) で表現する独自拡張を入れる

### Q11. tag (mutable ref) のユーザ可視性

`experiment:latest`, `experiment:v2` のような tag (Git の branch 相当) を OMMX が API として前面に出すか、内部実装にとどめるか。前面に出すと "branch を切る" 概念をユーザに教える必要がある。

### Q12. Garbage collection

長期運用で古い manifest や未参照 blob が registry に滞留する。対応:

- v3 ではスコープ外、設計上のフックだけ用意
- `ommx artifact gc` 相当のサブコマンドを Phase E で導入
- 完全に外部ツール (registry の管理機能) に委ねる

### Q13. diff / lineage 走査 API

`experiment.history()`, `experiment.diff(other)`, `experiment.parent()` を v3 のユーザ API に含めるか、v3.x 後続マイルストーンに送るか。manifest の layer descriptor 列を比較する diff は実装的には軽量。

## 8. リスクと懸念

- **OCI Distribution の HTTP 周りが最重量**: auth (Bearer / Basic), manifest PUT/GET, blob upload session (cross-repo mount, chunked upload) を網羅すると相応の実装量
- **既存 `.ommx` ファイルの後方互換**: OCI Image Layout 標準を逸脱していなければ影響なしだが、annotations のキー命名等で OMMX 固有の慣習があれば要確認
- **minto ユーザへの影響**: 吸収のタイミング・API 変更の予告期間
- **テスト不足**: Phase A を確実に通過しないと Phase B/C のリグレッション検出が困難
- **OTel TracerProvider の二重登録**: minto も独自に provider を立てている場合、吸収後は `ommx.tracing` の lazy 初期化と衝突しないか要検証 (OTel は provider 差し替えを拒否するため、テスト初期化順が壊れやすい — `test_tracing_magic.py` の冒頭参照)
- **Logger の出力先**: stdout 直書きから OTel 経由に変えると、OTel exporter 未設定環境ではコンソールに何も出ない事故が起こり得る。デフォルトで NoOp ConsoleExporter を載せるか、明示的に opt-in させるかの判断が必要
- **OCI v1.1 `subject` フィールドのレジストリ対応**: 全レジストリが v1.1 manifest と Referrers API を実装しているわけではない (古い ghcr.io 設定や self-hosted の一部)。archive / dir 形式は完全制御できるので影響なし。remote 経由で `subject` push が拒否される registry への fallback 戦略 (annotation でのポリフィル / 系譜情報を諦めて push 続行) を Phase C で要検討

## 9. 参考: 関連ファイル

### OMMX Rust
- `rust/ommx/src/artifact/{artifact,builder,annotations,media_types,config}.rs`
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
