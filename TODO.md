# WASM Emscripten Support TODO

## 目標
OMMX Rust SDK を pyodide (wasm32-unknown-emscripten) で使えるようにする。
ネットワーク機能（push/pull）のみを無効化し、Artifact の単一ファイル出力（OciArchive）は維持する。

## 完了済み ✅

### 1. インフラストラクチャ設定
- [x] Taskfile に `task rust:check:wasm32-emscripten` コマンド追加
- [x] GitHub Actions に wasm-emscripten チェックジョブ追加
- [x] コミット済み (076028ab)

### 2. ocipkg に remote feature 追加
- [x] Cargo.toml: `ureq` を optional dependency に
- [x] `remote` feature 追加（デフォルトで有効）
- [x] distribution モジュールの条件付きコンパイル
- [x] image/remote.rs の条件付きコンパイル
- [x] テスト: `cargo check -p ocipkg --no-default-features` 成功
- [x] テスト: `cargo +nightly check -p ocipkg --target wasm32-unknown-emscripten --no-default-features` 成功 ✅
- [x] GitHub Actions に wasm-emscripten チェック追加
- [x] コミット済み (74f9541, 60750b1) - branch: optional-remote-feature

### 3. ommx の oci-spec 0.7.1 対応
- [x] workspace Cargo.toml で ocipkg を path dependency に変更
- [x] `Digest::new()` → `digest.parse()` に修正
- [x] Digest 比較の修正
- [x] テスト: `cargo check -p ommx` 成功
- [x] コミット済み (72b1e3f5, 7dcbf007)

### 4. ommx に remote-artifact feature 追加
- [x] Cargo.toml に `remote-artifact` feature を追加（デフォルトで有効）
- [x] Remote, RemoteBuilder を条件付き import に変更
- [x] `impl Artifact<OciArchive>` の `push()` メソッドを条件付きに
- [x] `impl Artifact<OciDir>` の `push()` メソッドを条件付きに
- [x] `impl Artifact<Remote>` 全体を条件付きに (from_remote/pull)
- [x] `auth_from_env()` を条件付きに
- [x] dataset モジュールを条件付きに（remote 前提のため）
- [x] ommx CLI バイナリを条件付きに（remote 前提のため）
- [x] テスト: `cargo check -p ommx` 成功
- [x] テスト: `cargo +nightly check -p ommx --target wasm32-unknown-emscripten --no-default-features` 成功 ✅
- [x] コミット済み (84064df3)

## 未着手 📋

### 5. テストと検証
- [x] 通常ビルドのテスト: `cargo check -p ommx` ✅
- [x] wasm ビルドのテスト: `cargo +nightly check -p ommx --target wasm32-unknown-emscripten --no-default-features` ✅
- [ ] ユニットテストが通ることを確認: `task rust:test`
- [ ] Python SDK のビルドが通ることを確認

### 6. ドキュメント更新
- [ ] CLAUDE.md に wasm サポートについて記載
- [ ] Cargo.toml の features について README に記載
- [ ] ocipkg の remote feature について説明

### 7. コミットとマージ
- [x] ocipkg の変更をコミット (74f9541, 60750b1) ✅
- [x] ommx の変更をコミット (84064df3) ✅
- [ ] PR 作成
- [ ] ocipkg の upstream への貢献を検討
  - termoshtt/ocipkg に PR を出す
  - バージョン 0.4.0 以降で remote feature が利用可能になったら、path dependency を削除

## 技術的な課題

### ✅ ocipkg の remote feature を無効化する方法（解決済み）
**問題**: ommx が `ocipkg = { workspace = true, features = ["remote"] }` と指定しているため、
wasm ビルド時も remote feature が有効になってしまう。

**採用した解決策**:
- ommx 側で `remote-artifact` feature を追加: `default = ["remote-artifact"]`, `remote-artifact = ["ocipkg/remote"]`
- wasm ビルド時は `--no-default-features` を使用することで remote 機能を無効化
- この方法により、デフォルトでは remote 機能が有効、明示的に無効化も可能

## メモ

- ocipkg のバージョン: 0.4.0 (path dependency)
- oci-spec のバージョン: 0.7.1
- Emscripten SDK セットアップ: mymindstorm/setup-emsdk@v14
