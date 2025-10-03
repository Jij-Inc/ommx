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

### Phase 4: 古いArtifact<T>の削除と移行

- [ ] 既存の`Artifact<T>`実装を削除
  - [ ] `impl Artifact<OciArchive>`
  - [ ] `impl Artifact<OciDir>`
  - [ ] `impl Artifact<Remote>`
  - [ ] `impl Artifact<Base>`（汎用実装）

- [ ] 新しい`Artifact`に移行
  - [ ] 既存のメソッドを新しいenumベース実装に移植
  - [ ] ローカルレジストリ関連のutil関数を一箇所に整理し、公開APIの命名・役割を明確化

### Phase 5: Python側のAPI整理

- [ ] `python/ommx/src/artifact.rs`の更新
  - [ ] 新しい`Artifact`のPyO3バインディングを追加
  - [ ] `ArtifactArchive`、`ArtifactDir`のPython公開を削除
  - [ ] 既存の`load_archive`/`load_dir`の`#[pyfunction]`を現状維持（内部実装差し替えのみ、警告は発しない）

- [ ] `python/ommx/ommx/artifact.py`の更新
  - [ ] `ArtifactArchive`、`ArtifactDir`クラスを削除
  - [ ] `Artifact`クラスを新しいRust `Artifact`の薄いラッパーに変更
  - [ ] `Artifact.load_archive`/`load_dir`は現行どおり提供（挙動は維持、警告は発しない）
  - [ ] `__all__`から`ArtifactArchive`、`ArtifactDir`を削除

- [ ] `python/ommx/ommx/__init__.py`の更新
  - [ ] エクスポートリストを確認・更新

### Phase 6: CLIツールの更新

- [ ] `rust/ommx/src/bin/ommx.rs`の更新
  - [ ] `ImageNameOrPath`の実装を簡略化
  - [ ] 新しい`Artifact` APIを使用
  - [ ] `load`コマンドをoci-archive形式デフォルトに変更（必要に応じて`--format dir`オプションを追加し警告メッセージで通知）
  - [ ] 既存の`load --dir`など旧オプションがある場合は非推奨警告を表示

### Phase 7: テストとドキュメントの更新

- [ ] Rustのテストを更新
  - [ ] 新しい`Artifact`のテストを追加
  - [ ] 既存のテストを新しいAPIに合わせて修正
  - [ ] `artifact.rs`内のテストを確認
  - [ ] ローカルレジストリ内にarchive/dir両方が存在するケース、片方のみ存在するケース、破損アーカイブにフォールバックするケースを網羅
  - [ ] `Remote` variantが`pull()`後に`Archive`へ遷移することを検証

- [ ] Pythonのテストを更新
  - [ ] `test_artifact_dual_format.py`を新しいAPIに合わせて修正
  - [ ] `ArtifactArchive`/`ArtifactDir`への直接参照を削除
  - [ ] 他のテストファイルで影響を受けるものを修正
  - [ ] `Artifact.load_archive`/`load_dir`が現行の期待どおりに動作することを検証

- [ ] ドキュメントの更新
  - [ ] `ARTIFACT.md`に新しいAPI設計を記載
  - [ ] 移行ガイドを追加（破壊的変更のため）
  - [ ] Rustdocを更新
  - [ ] CLIヘルプとユーザーガイドに`--format`オプションおよびデフォルト変更を明記
  - [ ] Python API互換性ポリシー（現行関数の継続提供）をドキュメント化し、サンプルコードを更新

### Phase 8: 最終確認

- [ ] 非推奨化の記録
  - [ ] `get_image_dir()`は既に非推奨（2.1.0）
  - [ ] 古い`Artifact<T>`型は削除（破壊的変更）

- [ ] 破壊的変更の記録
  - [ ] CHANGELOG.mdに記載
  - [ ] Python側の`ArtifactArchive`、`ArtifactDir`削除
  - [ ] Rust側の`Artifact<T>`削除

## 破壊的変更のチェックリスト

### Rust API（破壊的変更）
- [ ] `Artifact<T: Image>`型が削除され、新しい`Artifact`型に置き換え
  - 型パラメータがなくなる
  - メソッドシグネチャが変更される
  - `Artifact::from_oci_archive()` / `from_oci_dir()` / `from_remote()` が新しいAPI

- [ ] メソッドの変更
  - `pull()` → `Result<Artifact<OciDir>>` から `Result<()>` に変更（自身を変更）
  - `load()` → 既存の挙動（ローカル探索→未発見ならPull）を維持
  - `save()` → 存続（ローカルレジストリへ既定: oci-archiveで保存）。必要に応じて`save_as_archive(path)`/`save_as_dir(path)`の補助メソッドを提供
  - ローカルレジストリ探索とPullの一貫ロジックは`Artifact::load()`内部で完結させる（追加のutilは作らない）

### Python API（部分的に破壊的）
- [ ] `ArtifactArchive`、`ArtifactDir`が削除される
  - ただし`__all__`に含まれていないので外部への影響は限定的
  - もし使っているコードがあれば`Artifact`に移行が必要

- [ ] `Artifact`の内部実装が変わる
  - 公開APIは可能な限り互換性を保つ
  - `load_archive()`/`load_dir()`は現行仕様で継続提供（非推奨化しない）

### CLI（ユーザー影響あり）
- [ ] `load`コマンドがデフォルトでoci-archive形式で保存
  - 既存のoci-dir形式のアーティファクトは引き続き読み込み可能
  - 新しく保存する場合は`.ommx`ファイルになる
  - CLIヘルプと`--format dir`などの明示オプションで変更点を通知し、実行時にも警告メッセージを表示

## リスクと対策

1. **リスク**: 既存のoci-dir形式のアーティファクトが読み込めなくなる
   - **対策**: `get_local_registry_path()`の結果に基づきarchive/dir双方を検知し、フォールバックロジックと整合性チェックを実装

2. **リスク**: Python側の既存コードが動かなくなる
   - **対策**: 互換レイヤーではなく現行APIの継続提供により互換性を維持

3. **リスク**: 大規模な変更でバグが混入する
   - **対策**: 段階的に実装し、各Phaseでテストを実行

4. **リスク**: 段階的移行期間中に互換レイヤーの挙動が二重実装になり管理コストが増加
   - **対策**: フィーチャーフラグや環境変数で旧API経路を有効化できるようにし、移行完了後に削除する計画をドキュメント化

## 完了基準

- [ ] すべてのRustテストがパス
- [ ] すべてのPythonテストがパス
- [ ] CLIツールで両フォーマットの読み書きが正常に動作
- [ ] ドキュメントが更新されている
- [ ] CLIヘルプと実行時警告でデフォルト変更が通知されている
  
