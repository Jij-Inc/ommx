# OMMX Adapterを実装する

このページは、独自のBackendをOMMXへ接続するAdapter実装者向けの入口です。まず、Adapter利用者にも共通する次のモデルを確認してください。

1. [Adapterのexact input（INPUT_CLASS）](../user_guide/adapter_input_class.md)
2. [Instance PreparationとPreparationPolicy](../user_guide/preparation_policy.md)
3. [Removed constraintsと実行可能性](../user_guide/removed_constraints.md)

## 実装時に参照するAPI

- Solver Adapter: {class}`~ommx.adapter.SolverAdapter`
- Sampler Adapter: {class}`~ommx.adapter.SamplerAdapter`
- Backendへ渡す決定変数: {attr}`~ommx.Instance.used_decision_variables`
- Backendの出力を評価するAPI: {meth}`~ommx.Instance.evaluate` / {meth}`~ommx.Instance.evaluate_samples`

## 参照実装

Solver Adapterは [PySCIPOpt Adapterの実装](https://github.com/Jij-Inc/ommx/blob/main/python/ommx-pyscipopt-adapter/ommx_pyscipopt_adapter/adapter.py) を参照してください。`INPUT_CLASS`、applicability check、used decision variablesのencode/decode、同じ`Instance`による評価までをまとめて確認できます。

Sampler Adapterが必要な場合は、[OpenJij Adapterの実装](https://github.com/Jij-Inc/ommx/blob/main/python/ommx-openjij-adapter/ommx_openjij_adapter/adapter.py)と[サンプルのdecode処理](https://github.com/Jij-Inc/ommx/blob/main/python/ommx-openjij-adapter/ommx_openjij_adapter/_decode.py)も参照してください。
