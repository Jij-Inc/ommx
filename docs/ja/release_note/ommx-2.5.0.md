# OMMX Python SDK 2.5.0

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.5.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.5.0)

2026-03-19 リリース。

## 新機能

### `NamedFunction` ([#748](https://github.com/Jij-Inc/ommx/pull/748))

最適化問題に付随する補助関数（コスト、ペナルティ、KPIなど）を追跡するための `NamedFunction` メッセージと対応するPythonクラスを導入しました。関連する `EvaluatedNamedFunction` と `SampledNamedFunction` も追加されています。

名前付き関数は `Instance` に付与でき、`Instance.evaluate` 呼び出し時に自動的に評価されて結果が `Solution` に格納されます。`Solution.named_functions_df` を通じてpandas `DataFrame` へのエクスポートにも対応しています。

この機能は以下の用途に有用です：
- 複数の目的関数成分の追跡（例：コストとペナルティの内訳）
- 解とともにドメイン固有のメトリクスを記録
- 異なるソルバー実行間での補助量の比較

### バグ修正: `extract_decision_variables` がパラメータを無視するように変更 ([#745](https://github.com/Jij-Inc/ommx/pull/745))

`extract_decision_variables` が変数の識別にパラメータではなく添字のみを使用するように変更しました。従来は同じ添字でもパラメータが異なる変数があると抽出に失敗していました。問題インスタンス間でパラメータが変化しても添字が安定している実用的なケースでの修正です。
