---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: Python 3 (ipykernel)
  language: python
  name: python3
---

# 特殊制約の意味と評価

OMMXは、通常の等式・不等式に加えて、Indicator・OneHot・SOS1を制約として扱います。それぞれの**成立条件はソルバーとは独立に定義されており、ソルバーを呼ばずに評価できます。** 特殊制約は、特定のソルバーへの指示ではなく、問題そのものの条件です。

| 制約の種類 | 表す条件 |
|---|---|
| {class}`~ommx.IndicatorConstraint` | バイナリ変数が1のときだけ、指定した等式・不等式を課す |
| {class}`~ommx.OneHotConstraint` | バイナリ変数のうち、ちょうど1個が1 |
| {class}`~ommx.Sos1Constraint` | 変数のうち、高々1個が非ゼロ |

このページでは[InstanceとStateの評価](./solution.md)を使って、それぞれが何を意味するかを確認します。求解の手順は[Adapterで最適化問題を解くTutorial](../tutorial/solve_with_ommx_adapter)を参照してください。

## Indicator：条件付きの制約

バイナリ変数 $z$ に対して、$z=1$ のときだけ $f(x)\leq0$ または $f(x)=0$ を要求します。$z=0$ のときは、式の値によらずIndicator制約は満たされます。ただし、変数自身のdomainや、他の制約まで無効になるわけではありません。

次の例は「設備を使う場合だけ、量を5以下にする」という条件です。

```{code-cell} ipython3
from ommx import Instance

indicator = Instance.maximize()
enabled = indicator.new_binary("enabled")
amount = indicator.new_continuous("amount", lower=0, upper=10)
indicator.objective = amount
indicator.add_indicator_constraint((amount <= 5).with_indicator(enabled))

assert indicator.evaluate({enabled.id: 0, amount.id: 6}).feasible
assert indicator.evaluate({enabled.id: 1, amount.id: 5}).feasible
assert not indicator.evaluate({enabled.id: 1, amount.id: 6}).feasible
```

{meth}`~ommx.Constraint.with_indicator` で通常制約に制御変数を指定します。これは「$z=1$ならば制約が成立する」という片方向の含意であり、「制約が成立するなら必ず$z=1$」とは限りません。

## OneHot：ちょうど一つ選ぶ

バイナリ変数 $x_0,\ldots,x_{n-1}$ に対して、$\sum_i x_i=1$ を要求します。選ばれない場合や、複数選ばれる場合は実行不可能です。

```{code-cell} ipython3
from ommx import OneHotConstraint

one_hot = Instance.maximize()
choices = [one_hot.new_binary("choice", subscripts=[i]) for i in range(3)]
one_hot.objective = sum(v * x for v, x in zip([5, 10, 3], choices))
one_hot.add_one_hot_constraint(OneHotConstraint(variables=choices))

chosen = one_hot.evaluate({choices[0].id: 0, choices[1].id: 1, choices[2].id: 0})
assert chosen.feasible
assert chosen.objective == 10
assert not one_hot.evaluate({x.id: 0 for x in choices}).feasible
```

数学的には通常の等式と同じ条件です。OneHotとして保持すると、「いずれか一つを選ぶ」という構造を、変換前にも変換後の結果確認にも利用できます。参照する変数はすべてBinaryである必要があります。

(sos1-constraint)=
## SOS1：非ゼロは高々一つ

SOS1は、指定した変数のうち高々1個だけが非ゼロであることを要求します。変数はBinaryでなくてもよく、全変数が0でも満たされます。非ゼロの変数の値を1に制限する条件でもありません。

```{code-cell} ipython3
from ommx import Sos1Constraint

sos1 = Instance.maximize()
amounts = [
    sos1.new_continuous("amount", lower=0, upper=10, subscripts=[i])
    for i in range(3)
]
sos1.objective = sum(amounts)
sos1.add_sos1_constraint(Sos1Constraint(variables=amounts))

assert sos1.evaluate({x.id: 0 for x in amounts}).feasible
assert sos1.evaluate({amounts[0].id: 7, amounts[1].id: 0, amounts[2].id: 0}).feasible
assert not sos1.evaluate({amounts[0].id: 7, amounts[1].id: 1, amounts[2].id: 0}).feasible
```

OneHotと違って、「どれも選ばない」「一つだけ正の量を割り当てる」といった候補を表せます。負の値が許される変数も使え、符号によらず非ゼロかどうかを判定します。数値評価では、ほかの制約と同様に許容誤差を使います。

(sos1-big-m-formulation)=
## 通常制約へのloweringと、特殊制約へのpromotion

ソルバーが特殊制約を直接扱わない場合でも、OMMXの定義する条件を通常制約で表せる場合があります。この変換をloweringと呼びます。例えばOneHotは等式へ、有限なboundを持つIndicatorやSOS1はBig-Mを使う通常制約へ変換できます。

重要なのは、ソルバーへの対応と、制約の意味を分けて考えることです。直接渡す場合もloweringする場合も、出発点は上で定義した条件です。ただし、適用に必要なboundや数値計算上の条件は変換によって異なります。単に大きな係数を置けばよいわけではありません。

逆に、指定した通常制約の組がSOS1を表すことを検証し、特殊制約として表し直す操作がpromotionです。変換式・生成物・正確な受理条件はAPI Referenceを参照してください。

- Indicatorのlowering：{meth}`~ommx.Instance.convert_indicator_to_constraint`
- OneHotのlowering：{meth}`~ommx.Instance.convert_one_hot_to_constraint`
- SOS1のBig-M lowering：{meth}`~ommx.Instance.convert_sos1_to_constraints`
- 通常制約からSOS1へのpromotion：{meth}`~ommx.Instance.promote_sos1_big_m`

loweringでは、元の特殊制約をremovedとして保持し、生成した通常制約から変換元を参照できます。結果を元の制約として検査する仕組みは[変換後の問題とRemoved constraints](./removed_constraints.md)で説明します。

## 制約IDと評価結果の参照

通常制約・Indicator・OneHot・SOS1のIDの番号は、removed を含む同じInstance内の全制約で一意です。`from_components` に渡す種類別の辞書にも、重複しない番号を指定してください。各制約が参照する変数は、制約とは別の決定変数IDで指定します。

Instanceは全種類のactive／removedの最大番号より後から新しい番号を発行します。lowering・promotionで生成する制約には新しい番号を付け、removedに移す元の制約は元の番号を維持します。変換前後の関係はprovenanceや削除理由に記録されます。

`ParametricInstance`も同じ規則に従い、評価後の`Solution`・`SampleSet`もIDを維持します。protobufの読み込みでは種類間の番号重複をエラーとし、自動再採番はしません。独立したID空間を使っていた3.0.0 beta期間中のデータにも、この検証を適用します。

SolutionやSampleSetのDataFrameは、`kind=` で種類を選びます。

| 制約の種類 | `kind=` |
|---|---|
| 通常制約 | `"regular"`（既定値） |
| Indicator | `"indicator"` |
| OneHot | `"one_hot"` |
| SOS1 | `"sos1"` |

```{code-cell} ipython3
chosen.constraints_df(kind="one_hot")
```

Indicatorでは `indicator_active` 列により、制御変数が0だった場合と、制御変数が1で本体の条件を検査した場合を区別できます。通常制約の式の値と、特殊制約ごとの違反量は意味が異なるので、各DataFrameの列の定義は {meth}`~ommx.Solution.constraints_df` で確認してください。
