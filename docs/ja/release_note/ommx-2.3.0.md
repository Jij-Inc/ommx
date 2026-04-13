# OMMX Python SDK 2.3.0

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
