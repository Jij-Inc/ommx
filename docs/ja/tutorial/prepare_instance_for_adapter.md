---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: ommx-update-books (3.9.23)
  language: python
  name: python3
---

# Adapter向けにInstanceを準備する

OMMX Instanceは広い範囲の数理最適化問題を記述できますが、すべてのAdapterがそれを受け入れられるとは限りません。実務における数理最適化では、例えば特定の条件を満たした場合だけ制約条件を課したいケース（Indicator制約）や、複数の選択肢の中から一つだけ選ぶ必要があるケース（OneHot制約）、複数の変数のうち一つの変数だけ０でない必要がある（SOS1制約）、と言った複雑な条件を扱う必要がしばしばあります。これらは特定のソルバーでは効率的に扱える一方、例えば純粋なMILPソルバーで扱うにはMILPの範囲でこれを表現する変換が必要になります。

OMMXではこのような一部のソルバーでは効率的に扱える制約条件を「特殊制約」と呼んでInstanceに保持し、対応しているソルバーに対してはAdapterを通して専用のAPIを使ってソルバーの性能を限界まで引き出す方針を取ります。一方特殊制約を直接扱えないソルバーに対しては、OMMXがそのソルバーが扱える形に変換します。このAdapterが扱えるように変換する操作のことをOMMXでは "Preparation" と呼びます。OMMX Adapterは自分が扱える問題の範囲を宣言し、OMMXのAPIがその範囲に治るようにInstanceを変換します。変換方法は一般には一意には定まらないので、変換Policyによって制御します。Adapterは推奨Policyを提示し、ユーザーが必要に応じてPolicyを修正します。

ここではまずIndicator制約とSOS1制約を含むInstanceを直接サポートしているPySCIPOpt Adapterで解く場合と、直接対応していないHiGHS Adapterで解く場合をそれぞれ解説します。

## 必要なライブラリのインストール

```
pip install ommx-pyscipopt-adapter ommx-highs-adapter
```

## IndicatorとSOS1をそのまま解く

Indicator制約は、バイナリ変数が1のときだけ有効になる制約です。SOS1制約は、指定した変数のうち高々1つだけが非ゼロになることを要求します。PySCIPOpt Adapterは、線形なIndicator制約とSOS1制約を直接扱えます。

次のモデルでは、`enabled = 1` のとき `production <= 5` というIndicator制約が有効になります。また、SOS1制約により `option_a` と `option_b` の両方を同時に非ゼロにはできません。

```{code-cell} ipython3
from ommx import DecisionVariable, Instance, Sos1Constraint

enabled = DecisionVariable.binary(0, name="enabled")
production = DecisionVariable.continuous(
    1, lower=0, upper=10, name="production"
)
option_a = DecisionVariable.continuous(2, lower=0, upper=4, name="option_a")
option_b = DecisionVariable.continuous(3, lower=0, upper=6, name="option_b")

source = Instance.from_components(
    decision_variables=[enabled, production, option_a, option_b],
    objective=production + option_a + option_b,
    constraints={0: enabled == 1},
    indicator_constraints={
        0: (production <= 5).with_indicator(enabled),
    },
    sos1_constraints={
        0: Sos1Constraint(variables=[option_a, option_b]),
    },
    sense=Instance.MAXIMIZE,
)
```

このInstanceはPreparationせず、そのまま解けます。

```{code-cell} ipython3
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

direct_solution = OMMXPySCIPOptAdapter.solve(source)

# production = 5, option_a = 0, option_b = 6
assert abs(direct_solution.objective - 11.0) < 1e-6
```

## Adapterが直接受け取れる入力

各Adapterの `INPUT_CLASS` は、そのAdapterが**直接受け取れるInstanceそのものの集合**です。PySCIPOpt Adapterの入力classには、上で使った線形Indicator制約とSOS1制約が含まれます。

```{code-cell} ipython3
scip_input_class = OMMXPySCIPOptAdapter.INPUT_CLASS
assert scip_input_class is not None
assert scip_input_class.contains(source)
```

HiGHS Adapterは通常の線形制約を受け取りますが、Indicator制約やSOS1制約を直接は受け取りません。そのため、同じsource InstanceはHiGHSの入力classには入りません。

```{code-cell} ipython3
from ommx_highs_adapter import OMMXHighsAdapter

highs_input_class = OMMXHighsAdapter.INPUT_CLASS
assert highs_input_class is not None
assert not highs_input_class.contains(source)
```

## 推奨PolicyでPreparationする

Adapterの `recommended_preparation_policy()` は、その入力classへ近づけるためにAdapterが推奨する {class}`~ommx.PreparationPolicy` を返します。HiGHSの推奨PolicyはIndicator制約とSOS1制約を通常制約へloweringします。呼び出すたびに新しいPolicyが返るので、ユーザーは必要に応じて編集できます。

推奨Policyを取得しただけではInstanceは変わりません。どのInstanceをPreparationするかを決めて {meth}`Instance.prepare() <ommx.Instance.prepare>` を呼ぶのはユーザーです。元のモデルを残すため、ここでは `copy.copy()` で作ったコピーをPreparationします。

```{code-cell} ipython3
import copy

prepared = copy.copy(source)
policy = OMMXHighsAdapter.recommended_preparation_policy()

prepared.prepare(highs_input_class, policy)

assert highs_input_class.contains(prepared)
assert source.indicator_constraints and source.sos1_constraints
assert not prepared.indicator_constraints
assert not prepared.sos1_constraints
```

source Instanceは変わらず、`prepared` だけがloweringされています。入力classに入った `prepared` をHiGHS Adapterへ渡して解きます。

```{code-cell} ipython3
prepared_solution = OMMXHighsAdapter.solve(prepared)

# どちらのexact inputも同じsource問題を表す
assert abs(prepared_solution.objective - direct_solution.objective) < 1e-6
```

## まとめ

- Adapterが直接受け取れるInstanceは、そのまま `solve()` に渡せます。
- `INPUT_CLASS` は、Adapterが直接受け取れるexactな入力の集合です。
- 推奨Policyは変換の提案であり、Preparationの実行ではありません。
- 変換するInstanceとPolicyを選び、`Instance.prepare()` を呼ぶ責任はユーザーにあります。
- 同じsource modelでも、Adapterごとに異なるexact inputへ準備できます。

Preparationの各変換やexact inputの詳しい意味は、[Adapterのexact inputとInstance preparation](../user_guide/capability_model.md)を参照してください。Indicator、OneHot、SOS1の数学的な意味と個別の変換APIは、[特殊制約型](../user_guide/special_constraints.md)で説明しています。
