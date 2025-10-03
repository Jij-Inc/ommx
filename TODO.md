# Artifact API Refactoring Plan

## 実装方針の変更

新しいArtifact実装は**experimental API**として`ommx::experimental::artifact`モジュールに作成します。
既存の`ommx::artifact`モジュールは当面そのまま維持し、experimental APIが安定したら移行します。

### 現在の実装状況

- ✅ `ommx::experimental`モジュールを作成
- ✅ `ommx::experimental::artifact::Artifact` enumを実装
- ✅ 基本メソッド（`image_name`, `get_manifest`, `get_blob`, `get_layer`, `get_config`など）を実装
- ⏳ Phase 2以降の実装（読み込み・保存メソッド）

## PR #639の背景

このPRは、OMMX Local Registryに**oci-dir**（ディレクトリ）と**oci-archive**（単一ファイル）の両フォーマットのサポートを追加しました。新しいアーティファクトはデフォルトでoci-archive形式を使用し、AWS S3やGoogle Cloud Storageなどのクラウドオブジェクトストレージとの互換性を向上させます。

### 元の問題（Issue #638）

以前、OMMX Local Registryはoci-dir形式のみをサポートしており、アーティファクトをディレクトリ構造として保存していました。これはローカル開発では問題ありませんが、クラウドオブジェクトストレージでは以下の課題がありました：

- ディレクトリ構造のアップロード/ダウンロードに複数のAPI呼び出しが必要
- 複雑な同期と整合性の問題
- 多数の小さなファイル操作によるコスト増加
- アトミック操作とバックアップの困難さ

### PR #639の解決策

両方のストレージ形式をサポートする統一されたAPIを追加：

- **OCI Archive形式（新しいデフォルト）**: 単一の`.ommx`ファイル（tarアーカイブ）として保存
- **OCI Directory形式（レガシーサポート）**: 既存のディレクトリベースのストレージを維持

### PR #639で実装された内容

- `get_local_registry_path()` - 単一のフォーマット非依存パス生成関数
- `Artifact.load()` - 両フォーマットの自動検出と読み込み
- `ArtifactBuilder.new()` - デフォルトでoci-archive形式を使用
- CLIツールの両フォーマット対応
- 後方互換性の完全維持

## 残された問題

現在のdual format対応により、local registryはoci-dirとoci-archiveの両フォーマットをサポートするようになりました。しかし、既存のAPIが古い設計（oci-dirのみ）のままで、以下の問題があります：

### 現状の問題点

1. **型パラメータと実際の動作の不一致**
   - 新規作成はoci-archiveがデフォルトなのに、`load()`/`pull()`は常にoci-dir形式で保存
   - `Artifact<OciArchive>`の型パラメータの意味が曖昧

2. **存在チェックと読み込みの不整合**
   - `load()`/`pull()`がarchive形式の存在をチェックするが、dir形式としてしか読み込めない
   - archive形式のみ存在する場合に失敗する

3. **Python側のAPI設計**
   - `ArtifactArchive`と`ArtifactDir`がPython側に露出しているが、実際には使われていない
   - Python側では`Artifact`クラスが内部で動的にフォーマットを切り替えている

## 新しい設計方針

### Rust側

1. **`Artifact`を動的な型に変更**
   - 現在の`Artifact<T: Image>`を廃止
   - 内部でフォーマットを動的に管理する新しい`Artifact`型を導入
   ```rust
   pub enum Artifact {
       Archive(OciArtifact<OciArchive>),
       Dir(OciArtifact<OciDir>),
       Remote(OciArtifact<Remote>),
   }
   ```
   - `Remote` variantは`pull()`完了後に`Archive`または`Dir` variantへ置き換える。状態遷移ルールを明文化し、インスタンスが常に一貫したローカル/リモート状態を持つようにする。
   - ローカルレジストリへのパス解決は既存の`get_local_registry_*`関数群（stateless util）を整理・拡張して担わせ、`Artifact` enum本体はIOを直接抱えない。

2. **統一されたAPI**
   - `Artifact::from_oci_archive(path)` → oci-archiveから読み込み
   - `Artifact::from_oci_dir(path)` → oci-dirから読み込み
   - `Artifact::from_remote(image_name)` → リモートから読み込み
   - `Artifact::load(image_name)` → ローカルレジストリを探索し、なければリモートからPullしてローカル（既定: oci-archive）へ保存して返す（内部で`get_local_registry_*` utilを使用）
   - （任意）`Artifact::from_path(path)` → パスからフォーマットを判定して読み込み（`.ommx`優先、両方存在時はArchive優先）。現状APIには存在しないため、追加する場合の候補として扱う
   - ローカルレジストリのベースパス解決は既存の`get_local_registry_path()`を唯一のユーティリティとして採用し、追加の新規utilは作らない

3. **保存メソッド**
   - `artifact.save_as_archive(path)` → oci-archive形式で保存（variantを必要に応じて`Archive`へ再構築）
   - `artifact.save_as_dir(path)` → oci-dir形式で保存（variantを必要に応じて`Dir`へ再構築）
   - `artifact.save()` → デフォルトでoci-archive形式でローカルレジストリに保存
   - `artifact.pull()` → リモートからpullしてデフォルトでoci-archive形式で保存し、`Remote` variantを`Archive`variantへ置き換え

### Python側

1. **内部型の非公開化**
   - `ArtifactArchive`、`ArtifactDir`をPython APIから削除（Rust側でも廃止）
   - 既存の`Artifact`/`ArtifactBuilder`の公開メソッドとシグネチャは維持し、利用側の互換性を確保
   - `ArtifactArchive`/`ArtifactDir`は外部から直接使われておらず、廃止による影響は極小と判断

2. **統一されたAPI**
   - `Artifact.load(image_name)` → Rust側の`Artifact::load()`を呼び、ローカル探索→未発見ならPullの挙動を維持
   - `Artifact.load_archive(path)`/`Artifact.load_dir(path)` → 現行どおりPath指定で読み込み
   - `Artifact.load_archive()`/`Artifact.load_dir()`は現行の挙動を維持し、引き続きサポート（非推奨化しない）
   - ドキュメントとdocstringに使用例（例：`artifact = Artifact.load_archive("model.ommx")`）を明記

## 実装手順

### Phase 1: 新しいArtifact型の実装

- [x] 新しい`Artifact` enumを実装
  ```rust
  pub enum Artifact {
      Archive(OciArtifact<OciArchive>),
      Dir(OciArtifact<OciDir>),
      Remote(OciArtifact<Remote>),
  }
  ```

- [x] 基本メソッドの実装
  - [x] `image_name() -> Option<String>`
  - [x] `annotations() -> Result<HashMap<String, String>>`
  - [x] `layers() -> Result<Vec<Descriptor>>`
  - [x] `get_blob(digest: &Digest) -> Result<Vec<u8>>`
  - [x] `get_manifest() -> Result<ImageManifest>`
  - [x] 追加実装: `get_layer()`, `get_config()`, `get_solution()`, `get_instance()`, `get_parametric_instance()`, `get_sample_set()`, `get_solutions()`, `get_instances()`

### Phase 2: 読み込みメソッドの実装

- [x] `Artifact::from_oci_archive(path: &Path) -> Result<Self>`
  - [x] `OciArtifact::from_oci_archive()`を呼んで`Archive`variantに格納

- [x] `Artifact::from_oci_dir(path: &Path) -> Result<Self>`
  - [x] `OciArtifact::from_oci_dir()`を呼んで`Dir`variantに格納

- [x] `Artifact::from_remote(image_name: ImageName) -> Result<Self>`
  - [x] `OciArtifact::from_remote()`を呼んで`Remote`variantに格納

- [x] `Artifact::load(image_name: &ImageName) -> Result<Self>`
  - [x] `get_local_registry_path()`でベースパスを取得し、`.ommx`（archive）/ディレクトリ（dir）の存在を判定
  - [x] `.ommx`とディレクトリが両方ある場合はArchiveを優先、破損検知時はDirへフォールバック
  - [x] どちらも存在しない → `from_remote()`で取得してローカルレジストリへoci-archiveで保存し、`Archive` variantで返す

- （任意）[ ] `Artifact::from_path(path: &Path) -> Result<Self>`（将来的な追加候補）
  - [ ] パスがファイル → `from_oci_archive()`
  - [ ] パスがディレクトリ → `from_oci_dir()`
  - [ ] `.ommx`とディレクトリが同居する場合の選択ロジックを共通化

- [x] 既存のローカルレジストリ関連ユーティリティは`get_local_registry_path()`を中核としてそのまま再利用し、名称変更や新設は行わない

### Phase 3: 保存メソッドの実装

- [x] `save_as_archive(&mut self, path: &Path) -> Result<()>`
  - [x] 内部のvariantに関わらず、oci-archive形式で保存
  - [x] 保存後に`Archive` variantへ自身を更新

- [x] `save_as_dir(&mut self, path: &Path) -> Result<()>`
  - [x] 内部のvariantに関わらず、oci-dir形式で保存
  - [x] 保存後に`Dir` variantへ自身を更新

- [x] `save(&mut self) -> Result<()>`
  - [x] イメージ名を取得
  - [x] デフォルトでoci-archive形式でローカルレジストリに保存
  - [x] 既に存在する場合はスキップ

- [x] `pull(&mut self) -> Result<()>`
  - [x] `Remote`の場合のみ動作
  - [x] デフォルトでoci-archive形式でローカルレジストリに保存し、`Remote` variantを`Archive`variantへ更新

- [x] `push(&mut self) -> Result<()>`
  - [x] リモートにpush
  - [x] Basic認証のサポート

- [x] `Builder` enumの実装
  - [x] Archive/Dir variantsをサポート
  - [x] `add_instance()`, `add_solution()`, `add_parametric_instance()`, `add_sample_set()`, `add_config()`
  - [x] `add_annotation()` - manifest annotationsを設定
  - [x] `build()` - `Artifact`を生成

- [x] テストの実装
  - [x] 13個のテストケース (全てパス)
  - [x] load/save/format変換/annotation設定のテスト

### Phase 4: experimental::artifact への段階的移行

**方針変更**: `ommx::artifact::Artifact<T>`は当面削除せず、使用箇所を段階的に`ommx::experimental::artifact::Artifact`に置き換える。

- [x] Rust内部コードの移行
  - [x] CLIツール (`rust/ommx/src/bin/ommx.rs`) を experimental::artifact に移行
  - [x] 全てのRustテストがパス（313テスト）
  - [ ] 既存のテストコードを確認し、必要に応じて experimental 版を使用
  - [ ] 他のRustコードで `ommx::artifact::Artifact<T>` を使用している箇所を調査

- [ ] 移行が完了した段階で評価
  - [ ] experimental APIの安定性を確認
  - [ ] パフォーマンスやエラーハンドリングに問題がないか検証
  - [ ] 必要に応じて experimental から正式版に昇格 (`ommx::artifact` として公開)

### Phase 5: Python側のAPI整理（experimental API使用）

- [x] `python/ommx/src/artifact.rs`の更新
  - [x] experimental::artifact のPyO3バインディングを追加（`PyArtifact`, `PyArtifactBuilder`）
  - [x] 既存のPython APIは内部実装のみ experimental 版に差し替え
  - [x] 古い`ArtifactArchive`、`ArtifactDir`、`ArtifactArchiveBuilder`、`ArtifactDirBuilder`を削除
  - [x] `builder.rs`ファイルを削除
  - [x] `PyArtifactBuilder`を`Option<Builder>`で実装（所有権管理）
  - [x] `PyArtifact`は`Mutex<ExperimentalArtifact>`（Syncトレイト要件のため）
  - [x] `ArtifactBuilder.new_dir()`メソッドを追加してoci-dir形式をサポート

- [x] Pythonテストの確認
  - [x] 既存のテストが全てパスすることを確認（93 unit + 1 doctest + 37 benchmark + 全アダプター）
  - [x] experimental 版への移行による動作変更がないことを検証
  - [x] stubgen実行とtype stub更新

### Phase 6: ドキュメントとテストの更新

- [ ] Rustdocの更新
  - [ ] experimental::artifact のドキュメントを充実
  - [ ] 使用例とマイグレーションガイドを追加

- [ ] 統合テストの追加
  - [ ] CLI経由での動作確認テスト
  - [ ] Python API経由での動作確認テスト

### Phase 7: 正式版への昇格（将来）

experimental APIが十分に安定したら以下を実施:

- [ ] `ommx::artifact` を `ommx::artifact_legacy` に移動
- [ ] `ommx::experimental::artifact` を `ommx::artifact` として公開
- [ ] 非推奨警告の追加
- [ ] CHANGELOGの更新

## 移行のリスクと対策

**方針変更により破壊的変更は最小化**:

1. **リスク**: experimental APIの安定性が不十分
   - **対策**: 段階的移行により十分な検証期間を確保
   - **対策**: 既存APIは残すので、問題があれば切り戻し可能

2. **リスク**: Python側の既存コードへの影響
   - **対策**: 内部実装のみ差し替え、外部APIは完全互換を維持
   - **対策**: 既存テストが全てパスすることを確認

3. **リスク**: CLIツールの動作変更
   - **対策**: experimental版への移行は慎重に実施
   - **対策**: 既存の動作を保持しつつ、新しい機能を追加

## Phase 4 & 5の完了基準

- [x] CLIツールが experimental::artifact を使用
- [x] 全てのRustテストがパス（313テスト）
- [x] 全てのPythonテストがパス（93 unit + 1 doctest + 37 benchmark + 全アダプター）
- [x] 既存の機能に変更がないことを確認
- [x] experimental APIの動作が安定していることを確認
  
