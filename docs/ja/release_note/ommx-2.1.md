# OMMX Python SDK 2.1.x

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.1.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.1.0)

2025-11-13 リリース。

## 破壊的変更

### Python 3.9 のサポート終了、Python 3.10-3.14 対応 ([#669](https://github.com/Jij-Inc/ommx/pull/669))

Python 3.9 がEnd of Lifeを迎えたため、サポートを終了しました。PyO3 ABI3のベースラインを `py39` から `py310` に引き上げ、Python 3.10 (ABI3)、3.13t、3.14t (free-threaded) 向けのwheelを提供します。

## 新機能

### evaluate メソッドに `atol` パラメータを追加 ([#666](https://github.com/Jij-Inc/ommx/pull/666))

`Instance.evaluate`、`Function.evaluate`、`Constraint.evaluate` など全てのevaluateメソッドに、オプションのキーワード引数 `atol` を追加しました。実行可能性チェックの絶対許容誤差をカスタマイズできます。デフォルト値は従来通り `1e-6` です。

### `decision_variable_names` と `extract_all_decision_variables` ([#667](https://github.com/Jij-Inc/ommx/pull/667))

- `Instance`、`Solution`、`SampleSet` に `decision_variable_names` プロパティを追加。全決定変数名のセットを返します。
- `extract_all_decision_variables()` メソッドを追加。変数名から添字と値のマッピングへの辞書を返します。既存の `extract_decision_variables(name)` メソッドを補完します。

### `DecisionVariableAnalysis` の Display とシリアライズ対応 ([#668](https://github.com/Jij-Inc/ommx/pull/668))

`substitute_acyclic` で作られた従属変数の識別など、決定変数の種類・用途別の分類を提供する `DecisionVariableAnalysis` に、Python側で `to_dict()` と `__repr__()` を、Rust側で `Display` トレイトと `Serialize`/`Deserialize` を追加しました。
