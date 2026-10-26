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

Instanceが保持する変数の一覧は、そのままBackend変数の一覧にはなりません。変換前の表現や評価のために保持している変数もあります。この区別が初見なら、[Instanceの基本モデル](../user_guide/instance.md)を先に確認してください。

## Adapterが実装する契約

- `INPUT_CLASS`を必ず宣言する。これはAdapterがPreparationなしで直接受け取るInstanceの集合であり、applicabilityの完全な条件でもあります。
- `check_applicability()`と`require_applicable()`は`INPUT_CLASS` membershipだけを報告・強制する。converterやBackendが返すerrorを追加のapplicability条件や{class}`~ommx.adapter.AdapterNotApplicableError`として扱いません。
- Solver Adapterは{meth}`~ommx.adapter.SolverAdapter.solve_without_preparation`、Sampler Adapterは{meth}`~ommx.adapter.SamplerAdapter.sample_without_preparation`を実装する。これらは入力をPreparationせず、non-memberを拒否する厳格な経路です。
- 通常の`solve()`と`sample()`は入力のcopyへ{meth}`~ommx.adapter.SolverAdapter.recommended_preparation_policy`を適用してから厳格な経路を呼びます。Backend固有の引数のためにこれらをoverrideする場合も、このcopy、Preparation、strict methodという順序を保ちます。
- Adapter constructorもexact inputだけを受け取り、Preparationしません。
- OMMX由来の独立変数としてBackendへencodeするのは、{attr}`~ommx.Instance.used_decision_variables`に限定する。Backend独自の補助変数を追加することはできます。OMMX IDとBackend変数の対応を保持し、すべてのused IDをdecodeしてください。fixed、dependent、irrelevant、removed-constraint-only、named-function-only、`output_objective`-onlyの変数は独立したBackend入力ではありません。
- decodeしたstateまたはsamplesは、Backend入力の作成に使ったexact Instanceと数学的に同じ、作成後に変更していないsnapshotで評価する。Adapter内にprivateなcopyを保持するか、encodeからBackend実行、decode完了まで入力を変更しない契約を設けてください。そのInstanceがdependent variableの復元とremoved constraintの評価を所有します。

Backendが報告した最適性をSolutionへ付与するときは、評価に使ったInstanceの{meth}`~ommx.Instance.map_active_optimality`を通してください。変換後の問題の最適性と、返す目的値に対する最適性の区別は[Removed constraintsと実行可能性](../user_guide/removed_constraints.md)を参照してください。

## 参照実装

以下の参照実装は、本ページで確認した基準revision `dc533e65be3520137f202538023ab18cbbe631ce` に固定しています。公開済みSDKに対してAdapterを実装する場合は、対象releaseのソースコードとAPI契約を確認してください。

Solver Adapterは [PySCIPOpt Adapterの実装](https://github.com/Jij-Inc/ommx/blob/dc533e65be3520137f202538023ab18cbbe631ce/python/ommx-pyscipopt-adapter/ommx_pyscipopt_adapter/adapter.py) を参照してください。`INPUT_CLASS`、applicability check、used decision variablesのencode/decode、同じ`Instance`による評価までをまとめて確認できます。この実装はconstructorへ渡されたInstanceを参照として保持するため、呼び出し側がencodeからdecode完了まで変更しないことを前提にします。

Sampler Adapterが必要な場合は、[OpenJij Adapterの実装](https://github.com/Jij-Inc/ommx/blob/dc533e65be3520137f202538023ab18cbbe631ce/python/ommx-openjij-adapter/ommx_openjij_adapter/adapter.py)と[サンプルのdecode処理](https://github.com/Jij-Inc/ommx/blob/dc533e65be3520137f202538023ab18cbbe631ce/python/ommx-openjij-adapter/ommx_openjij_adapter/_decode.py)も参照してください。
