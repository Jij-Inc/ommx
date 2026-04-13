# OMMX Python SDK 2.2.0

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.2.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.2.0)

2025-11-14 リリース。

## 破壊的変更

### `EvaluatedDecisionVariable` の不変条件の緩和 ([#676](https://github.com/Jij-Inc/ommx/pull/676))

従来、`EvaluatedDecisionVariable` の構築時に、割り当てた値が変数の上下限と種類の制約を満たしていることが検証されていました。この制約により、実行不可能な解を表現できませんでした。

このリリースでは構築時の上下限・種類チェックを削除しました。代わりに `Solution.feasible` が制約の充足性**と**決定変数の上下限・種類の準拠の両方をチェックするようになりました。これにより、タイムリミット付きの実行などで得られた実行不可能な解を、構築時にエラーを発生させることなく返すことができます。
