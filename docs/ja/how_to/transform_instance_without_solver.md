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

# Instanceを変換し、結果を確認する

問題を特定の表現へ変換したい場合は、**変換先のInstanceClassと、許可する変換のPolicyを指定して`prepare()`を呼びます。** このページではOneHot制約を通常の線形等式へ変え、手元の候補を評価して変換前後の結果を確認します。

## 実行環境

{ref}`Python SDKのインストール <install-python-sdk>`までを済ませてください。Adapterやソルバーの追加は不要です。このページのコードは、ほかのTutorialの実行状態を使いません。

## 1. 問題と比較用の評価結果を用意する

二つのBinary変数から、ちょうど一つを選ぶ問題です。選択の価値を`3 * x[0] + 2 * x[1]`とします。後で変換前後を比較するため、一つだけ選ぶ候補と二つとも選ぶ候補の評価結果を先に保存します。`evaluate()`は候補を検査する操作で、求解ではありません。

```{code-cell} ipython3
from ommx import Instance, OneHotConstraint

instance = Instance.maximize()
x = [instance.new_binary("x", subscripts=[i]) for i in range(2)]
instance.objective = 3 * x[0] + 2 * x[1]
instance.add_one_hot_constraint(OneHotConstraint(variables=x))

state = {x[0].id: 1, x[1].id: 0}
both = {x[0].id: 1, x[1].id: 1}
before = instance.evaluate(state)
before_both = instance.evaluate(both)
```

## 2. 変換先の表現を指定する

ここでは、Binary変数、線形目的関数、通常の線形等式だけを許すclassを作ります。OneHot制約はそのままでは入りません。

```{code-cell} ipython3
from ommx import (
    Equality, InstanceClass, InstanceClassClause, Kind,
    PolynomialRequirement, Sense,
)

linear = PolynomialRequirement.at_most(1)
target = InstanceClass([
    InstanceClassClause(
        label="binary-linear-equality",
        allowed_variable_kinds={Kind.Binary},
        objective_polynomial_requirement=linear,
        regular_constraint_polynomial_requirements={Equality.EqualToZero: linear},
        allowed_senses={Sense.Maximize},
    )
])

assert not target.contains(instance)
```

`contains()`は、いまのInstanceが条件を満たすかを調べるだけです。まだ変換は行いません。条件の組合せや表現形式については[InstanceClass](../user_guide/instance_class.md)を参照してください。

## 3. Policyを選び、Instanceを変換する

OneHotだけを通常制約へloweringするPolicyを選びます。`prepare()`は手元のInstanceを直接変更します。比較に使う評価結果は保存済みなので、変換前のInstance自体を残す必要はありません。

```{code-cell} ipython3
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

この例ではOneHotの条件「ちょうど一つが1」を、通常の等式`x[0] + x[1] == 1`で表しました。Policyは変換を実行する関数ではなく、`prepare()`が使ってよい操作の選択です。

## 4. Activeとremovedを確認する

現在の問題で課す制約をactive、課す条件から外して評価用に保持する制約をremovedと呼びます。変換後は通常のactiveな制約が一つあり、元のOneHotはremoved側に保持されています。

```{code-cell} ipython3
assert len(instance.constraints) == 1
assert not instance.constraints
assert len(instance.removed_constraints) == 1

instance.constraints_df(kind="regular")
```

removedは「もう評価しない」という意味ではありません。現在課す通常制約とは別に、元のOneHotとしても候補を検査できます。詳しくは[変換後の問題とRemoved constraints](../user_guide/removed_constraints.md)を参照してください。

## 5. 手元の候補を評価する

`x[0] = 1`、`x[1] = 0`という同じ候補を、変換済みのInstanceで再び評価します。変換前に保存した評価結果と、実行可能性と目的値を比較します。

```{code-cell} ipython3
after = instance.evaluate(state)

assert before.feasible and after.feasible
assert before.objective == after.objective == 3
print(after.objective)
```

二つとも選ぶ候補は、変換前後とも実行不可能です。

```{code-cell} ipython3
assert not before_both.feasible
assert not instance.evaluate(both).feasible
```

これで、Instanceを指定した表現へ変換し、候補の評価結果を比較できました。`target.contains(instance)`が示すのは**要求した表現の条件を満たしたこと**です。元の問題との数学的な対応は、選んだ操作の保証と適用条件から判断します。この例の候補評価の一致だけで、任意の変換の等価性や最適性を証明したことにはなりません。

変換先・Policy・実行の関係は[Instance PreparationとPreparationPolicy](../user_guide/preparation_policy.md)、OneHot loweringの正確な契約は{meth}`~ommx.Instance.convert_one_hot_to_constraint`を参照してください。自分のPolicyでAdapterを実行したい場合は、別の操作ガイド[Preparationで変換する制約族を選ぶ](./customize_preparation.md)へ進めます。
