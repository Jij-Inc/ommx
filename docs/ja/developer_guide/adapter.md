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

## Adapterが実装する契約

- `INPUT_CLASS`を必ず宣言する。これはAdapterがPreparationなしで直接受け取るInstanceの集合であり、applicabilityの完全な条件でもあります。
- `check_applicability()`と`require_applicable()`は`INPUT_CLASS` membershipだけを報告・強制する。converterやBackendが返すerrorを追加のapplicability条件や{class}`~ommx.adapter.AdapterNotApplicableError`として扱いません。
- Solver Adapterは{meth}`~ommx.adapter.SolverAdapter.solve_without_preparation`、Sampler Adapterは{meth}`~ommx.adapter.SamplerAdapter.sample_without_preparation`を実装する。これらは入力をPreparationせず、non-memberを拒否する厳格な経路です。
- 通常の`solve()`と`sample()`は入力のcopyへ{meth}`~ommx.adapter.SolverAdapter.recommended_preparation_policy`を適用してから厳格な経路を呼びます。Backend固有の引数のためにこれらをoverrideする場合も、このcopy、Preparation、strict methodという順序を保ちます。
- Adapter constructorもexact inputだけを受け取り、Preparationしません。
- Backendには{attr}`~ommx.Instance.used_decision_variables`だけをencodeし、OMMX IDとBackend変数の対応を保持して、すべてのused IDをdecodeする。fixed、dependent、irrelevant、removed-constraint-only、named-function-only、`output_objective`-onlyの変数は独立したBackend入力ではありません。
- decodeしたstateまたはsamplesは、Backendへ渡した同じexact Instanceで評価する。そのInstanceがdependent variableの復元とremoved constraintの評価を所有します。

## 参照実装

Solver Adapterは [PySCIPOpt Adapterの実装](https://github.com/Jij-Inc/ommx/blob/main/python/ommx-pyscipopt-adapter/ommx_pyscipopt_adapter/adapter.py) を参照してください。`INPUT_CLASS`、applicability check、used decision variablesのencode/decode、同じ`Instance`による評価までをまとめて確認できます。

Sampler Adapterが必要な場合は、[OpenJij Adapterの実装](https://github.com/Jij-Inc/ommx/blob/main/python/ommx-openjij-adapter/ommx_openjij_adapter/adapter.py)と[サンプルのdecode処理](https://github.com/Jij-Inc/ommx/blob/main/python/ommx-openjij-adapter/ommx_openjij_adapter/_decode.py)も参照してください。
