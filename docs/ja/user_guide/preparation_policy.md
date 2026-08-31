# Instance Preparation と PreparationPolicy

Instance Preparationは、{class}`~ommx.Instance`に既存の変換操作を適用し、targetの{class}`~ommx.InstanceClass`に属する入力へ近づける仕組みです。通常のAdapter APIは入力のcopyに推奨Policyを自動適用し、明示的な{meth}`~ommx.Instance.prepare`は渡されたInstanceをin-placeに変更します。

```{important}
まず`solve()`または`sample()`へ元のInstanceを渡します。Policyをapplication固有に編集する必要がある場合だけ、呼び出し側がcopyを作って明示的にPreparationし、`*_without_preparation()`へ渡します。
```

`INPUT_CLASS`が表すexactな集合とAdapter applicabilityは[Adapterのexact input（INPUT_CLASS）](./adapter_input_class.md)を参照してください。本ページでは、その集合へ向けたPreparationの責任境界を説明します。

## Adapter の推奨と呼び出し側の実行

{meth}`~ommx.adapter.SolverAdapter.recommended_preparation_policy` は、Adapter の exact input へ到達する際に一般的に有用な {class}`~ommx.PreparationPolicy` を返します。

このメソッドは次の契約を持ちます。

- 呼び出すたびに、呼び出し側が編集できる新しい Policy を返す
- 特定の Instance を参照または変更しない
- Preparation を実行しない
- target membershipへの到達を保証しない

`INPUT_CLASS`と推奨Policyは独立した値です。標準のeasy APIは、次の処理をAdapter側で行います。

```python
# sourceは変更されない。内部のcopyだけがPreparationされる。
result = Adapter.solve(source)
```

これは概念的には「`source`のcopyを作る、推奨Policyでprepareする、厳格なmethodへ渡す」という流れです。固定penaltyの大きさなど、Adapterが安全な値を推奨できないapplication固有の選択がある場合は、呼び出し側が次の明示的な経路を使います。

```python
import copy

input_class = Adapter.INPUT_CLASS
policy = Adapter.recommended_preparation_policy()

# 必要なら、application 固有の判断で policy を編集します。
prepared = copy.copy(source)
prepared.prepare(input_class, policy)

# prepare()の成功によりapplicability、つまりmembershipが成立します。
assert input_class.contains(prepared)
result = Adapter.solve_without_preparation(prepared)
```

`prepare()`は渡されたInstanceをin-placeに変更します。通常の`solve()`と`sample()`が作るcopyはこの変更を呼び出し元から隔離します。明示的な経路でsource modelを別のAdapterにも使う場合や変換前後を比較する場合は、自分でcopyしてください。実行可能な例は[Adapter向けにInstanceを準備する](../tutorial/prepare_instance_for_adapter.md)を参照してください。

## PreparationPolicy が選ぶ phase

`PreparationPolicy` の各 field は optional で、未指定の phase は実行されません。デフォルトではすべての phase が無効です。各 field は変換の実装そのものではなく、`Instance` が所有する既存操作のうちどれを呼ぶかを選択します。各操作の入力条件や error は、その owner operation が引き続き所有します。

{meth}`~ommx.Instance.prepare` は、選択された phase を高々1回ずつ次の固定順序で適用します。

| 順序 | Policy field | 役割 |
|---|---|---|
| 1 | `special_constraints` | 選択した active な特殊制約を通常制約へ lowering |
| 2 | `objective` | active objectiveを指定したsenseへ変換 |
| 3 | `integer_slack` | active な通常不等式への Integer slack 導入 |
| 4 | `fixed_penalty` | active な通常制約を固定 weight の penalty として目的関数へ移動 |
| 5 | `integer_encoding` | 使用中の Integer 変数を Binary 変数で encoding |
| 6 | `binary_power_reduction` | activeなBinary変数の冪を縮約 |

各fieldに設定する型とfactoryの全引数は、{class}`~ommx.SpecialConstraintPreparation`、{class}`~ommx.ObjectivePreparation`、{class}`~ommx.IntegerSlackPreparation`、{class}`~ommx.FixedPenaltyPreparation`、{class}`~ommx.IntegerEncodingPreparation`、{class}`~ommx.BinaryPowerPreparation`のAPI Referenceを参照してください。特殊制約loweringの数式と成立条件も、各変換メソッドのAPI Referenceが所有します。特殊制約そのものの意味は[特殊制約型](./special_constraints.md)を参照してください。

固定 penalty の大きさのように、問題の scale や求める解の性質に依存して安全な共通値を決められない選択があります。その値は Adapter の推奨に任せず、application が明示的に決めます。

## Target-directed な実行

Preparation は Policy の全 phase を必ず最後まで実行する batch recipe ではありません。`prepare()` は実行前と各 selected phase の完了後に、Instance 全体について target membership を調べます。target に入った時点で停止するため、それより後の phase は選択されていても実行されません。最初から member なら Instance は変更されません。

成功時のpostcondition、つまりAdapter applicabilityは次の1点です。

```python
assert input_class.contains(instance)
```

converterやBackendは、その後のsolver input構築や実行で別のerrorを返すことがあります。それらは追加のapplicability条件ではありません。

Solver の出力を {class}`~ommx.Solution` や {class}`~ommx.SampleSet` に戻すときも、Adapter へ渡した同じ prepared Instance が評価の owner です。その Instance が保持する decision-variable dependency と removed constraint により、encoding 前の変数値や変換前の制約も評価できます。removed constraint の評価規則は [Removed constraints と実行可能性](./removed_constraints.md) を参照してください。

## 到達できない場合と部分的な変更

すべての configured phase を適用しても target に到達しない場合、{class}`~ommx.PreparationTargetNotReachedError` が送出されます。`report` 属性には、最終状態に対する {class}`~ommx.InstanceClassMembershipReport` が入ります。呼び出し側は report を見て Policy を追加・変更するか、異なる target を選べます。

Preparation全体を囲むglobal transactionはありません。後のphaseやowner operationが失敗しても、それより前に完了した変更はInstanceに残ります。個々のowner operationがどこまで検証してから変更するかは、それぞれのAPI contractに従います。通常の`solve()`と`sample()`ではこの部分的な変更も内部copyだけに残ります。明示的な`prepare()`でsourceを失敗前の状態に保つ必要がある場合は、先にcopyを作ってください。

この failure semantics は Preparation の詳細です。最初に Adapter を利用するときは Tutorial の happy path から始め、必要になった時点で本節と各 owner operation の API Reference を確認してください。
