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

## パッチリリース

### 2.3.1

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.3.1-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.3.1)

- アダプターでのタイムリミット例外の改善 ([#688](https://github.com/Jij-Inc/ommx/pull/688))
- 論理メモリプロファイラー ([#683](https://github.com/Jij-Inc/ommx/pull/683))

### 2.3.2

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.3.2-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.3.2)

- PySCIPOpt依存関係の更新 ([#691](https://github.com/Jij-Inc/ommx/pull/691))
- 量子アダプターの追加 ([#690](https://github.com/Jij-Inc/ommx/pull/690))

### 2.3.3

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.3.3-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.3.3)

- `log_encode` で `used_decision_variables` のみを使用 ([#696](https://github.com/Jij-Inc/ommx/pull/696))

### 2.3.4

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.3.4-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.3.4)

- タイムリミットテストの更新 ([#695](https://github.com/Jij-Inc/ommx/pull/695))
- OMMXOpenJijAdapter が Python 3.13 に対応 ([#704](https://github.com/Jij-Inc/ommx/pull/704))

### 2.3.5

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.3.5-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.3.5)

- 修正: 定数が非ゼロの場合の `Function().terms` メソッド ([#714](https://github.com/Jij-Inc/ommx/pull/714))

### 2.3.6

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.3.6-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.3.6)

- Pyodideビルドを maturin から cibuildwheel に移行 ([#708](https://github.com/Jij-Inc/ommx/pull/708))
- Python MIP 1.17 の使用 ([#724](https://github.com/Jij-Inc/ommx/pull/724))
- 週次Python依存関係更新ワークフローの追加 ([#728](https://github.com/Jij-Inc/ommx/pull/728))
