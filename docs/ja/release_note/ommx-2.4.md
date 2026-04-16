# OMMX Python SDK 2.4.x

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.4.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.4.0)

詳細な変更点は上のGitHub Releaseをご覧ください。以下に主な変更点をまとめます。

## 破壊的変更

### `removed_constraint` に固定・従属変数を含むことを許可 ([#738](https://github.com/Jij-Inc/ommx/pull/738))

従来、 `removed_constraint` は暗黙的に固定変数や従属変数のIDを参照しないことが前提でした。このリリースでその制約を撤廃し、 `removed_constraint` にこれらの変数を含むことができるようになりました。これに併せて `partial_evaluate` が `removed_constraint` をスキップするようになり、使っていない制約に由来する性能の低下を防止します。これらは `restore_constraint` を使って制約を復元する際に部分評価が実行されます。

## バグ修正

### ペナルティメソッドでの制約ヒントのクリア ([#739](https://github.com/Jij-Inc/ommx/pull/739))

`Instance.penalty_method` と `Instance.uniform_penalty_method` で、制約を `removed_constraints` に移動する際に制約ヒントが正しくクリアされるようになりました。従来は、既に存在しないアクティブな制約を参照する古いヒントが残る場合がありました。

### 制約ヒント破棄ログのレベル変更 ([#740](https://github.com/Jij-Inc/ommx/pull/740))

制約ヒントが破棄される際のログメッセージを `warn` から `debug` に変更し、通常利用時のノイズを削減しました。
