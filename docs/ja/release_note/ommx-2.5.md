# OMMX Python SDK 2.5.x

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.5.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.5.0)
[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.5.1-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.5.1)
[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.5.2-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.5.2)

詳細な変更点は上のGitHub Releaseをご覧ください。以下に主な変更点をまとめます。

## 新機能

### `NamedFunction` (2.5.0, [#748](https://github.com/Jij-Inc/ommx/pull/748))

最適化問題に付随する補助関数（コスト、ペナルティ、KPIなど）を追跡するための `NamedFunction` メッセージと対応するPythonクラスを導入しました。関連する `EvaluatedNamedFunction` と `SampledNamedFunction` も追加されています。

名前付き関数は `Instance` に付与でき、`Instance.evaluate` 呼び出し時に自動的に評価されて結果が `Solution` に格納されます。`Solution.named_functions_df` を通じてpandas `DataFrame` へのエクスポートにも対応しています。

この機能は以下の用途に有用です：
- 複数の目的関数成分の追跡（例：コストとペナルティの内訳）
- 解とともにドメイン固有のメトリクスを記録
- 異なるソルバー実行間での補助量の比較

### 前方互換性のための format_version フィールド (2.5.2, [#835](https://github.com/Jij-Inc/ommx/pull/835))

OMMXの主要な4つのメッセージ (`Instance`, `Solution`, `SampleSet`, `ParametricInstance`) に `format_version` フィールドを追加しました。これにより、将来セマンティックな破壊的変更（たとえばPython SDK v3ではOneHotやSOS1制約が特殊制約として昇格され、v2の `ConstraintHints` として扱われなくなります）を含む新しいフォーマットのデータが、黙って誤って解釈されることを防ぎます。

これはv3リリース前に必要なv2メンテナンスリリースであり、v3で生成されたデータをこのSDKで読み込もうとしたユーザーが、誤ったパース結果ではなく明確なエラーを受け取れるようにするものです。

ポリシーの要点：

- `ommx.v1` の後方互換性は変更ありません。古いSDKで生成されたデータは常に新しいSDKで読み込めます。
- セマンティックな破壊を伴わないproto追加については、引き続きprotobuf標準の前方互換性（未知のフィールドは無視）に依存します。
- セマンティックな破壊的変更では `format_version` を上げます。これはSDKのメジャーバージョンアップデート時にのみ行われる *可能性があります* 。
  この場合新しいSDKで生成されたデータは古いSDKでは読み込めなくなり、SDKの更新を促すエラーが発生します。

## バグ修正

### `extract_decision_variables` がパラメータを無視するように変更 (2.5.0, [#745](https://github.com/Jij-Inc/ommx/pull/745))

`extract_decision_variables` が変数の識別にパラメータではなく添字のみを使用するように変更しました。従来は同じ添字でもパラメータが異なる変数があると抽出に失敗していました。問題インスタンス間でパラメータが変化しても添字が安定している実用的なケースでの修正です。

### 従属変数の評価順序 (2.5.1, [#753](https://github.com/Jij-Inc/ommx/pull/753))

従属変数がID順で評価されていたため、低いIDの変数が高いIDの変数に依存している場合に失敗していました。トポロジカル順で評価するように修正しました。
