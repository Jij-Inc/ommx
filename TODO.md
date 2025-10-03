# Artifact API Refactoring Plan

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
   pub struct Artifact {
       inner: ArtifactInner,
   }

   enum ArtifactInner {
       Archive(OciArtifact<OciArchive>),
       Dir(OciArtifact<OciDir>),
       Remote(OciArtifact<Remote>),
   }
   ```

2. **統一されたAPI**
   - `Artifact::from_oci_archive(path)` → oci-archiveから読み込み
   - `Artifact::from_oci_dir(path)` → oci-dirから読み込み
   - `Artifact::from_remote(image_name)` → リモートから読み込み
   - `Artifact::load(image_name)` → ローカルレジストリから両フォーマットをチェックして読み込み
   - `Artifact::from_path(path)` → パスからフォーマットを判定して読み込み

3. **保存メソッド**
   - `artifact.save_as_archive(path)` → oci-archive形式で保存
   - `artifact.save_as_dir(path)` → oci-dir形式で保存
   - `artifact.load_to_local_registry()` → デフォルトでoci-archive形式でローカルレジストリに保存
   - `artifact.pull()` → リモートからpullしてデフォルトでoci-archive形式で保存

### Python側

1. **内部型の非公開化**
   - `ArtifactArchive`、`ArtifactDir`をPython APIから削除
   - `_ArtifactArchive`、`_ArtifactDir`は内部実装として残す

2. **統一されたAPI**
   - `Artifact.load(image_name)` → Rust側の`LocalArtifact::load()`を呼ぶ
   - `Artifact.load_archive(path)` → `LocalArtifact::from_path()`を呼ぶ
   - フォーマットの詳細はユーザーから隠蔽

## 実装手順

### Phase 1: 新しいArtifact型の実装

- [ ] `ArtifactInner` enumを追加（非公開）
  - [ ] `Archive(OciArtifact<OciArchive>)`
  - [ ] `Dir(OciArtifact<OciDir>)`
  - [ ] `Remote(OciArtifact<Remote>)`

- [ ] 新しい`Artifact`構造体を実装
  ```rust
  pub struct Artifact {
      inner: ArtifactInner,
  }
  ```

- [ ] 基本メソッドの実装
  - [ ] `image_name() -> Option<&str>`
  - [ ] `annotations() -> &HashMap<String, String>`
  - [ ] `layers() -> &[Descriptor]`
  - [ ] `get_blob(digest: &str) -> Result<Vec<u8>>`
  - [ ] `get_manifest() -> Result<ImageManifest>`

### Phase 2: 読み込みメソッドの実装

- [ ] `Artifact::from_oci_archive(path: &Path) -> Result<Self>`
  - [ ] `OciArtifact::from_oci_archive()`を呼んで`Archive`variantに格納

- [ ] `Artifact::from_oci_dir(path: &Path) -> Result<Self>`
  - [ ] `OciArtifact::from_oci_dir()`を呼んで`Dir`variantに格納

- [ ] `Artifact::from_remote(image_name: ImageName) -> Result<Self>`
  - [ ] `OciArtifact::from_remote()`を呼んで`Remote`variantに格納

- [ ] `Artifact::load(image_name: &ImageName) -> Result<Self>`
  - [ ] `get_local_registry_path()`でベースパスを取得
  - [ ] `.ommx`ファイルが存在 → `from_oci_archive()`
  - [ ] ディレクトリが存在 → `from_oci_dir()`
  - [ ] どちらも存在しない → `from_remote()`でリモートから取得

- [ ] `Artifact::from_path(path: &Path) -> Result<Self>`
  - [ ] パスがファイル → `from_oci_archive()`
  - [ ] パスがディレクトリ → `from_oci_dir()`

### Phase 3: 保存メソッドの実装

- [ ] `save_as_archive(&mut self, path: &Path) -> Result<()>`
  - [ ] 内部のvariantに関わらず、oci-archive形式で保存

- [ ] `save_as_dir(&mut self, path: &Path) -> Result<()>`
  - [ ] 内部のvariantに関わらず、oci-dir形式で保存

- [ ] `load_to_local_registry(&mut self) -> Result<()>`
  - [ ] イメージ名を取得
  - [ ] デフォルトでoci-archive形式でローカルレジストリに保存
  - [ ] 既に存在する場合はスキップ

- [ ] `pull(&mut self) -> Result<()>`
  - [ ] `Remote`の場合のみ動作
  - [ ] デフォルトでoci-archive形式でローカルレジストリに保存

- [ ] `push(&mut self) -> Result<()>`
  - [ ] リモートにpush

### Phase 4: 古いArtifact<T>の削除と移行

- [ ] 既存の`Artifact<T>`実装を削除
  - [ ] `impl Artifact<OciArchive>`
  - [ ] `impl Artifact<OciDir>`
  - [ ] `impl Artifact<Remote>`
  - [ ] `impl Artifact<Base>`（汎用実装）

- [ ] 新しい`Artifact`に移行
  - [ ] `Deref`/`DerefMut`の実装を調整
  - [ ] 既存のメソッドを新しい実装に移植

### Phase 5: Python側のAPI整理

- [ ] `python/ommx/src/artifact.rs`の更新
  - [ ] 新しい`Artifact`のPyO3バインディングを追加
  - [ ] `ArtifactArchive`、`ArtifactDir`のPython公開を削除

- [ ] `python/ommx/ommx/artifact.py`の更新
  - [ ] `ArtifactArchive`、`ArtifactDir`クラスを削除
  - [ ] `Artifact`クラスを新しいRust `Artifact`の薄いラッパーに変更
  - [ ] `__all__`から`ArtifactArchive`、`ArtifactDir`を削除

- [ ] `python/ommx/ommx/__init__.py`の更新
  - [ ] エクスポートリストを確認・更新

### Phase 6: CLIツールの更新

- [ ] `rust/ommx/src/bin/ommx.rs`の更新
  - [ ] `ImageNameOrPath`の実装を簡略化
  - [ ] 新しい`Artifact` APIを使用
  - [ ] `load`コマンドをoci-archive形式デフォルトに変更

### Phase 7: テストとドキュメントの更新

- [ ] Rustのテストを更新
  - [ ] 新しい`Artifact`のテストを追加
  - [ ] 既存のテストを新しいAPIに合わせて修正
  - [ ] `artifact.rs`内のテストを確認

- [ ] Pythonのテストを更新
  - [ ] `test_artifact_dual_format.py`を新しいAPIに合わせて修正
  - [ ] `ArtifactArchive`/`ArtifactDir`への直接参照を削除
  - [ ] 他のテストファイルで影響を受けるものを修正

- [ ] ドキュメントの更新
  - [ ] `ARTIFACT.md`に新しいAPI設計を記載
  - [ ] 移行ガイドを追加（破壊的変更のため）
  - [ ] Rustdocを更新

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
  - `load()` → 削除（`load_to_local_registry()`に置き換え）
  - `save()` → `save_as_archive()` / `save_as_dir()` に分割

### Python API（部分的に破壊的）
- [ ] `ArtifactArchive`、`ArtifactDir`が削除される
  - ただし`__all__`に含まれていないので外部への影響は限定的
  - もし使っているコードがあれば`Artifact`に移行が必要

- [ ] `Artifact`の内部実装が変わる
  - 公開APIは可能な限り互換性を保つ
  - `load_archive()` → `load()` or `from_path()` への移行を推奨

### CLI（ユーザー影響あり）
- [ ] `load`コマンドがデフォルトでoci-archive形式で保存
  - 既存のoci-dir形式のアーティファクトは引き続き読み込み可能
  - 新しく保存する場合は`.ommx`ファイルになる

## リスクと対策

1. **リスク**: 既存のoci-dir形式のアーティファクトが読み込めなくなる
   - **対策**: `LocalArtifact::load()`で両フォーマットをサポート

2. **リスク**: Python側の既存コードが動かなくなる
   - **対策**: `Artifact`クラスのAPIは互換性を保つ、内部実装のみ変更

3. **リスク**: 大規模な変更でバグが混入する
   - **対策**: 段階的に実装し、各Phaseでテストを実行

## 完了基準

- [ ] すべてのRustテストがパス
- [ ] すべてのPythonテストがパス
- [ ] CLIツールで両フォーマットの読み書きが正常に動作
- [ ] ドキュメントが更新されている
- [ ] 非推奨警告が適切に表示される
