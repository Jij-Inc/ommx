# Instance Preparation と PreparationPolicy

Instance Preparationは、{class}`~ommx.Instance`に既存の変換操作を適用し、targetの{class}`~ommx.InstanceClass`に属する表現へ近づける仕組みです。**変換先を表すclass、許可する操作を選ぶPolicy、実際に変換する`Instance.prepare()`を分けて考えます。** ソルバーやAdapterがなくても利用できます。

| 要素 | 役割 |
|---|---|
| targetのInstanceClass | 満たしたい表現の条件 |
| PreparationPolicy | 使ってよい変換操作と設定の選択 |
| Instance.prepare | そのInstanceへ変換を適用し、target membershipを検査する |

classの意味は[InstanceClassとAdapterの入力条件](./instance_class.md)、SDKだけで一連の操作を実行する例は[ソルバーなしの変換How-to](../how_to/transform_instance_without_solver.md)を参照してください。後半では、この共通の仕組みをAdapterに適用します。

## Policyを作ることと、変換することは別

{class}`~ommx.PreparationPolicy`は、変換操作の選択を保持する値です。Policyを作成・編集しただけではInstanceは変わりません。

例えば、OneHotを通常の制約へloweringするPolicyは次のように作ります。ここでは変換元の`source`と、変換先の`target`が用意されているとします。

```python
import copy

from ommx import (
    PreparationPolicy, SpecialConstraintKind, SpecialConstraintPreparation,
)

policy = PreparationPolicy(
    special_constraints=SpecialConstraintPreparation.lower_special_constraints(
        kinds={SpecialConstraintKind.OneHot}
    )
)

prepared = copy.copy(source)
prepared.prepare(target, policy)
assert target.contains(prepared)
```

{meth}`~ommx.Instance.prepare`は呼び出したInstanceをin-placeに変更します。sourceを残して変換前後を比較する場合は、上のようにcopyへ適用します。sourceとtargetの構築も含む実行例は[変換How-to](../how_to/transform_instance_without_solver.md)にあります。

## Targetに入った時点で止まる

PreparationはPolicyのすべての操作を必ず最後まで実行するbatch recipeではありません。実行前と各selected phaseの完了後にInstance全体のmembershipを調べ、targetに入った時点で停止します。最初からmemberならInstanceは変更されません。

成功時に成立する条件は、指定したtargetへのmembershipです。

```python
assert target.contains(prepared)
```

この条件だけでは、元の問題との数学的な等価性や最適性は保証されません。変換元の条件、使う操作、その操作が保存するものを個別に確認します。Preparationは求解ではなく、ここで作業を終えることもできます。

変換後の候補は、そのInstanceが保持する変数・制約・目的の情報を使って解釈します。{ref}`Instanceの変数の役割 <instance-variable-roles>`と[変換後の問題とRemoved constraints](./removed_constraints.md)を参照してください。

## PreparationPolicy が選ぶ phase

`PreparationPolicy` の各 field は optional で、未指定の phase は実行されません。デフォルトではすべての phase が無効です。各 field は、`Instance`の変換操作のうちどれを呼ぶかを選択します。各操作の入力条件やerrorは、対応するメソッドのAPI Referenceを参照してください。

{meth}`~ommx.Instance.prepare` は、選択された phase を高々1回ずつ次の固定順序で適用します。

| 順序 | Policy field | 役割 |
|---|---|---|
| 1 | `special_constraints` | 選択した active な特殊制約を通常制約へ lowering |
| 2 | `objective` | active objectiveを指定したsenseへ変換 |
| 3 | `integer_slack` | active な通常不等式への Integer slack 導入 |
| 4 | `integer_encoding` | 使用中の Integer 変数を Binary 変数で encoding |
| 5 | `fixed_penalty` | active な通常制約を固定 weight の penalty として目的関数へ移動 |
| 6 | `binary_power_reduction` | activeなBinary変数の冪を縮約 |

各fieldに設定する型とfactoryの全引数は、{class}`~ommx.SpecialConstraintPreparation`、{class}`~ommx.ObjectivePreparation`、{class}`~ommx.IntegerSlackPreparation`、{class}`~ommx.FixedPenaltyPreparation`、{class}`~ommx.IntegerEncodingPreparation`、{class}`~ommx.BinaryPowerPreparation`のAPI Referenceを参照してください。特殊制約loweringの数式と成立条件も、各変換メソッドのAPI Referenceが所有します。特殊制約そのものの意味は[特殊制約型](./special_constraints.md)を参照してください。

固定penaltyの大きさのように、問題のscaleや求める解の性質に依存する選択があります。どの変換を許し、どの値を使うかはapplicationが判断します。

## Adapterへの適用：推奨Policyと自動Preparation

Adapterは、直接受け取れるInstanceの集合を`INPUT_CLASS`として宣言します。このInstanceClassをtargetとして使うのがAdapter向けのPreparationです。

通常の求解・samplingには、まず`solve()`または`sample()`を使えます。標準のeasy APIは、入力のcopyを作り、Adapterの推奨Policyでprepareしてから厳格なmethodを呼びます。

```python
# sourceは変更されない。内部のcopyだけがPreparationされる。
result = Adapter.solve(source)
```

{meth}`~ommx.adapter.SolverAdapter.recommended_preparation_policy`は、Adapterのexact inputへ到達する際に一般的に有用なPolicyを返します。この呼び出しは次の性質を持ちます。

- 呼び出すたびに、呼び出し側が編集できる新しいPolicyを返す
- 特定のInstanceを参照または変更せず、Preparationもしない
- target membershipへの到達を保証しない

`INPUT_CLASS`と推奨Policyは独立した値です。固定penaltyの大きさなど、Adapterが安全な値を推奨できないapplication固有の選択がある場合は、自分でPolicyを編集し、明示的な経路を使います。

```python
input_class = Adapter.INPUT_CLASS
policy = Adapter.recommended_preparation_policy()

# 必要なら、application固有の判断でpolicyを編集する。
prepared = copy.copy(source)
prepared.prepare(input_class, policy)

assert input_class.contains(prepared)
result = Adapter.solve_without_preparation(prepared)
```

`*_without_preparation()`とAdapter constructorはPreparationしません。上の経路では、編集したPolicyを自分で適用してから渡します。一連の実行例は[Preparationで変換する制約族を選ぶ](../how_to/customize_preparation.md)を参照してください。

Adapterでは`INPUT_CLASS` membershipがapplicabilityです。converterやBackendは、その後のsolver input構築や実行で別のerrorを返すことがあります。それらは追加のapplicability条件ではありません。

Solverの出力を{class}`~ommx.Solution`や{class}`~ommx.SampleSet`に戻すときも、Adapterへ渡した同じprepared Instanceで評価します。そのInstanceが保持する変数間のdependencyとremoved constraintにより、encoding前の変数値や変換前の制約も評価できます。値の復元は[Instanceの基本モデル](./instance.md)、制約の評価規則は[Removed constraintsと実行可能性](./removed_constraints.md)を参照してください。

## 詳細：到達できない場合と部分的な変更

すべての configured phase を適用しても target に到達しない場合、{class}`~ommx.PreparationTargetNotReachedError` が送出されます。`report` 属性には、最終状態に対する {class}`~ommx.InstanceClassMembershipReport` が入ります。呼び出し側は report を見て Policy を追加・変更するか、異なる target を選べます。

Preparation全体が失敗しても、自動的に変更前の状態へ戻るわけではありません。後のphaseが失敗しても、それより前に完了した変更はInstanceに残ります。通常の`solve()`と`sample()`ではこの変更も内部copyだけに残ります。明示的な`prepare()`でsourceを失敗前の状態に保つ必要がある場合は、先にcopyを作ってください。個々の変換が失敗時にどこまで変更を残すかは、それぞれのAPI Referenceを参照してください。

初めてAdapterを使う場合は[Tutorial](../tutorial/prepare_instance_for_adapter.md)から始め、失敗時の処理が必要になった時点で本節と各変換のAPI Referenceを確認してください。
