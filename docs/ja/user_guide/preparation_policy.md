# Instance Preparation と PreparationPolicy

Instance Preparation は、呼び出し側が選んだ {class}`~ommx.Instance` に既存の変換操作を適用し、target の {class}`~ommx.InstanceClass` に属する入力へ近づける仕組みです。Adapter が行う暗黙の前処理ではありません。

```{important}
Adapter は推奨 Policy を返します。どの Instance を変更するか、Policy をどう編集するか、そして {meth}`~ommx.Instance.prepare` を実行するかは呼び出し側が決めます。
```

`INPUT_CLASS` が表す exact な集合と Adapter applicability の違いは [Adapter の exact input（INPUT_CLASS）](./adapter_input_class.md) を参照してください。本ページでは、その集合へ向けた Preparation の責任境界を説明します。

## Adapter の推奨と呼び出し側の実行

{meth}`~ommx.adapter.SolverAdapter.recommended_preparation_policy` は、Adapter の exact input へ到達する際に一般的に有用な {class}`~ommx.PreparationPolicy` を返します。

このメソッドは次の契約を持ちます。

- 呼び出すたびに、呼び出し側が編集できる新しい Policy を返す
- 特定の Instance を参照または変更しない
- Preparation を実行しない
- target membership や Adapter 固有 precondition の成立を保証しない

`INPUT_CLASS` と推奨 Policy は独立した値です。標準的な責任分担は次のようになります。

```python
import copy

input_class = Adapter.INPUT_CLASS
assert input_class is not None
policy = Adapter.recommended_preparation_policy()

# 必要なら、application 固有の判断で policy を編集します。
prepared = copy.copy(source)
prepared.prepare(input_class, policy)

# prepare() の成功が保証するのは membership までです。
assert input_class.contains(prepared)
Adapter.require_applicable(prepared)
result = Adapter.solve(prepared)
```

`prepare()` は渡された Instance を in-place に変更します。source model を別の Adapter にも使う場合や、変換前後を比較する場合は、どの値を残すかを決めて明示的にコピーしてください。実行可能な例は [Adapter 向けに Instance を準備する](../tutorial/prepare_instance_for_adapter.md) を参照してください。

## PreparationPolicy が選ぶ phase

`PreparationPolicy` の各 field は optional で、未指定の phase は実行されません。デフォルトではすべての phase が無効です。各 field は変換の実装そのものではなく、`Instance` が所有する既存操作のうちどれを呼ぶかを選択します。各操作の入力条件や error は、その owner operation が引き続き所有します。

{meth}`~ommx.Instance.prepare` は、選択された phase を高々1回ずつ次の固定順序で適用します。

| 順序 | Policy field | 役割 |
|---|---|---|
| 1 | `special_constraints` | 選択した active な特殊制約を通常制約へ lowering |
| 2 | `sense` | 最小化問題への正規化 |
| 3 | `integer_slack` | active な通常不等式への Integer slack 導入 |
| 4 | `integer_encoding` | 使用中の Integer 変数を Binary 変数で encoding |
| 5 | `fixed_penalty` | active な通常制約を固定 weight の penalty として目的関数へ移動 |

各 field に設定する型と factory の全引数は、{class}`~ommx.SpecialConstraintPreparation`、{class}`~ommx.SensePreparation`、{class}`~ommx.IntegerSlackPreparation`、{class}`~ommx.IntegerEncodingPreparation`、{class}`~ommx.FixedPenaltyPreparation` の API Reference を参照してください。特殊制約 lowering の数式と成立条件も、各変換メソッドの API Reference が所有します。特殊制約そのものの意味は [特殊制約型](./special_constraints.md) を参照してください。

固定 penalty の大きさのように、問題の scale や求める解の性質に依存して安全な共通値を決められない選択があります。その値は Adapter の推奨に任せず、application が明示的に決めます。

## Target-directed な実行

Preparation は Policy の全 phase を必ず最後まで実行する batch recipe ではありません。`prepare()` は実行前と各 selected phase の完了後に、Instance 全体について target membership を調べます。target に入った時点で停止するため、それより後の phase は選択されていても実行されません。最初から member なら Instance は変更されません。

成功時の postcondition は次の1点です。

```python
assert input_class.contains(instance)
```

Adapter 固有 precondition はこの postcondition に含まれません。Preparation 後の値について `require_applicable()` を呼ぶか、その検査を行う厳格な Adapter API に渡します。

Solver の出力を {class}`~ommx.Solution` や {class}`~ommx.SampleSet` に戻すときも、Adapter へ渡した同じ prepared Instance が評価の owner です。その Instance が保持する decision-variable dependency と removed constraint により、encoding 前の変数値や変換前の制約も評価できます。removed constraint の評価規則は [Removed constraints と実行可能性](./removed_constraints.md) を参照してください。

## 到達できない場合と部分的な変更

すべての configured phase を適用しても target に到達しない場合、{class}`~ommx.PreparationTargetNotReachedError` が送出されます。`report` 属性には、最終状態に対する {class}`~ommx.InstanceClassMembershipReport` が入ります。呼び出し側は report を見て Policy を追加・変更するか、異なる target を選べます。

Preparation 全体を囲む global transaction はありません。後の phase や owner operation が失敗しても、それより前に完了した変更は Instance に残ります。個々の owner operation がどこまで検証してから変更するかは、それぞれの API contract に従います。source を失敗前の状態に保つ必要がある場合も、`prepare()` の前にコピーを作ることが呼び出し側の責任です。

この failure semantics は Preparation の詳細です。最初に Adapter を利用するときは Tutorial の happy path から始め、必要になった時点で本節と各 owner operation の API Reference を確認してください。
