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

OMMX Instanceは広い範囲の数理最適化問題を記述できますが、すべてのAdapterがそれを受け入れられるとは限りません。実務における数理最適化では例えば次のような特殊な制約条件を扱う必要があります。

- 特定の条件を満たした場合だけ制約条件を課したい（Indicator制約）
- 複数の選択肢の中から一つだけ選ぶ必要がある（OneHot制約）
- 複数の変数のうち一つの変数だけ０でない必要がある（SOS1制約）

これらは特定のソルバーでは効率的に扱える一方、例えば純粋なMILPソルバーで扱うにはMILPの範囲でこれを表現する変換が必要になります。

OMMXではこのような一部のソルバーでは効率的に扱える制約条件を「特殊制約」と呼んでInstanceに保持し、対応しているソルバーに対してはAdapterを通して専用のAPIを使ってソルバーの性能を限界まで引き出す方針を取ります。一方特殊制約を直接扱えないソルバーに対しては、OMMXがそのソルバーが扱える形に変換します。このAdapterが扱えるように変換する操作のことをOMMXでは "Preparation" と呼びます。OMMX Adapterは自分が扱える問題の範囲を宣言し、OMMXのAPIがその範囲に治るようにInstanceを変換します。変換方法は一般には一意には定まらないので、変換Policyによって制御します。Adapterは推奨Policyを提示し、ユーザーが必要に応じてPolicyを修正します。

ここではまずIndicator制約とSOS1制約を含むInstanceを直接サポートしているPySCIPOpt Adapterで解く場合と、直接対応していないHiGHS Adapterで解く場合をそれぞれ解説します。

## 必要なライブラリのインストール

```
pip install ommx-pyscipopt-adapter ommx-highs-adapter
```

## PySCIPOpt Adapterで特殊制約を直接扱う

Indicator制約は、バイナリ変数が1のときだけ有効になる制約です。SOS1制約は、指定した変数のうち高々1つだけが非ゼロになることを要求します。PySCIPOpt Adapterは、線形なIndicator制約とSOS1制約を直接扱えます。

次のモデルでは、`enabled = 1` のとき `option_a + option_b <= 1` というIndicator制約が有効になります。また、SOS1制約により `option_b` と `option_c` の両方を同時に1にはできません。

```{code-cell} ipython3
from ommx import Instance, Sos1Constraint

source = Instance.maximize()
enabled = source.new_binary("enabled")
option_a = source.new_binary("option_a")
option_b = source.new_binary("option_b")
option_c = source.new_binary("option_c")

source.objective = 10 * enabled + 6 * option_a + 5 * option_b + 4 * option_c
source.add_indicator_constraint(
    (option_a + option_b <= 1).with_indicator(enabled)
)
source.add_sos1_constraint(
    Sos1Constraint(variables=[option_b, option_c], name="exclusive_options")
)
```

PySCIPOpt AdapterはSOS1とIndicator制約を扱えるので、Preparationせずそのまま解けます。

```{code-cell} ipython3
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

direct_solution = OMMXPySCIPOptAdapter.solve(source)
```

## HiGHS Adapterで特殊制約を変換して扱う

次に直接IndicatorとSOS1を扱えないHiGHS Adapterの場合を見ていきます。

### Adapterが直接受け取れる入力

各Adapterは `INPUT_CLASS` を定義しており、これはそのAdapterが直接扱えるInstanceの性質を指定します。 {meth}`~ommx.InstanceClass.contains`で実際にInstanceがその条件を満たすかを判定できます。

```{code-cell} ipython3
scip_input_class = OMMXPySCIPOptAdapter.INPUT_CLASS
assert scip_input_class.contains(source)
```

HiGHSは純粋なMILPソルバーというわけではありませんが、Indicator制約やSOS1制約を直接は受け取りません。そのためこのInstanceはHiGHS Adapterの `INPUT_CLASS` には入りません。

```{code-cell} ipython3
from ommx_highs_adapter import OMMXHighsAdapter

highs_input_class = OMMXHighsAdapter.INPUT_CLASS
assert not highs_input_class.contains(source)
```

### 推奨PolicyでPreparationする

Adapterの {meth}`~ommx.SolverAdapter.recommended_preparation_policy` は、その入力classへ近づけるためにAdapterが推奨する {class}`~ommx.PreparationPolicy` を返します。HiGHSの推奨PolicyはIndicator制約とSOS1制約を通常制約へBig-Mでloweringします。

推奨Policyを取得しただけではInstanceは変わりません。どのInstanceをPreparationするかを決めて {meth}`Instance.prepare() <ommx.Instance.prepare>` を呼ぶのはユーザーです。元のモデルを残すため、ここでは `copy.copy()` で作ったコピーをPreparationします。

```{code-cell} ipython3
import copy

policy = OMMXHighsAdapter.recommended_preparation_policy()

prepared = copy.copy(source)
prepared.prepare(highs_input_class, policy)

# 変換後のInstanceは INPUT_CLASS に入る
assert highs_input_class.contains(prepared)

# 特殊制約はloweringされてなくなっている
assert not prepared.indicator_constraints
assert not prepared.sos1_constraints

# 特殊制約がBig-MでMILPで表現されているのでHiGHSは扱える
prepared_solution = OMMXHighsAdapter.solve(prepared)
```

## まとめ

- Adapterが直接受け取れるInstanceは、そのまま `solve()` に渡せます。
- `INPUT_CLASS` は、Adapterが直接受け取れるexactな入力の集合です。
- 推奨Policyは変換の提案であり、Preparationの実行ではありません。
- 変換するInstanceとPolicyを選び、`Instance.prepare()` を呼ぶ責任はユーザーにあります。
- 同じsource modelでも、Adapterごとに異なるexact inputへ準備できます。

Preparationの各変換やexact inputの詳しい意味は、[Adapterのexact inputとInstance preparation](../user_guide/capability_model.md)を参照してください。Indicator、OneHot、SOS1の数学的な意味と個別の変換APIは、[特殊制約型](../user_guide/special_constraints.md)で説明しています。
