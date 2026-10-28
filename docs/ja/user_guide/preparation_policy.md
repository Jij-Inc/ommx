# Instance Preparation と PreparationPolicy

Instance Preparationは、{class}`~ommx.Instance`に既存の変換操作を適用し、targetの{class}`~ommx.InstanceClass`に属する表現へ近づける仕組みです。**変換先を表すclass、許可する操作を選ぶPolicy、実際に変換する`Instance.prepare()`を分けて考えます。**

| 要素 | 役割 |
|---|---|
| targetのInstanceClass | 満たしたい表現の条件 |
| PreparationPolicy | 使ってよい変換操作と設定の選択 |
| Instance.prepare | そのInstanceへ変換を適用し、target membershipを検査する |

classの意味は[InstanceClassとAdapterの入力条件](./instance_class.md)、一連の操作例は[Instanceを変換するHow-to](../how_to/transform_instance_without_solver.md)を参照してください。後半では、この共通の仕組みをAdapterに適用します。

## Policyを作ることと、変換することは別

{class}`~ommx.PreparationPolicy`は、変換操作の選択を保持する値です。Policyを作成・編集しただけではInstanceは変わりません。

例えば「ちょうど一つを選ぶ」というOneHotの条件を、通常の等式へ置き換えるPolicyは次のように作ります。特殊制約を通常の制約へ書き換える操作をloweringと呼びます。ここでは変換する`instance`と、変換先の`target`が用意されているとします。

```python
from ommx import (
    PreparationPolicy, SpecialConstraintKind, SpecialConstraintPreparation,
)

policy = PreparationPolicy(
    special_constraints=SpecialConstraintPreparation.lower_special_constraints(
        kinds={SpecialConstraintKind.OneHot}
    )
)

instance.prepare(target, policy)
assert target.contains(instance)
```

{meth}`~ommx.Instance.prepare`は呼び出したInstanceをin-placeに変更します。変換前のInstance自体を後でも使う必要がある場合だけ、事前にcopyを作ります。候補の評価結果を比較するだけなら、変換前に評価結果を保存しておけば十分です。Instanceとtargetの構築も含む実行例は[変換How-to](../how_to/transform_instance_without_solver.md)にあります。

## Targetに入った時点で止まる

PreparationはPolicyのすべての操作を必ず最後まで実行する仕組みではありません。targetに入った時点で停止します。最初からmemberならInstanceは変更されません。

成功時に成立する条件は、指定したtargetへのmembershipです。

```python
assert target.contains(instance)
```

この条件だけでは、元の問題との数学的な等価性や最適性は保証されません。変換元の条件、使う操作、その操作が保存するものを個別に確認します。

候補・実行可能性・目的値・最適性について何を確認するかは、[変換が何を保存するか](./transformation_guarantees.md)で説明します。

変換後の候補は、そのInstanceが保持する変数・制約・目的の情報を使って解釈します。{ref}`Instanceの変数の役割 <instance-variable-roles>`と[変換後の問題とRemoved constraints](./removed_constraints.md)を参照してください。

## 使用を許可する変換を選ぶ

Policyには、特殊制約のlowering、整数変数の符号化、制約のpenalty化など、使用を許す変換を設定します。未指定の変換は実行されません。変換先のclassだけでなく、どの操作を使ってよいかも利用者が選びます。

設定できる項目・型は{class}`~ommx.PreparationPolicy`、実行順序・適用回数・停止判定の詳細は{meth}`~ommx.Instance.prepare`のAPI Referenceを参照してください。特殊制約loweringの数式と成立条件も、各変換メソッドのAPI Referenceが所有します。特殊制約そのものの意味は[特殊制約型](./special_constraints.md)を参照してください。

固定penaltyの大きさのように、問題のscaleや求める解の性質に依存する選択があります。どの変換を許し、どの値を使うかはapplicationが判断します。

## QUBOへの準備と係数への整形を分ける

QUBO向けの一般的なPolicyには {meth}`~ommx.PreparationPolicy.for_qubo` があります。これはPolicyを作るだけであり、Instanceを変更しません。`InstanceClass.qubo()`をtargetにして明示的に準備し、係数が必要なら後から整形します。

```python
from ommx import InstanceClass, PreparationPolicy

policy = PreparationPolicy.for_qubo(uniform_penalty_weight=2.0)
instance.prepare(InstanceClass.qubo(), policy)
coefficients, offset = instance.as_qubo_format()
```

ここで2.0は利用者が選んだ重みの例であり、任意の問題に十分な値ではありません。`for_qubo()`自体にも引数を省略した場合の既定値があります。Factoryの既定値があることと、その問題で望む候補を得られることは別です。特に、不等式の本体を直接二乗するpenaltyの注意は[変換の保証](./transformation_guarantees.md)を参照してください。

{meth}`~ommx.Instance.to_qubo` は、このQUBO向けPreparationとactive objectiveの係数整形をまとめた経路で、Instanceをin-placeに変更します。一方、{meth}`~ommx.Instance.as_qubo_format` はPreparationをしません。どちらも返す係数はactiveな目的関数のものであり、`instance.evaluate()`が返す出力目的値と常に同じとは限りません。

準備後も対象はOMMX Instanceです。変数の復元・制約・出力目的関数を確認する情報を保持できますが、係数と定数項だけを渡した先にそれらが自動的に付随するわけではありません。問題と候補をOMMXとして評価・保存する場合はInstanceも扱います。

一般の`for_qubo()`と、具体的なAdapterの推奨Policyは別の選択です。例えばOpenJij SA Adapterは二次より高次も直接受け取り、推奨Policyは固定penaltyの重みを利用者に委ねます。[OpenJij Adapterで小さな問題をサンプリングする](../how_to/sample_with_openjij_adapter)では、そのAdapterのtargetと推奨から始めます。

## Adapterへの適用：推奨Policyと自動Preparation

Adapterは、直接受け取れるInstanceの集合を`INPUT_CLASS`として宣言します。このInstanceClassをtargetとして使うのがAdapter向けのPreparationです。

通常の求解・samplingには、まず`solve()`または`sample()`を使えます。標準のeasy APIは、入力のcopyを作り、Adapterの推奨Policyでprepareしてから厳格なmethodを呼びます。

```python
# 呼び出し元のinstanceは変更されない。内部のcopyだけがPreparationされる。
result = Adapter.solve(instance)
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
instance.prepare(input_class, policy)

assert input_class.contains(instance)
result = Adapter.solve_without_preparation(instance)
```

`*_without_preparation()`とAdapter constructorはPreparationしません。上の経路では、編集したPolicyを自分で適用してから渡します。一連の実行例は[Preparationで変換する制約族を選ぶ](../how_to/customize_preparation.md)を参照してください。

Adapterでは`INPUT_CLASS` membershipがapplicabilityです。converterやBackendは、その後のsolver input構築や実行で別のerrorを返すことがあります。それらは追加のapplicability条件ではありません。

Solverの出力を{class}`~ommx.Solution`や{class}`~ommx.SampleSet`に戻すときも、Adapterへ渡した同じ変換済みInstanceで評価します。そのInstanceが保持する変数間のdependencyとremoved constraintにより、encoding前の変数値や変換前の制約も評価できます。値の復元は[変数の役割と値の復元・補完](./variable_roles.md)、制約の評価規則は[Removed constraintsと実行可能性](./removed_constraints.md)を参照してください。

## 失敗時のInstanceを再利用する場合

明示的な`prepare()`は、失敗しても変更前の状態へ自動的に戻るとは限りません。失敗後も変換前のInstance自体を使う必要がある場合だけ、事前にcopyを作ってください。

例外・診断情報の取得方法と変更が残る範囲は、{meth}`~ommx.Instance.prepare`および各変換メソッドのAPI Referenceにあります。初めてAdapterを使う場合は[Tutorial](../tutorial/prepare_instance_for_adapter.md)から始め、失敗の調査が必要になった時点で確認できます。
