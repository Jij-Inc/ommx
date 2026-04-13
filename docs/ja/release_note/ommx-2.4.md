# OMMX Python SDK 2.4.x

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.4.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.4.0)

2026-03-11 リリース。

## 破壊的変更

### 削除された制約に固定・従属変数を含むことを許可 ([#738](https://github.com/Jij-Inc/ommx/pull/738))

従来、削除された制約は暗黙的に固定変数や従属変数のIDを参照しないことが前提でした。このリリースでその制約を撤廃し、削除された制約にもこれらの変数を含むことができるようになりました。制約ヒントはアクティブな制約のみを参照するように更新されます。

## バグ修正

### ペナルティメソッドでの制約ヒントのクリア ([#739](https://github.com/Jij-Inc/ommx/pull/739))

`Instance.penalty_method` と `Instance.uniform_penalty_method` で、制約を `removed_constraints` に移動する際に制約ヒントが正しくクリアされるようになりました。従来は、既に存在しないアクティブな制約を参照する古いヒントが残る場合がありました。

### 制約ヒント破棄ログのレベル変更 ([#740](https://github.com/Jij-Inc/ommx/pull/740))

制約ヒントが破棄される際のログメッセージを `warn` から `debug` に変更し、通常利用時のノイズを削減しました。

## パフォーマンス

### `insert_constraints` 一括メソッド ([#735](https://github.com/Jij-Inc/ommx/pull/735))

複数の制約を一度に挿入する `insert_constraints` メソッドを追加しました。検証用のセットを制約ごとではなく一度だけ構築することで、オーバーヘッドを削減します。大規模問題（例：約75万変数、約15万制約）において、挿入時間が数十分から数秒に短縮されます。
