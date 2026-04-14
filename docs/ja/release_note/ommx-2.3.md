# OMMX Python SDK 2.3.x

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.3.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.3.0)

2025-11-18 リリース。

## 新機能

### Pyodide (WebAssembly) サポート ([#679](https://github.com/Jij-Inc/ommx/pull/679))

[Pyodide](https://pyodide.org/) を通じてブラウザ上でOMMXを実行できるようになりました。ネットワーク依存の機能（OCI Artifactのpush/pull）は `remote-artifact` フィーチャーフラグで分離し、コアSDKを `wasm32-unknown-emscripten` ターゲットにコンパイル可能にしました。

### 制約違反量の計算 ([#680](https://github.com/Jij-Inc/ommx/pull/680))

解における制約違反を定量的に評価する新しいメソッドを追加しました。

- `EvaluatedConstraint.violation` - 単一の制約の違反量を返します（実行可能な場合は0）。
- `Solution.total_violation_l1()` - 全制約の違反量の合計（L1ノルム）。
- `Solution.total_violation_l2()` - 全制約の違反量の二乗和の平方根（L2ノルム）。

準実行可能解の分析やペナルティベースの手法の実装に有用です。

## 新機能 (2.3.1–2.3.6)

### `NoSolutionObtained` 例外 (2.3.1, [#688](https://github.com/Jij-Inc/ommx/pull/688))

ソルバーがタイムアウトして実行可能解を見つけられなかったケースを、`InfeasibleDetected` や `UnboundedDetected` と区別する新しい `ommx.adapter.NoSolutionObtained` 例外を追加しました。PySCIPOpt および Python-MIP アダプターが適切な例外型を送出するように更新されています。

### 論理メモリプロファイラー (2.3.1, [#683](https://github.com/Jij-Inc/ommx/pull/683))

flamegraph互換のfolded-stack形式を出力する論理メモリプロファイリングシステムを導入しました。13以上のOMMX型をカバーし、Pythonから `instance.logical_memory_profile()` でアクセスできます。大規模インスタンスのメモリフットプリントの把握に有用です。

### `log_encode` を `used_decision_variables` に限定 (2.3.3, [#696](https://github.com/Jij-Inc/ommx/pull/696))

`log_encode` が目的関数や制約条件で実際に参照されている決定変数のみに対して変数を作成するようになりました。繰り返し呼び出し時の重複変数作成を防止し、未使用変数が多いインスタンスでのオーバーヘッドを削減します。

### その他の新機能

- (2.3.2) 量子アダプターのドキュメント ([#690](https://github.com/Jij-Inc/ommx/pull/690))
- (2.3.4) OMMXOpenJijAdapter が Python 3.13 に対応 ([#704](https://github.com/Jij-Inc/ommx/pull/704))

## バグ修正 (2.3.1–2.3.6)

### 定数が非ゼロの場合の `Function().terms` (2.3.5, [#714](https://github.com/Jij-Inc/ommx/pull/714))

`Function.terms` が定数項が非ゼロの場合に辞書エントリではなく生の `float` を返していた問題を修正しました。

## その他 (2.3.1–2.3.6)

- (2.3.2) PySCIPOpt依存関係の更新 ([#691](https://github.com/Jij-Inc/ommx/pull/691))
- (2.3.6) Pyodideビルドを maturin から cibuildwheel に移行 ([#708](https://github.com/Jij-Inc/ommx/pull/708))
- (2.3.6) Python MIP 1.17 の使用 ([#724](https://github.com/Jij-Inc/ommx/pull/724))
- (2.3.6) 週次Python依存関係更新ワークフローの追加 ([#728](https://github.com/Jij-Inc/ommx/pull/728))
