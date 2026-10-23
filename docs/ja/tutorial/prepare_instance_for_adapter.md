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

前の[PySCIPOpt Adapterで特殊制約をそのまま解く](./solve_special_constraints_with_pyscipopt_adapter.md)では、Indicator制約とSOS1制約をSCIPへ直接渡しました。しかし、すべてのAdapterが同じ種類のInstanceを直接受け取れるわけではありません。

このチュートリアルでは、同じモデルをHiGHS Adapterで解きます。覚えておくことは、**Adapterが直接扱えない表現も、OMMXが変換してから渡せる**ということです。この準備をInstance Preparationと呼びます。

## 必要なライブラリのインストール

```
pip install ommx-highs-adapter
```

## 同じモデルを作る

このページだけでも実行できるように、前のチュートリアルと同じモデルを作り直します。

```{code-cell} ipython3
from ommx import Instance, Sos1Constraint

source = Instance.maximize()
enabled = source.new_binary("enabled")
option_a = source.new_binary("option_a")
option_b = source.new_binary("option_b")
option_c = source.new_binary("option_c")

source.objective = (
    10 * enabled
    + 6 * option_a
    + 5 * option_b
    + 4 * option_c
)
source.add_indicator_constraint(
    (option_a + option_b <= 1).with_indicator(enabled)
)
source.add_sos1_constraint(
    Sos1Constraint(
        variables=[option_b, option_c],
        name="exclusive_options",
    )
)
```

## `solve()`にPreparationを任せる

HiGHS Adapterは通常の線形制約を受け取れますが、Indicator制約やSOS1制約は直接受け取りません。それでも、前のページと同じように`solve()`を呼び出せます。

```{code-cell} ipython3
from ommx_highs_adapter import OMMXHighsAdapter

solution = OMMXHighsAdapter.solve(source)

assert solution.feasible
assert abs(solution.objective - 20.0) < 1e-8

# 呼び出し元のInstanceは変更されない
assert source.indicator_constraints
assert source.sos1_constraints

solution.decision_variables_df()
```

`solve()`は、元のInstanceを変更せず、そのcopyをHiGHS向けに準備してから解きます。このモデルではIndicator制約とSOS1制約を通常の線形制約へ変換します。特殊制約を通常制約へ書き換える操作を**lowering**と呼びます。

この例の変数はすべてBinaryであり、変換に必要な有限の上下限があります。任意のモデルを必ず変換できるわけではありません。個々のloweringの数式や適用条件はAPI Referenceで説明します。

## 直接受け取れる範囲と、変換の方針

Adapterが直接受け取るInstanceの集合を`INPUT_CLASS`、その集合へ向けて選ぶ変換の方針を`PreparationPolicy`と呼びます。次のコードで、今回のsourceにはHiGHS向けの変換が必要だったことを確認できます。

```{code-cell} ipython3
assert not OMMXHighsAdapter.INPUT_CLASS.contains(source)
```

`solve()`はAdapterの推奨Policyを使いました。HiGHSの場合は今回のloweringを推奨しているため、利用者が変換コードを書く必要はありませんでした。一方、変換の選択や設定がアプリケーションに依存する場合は、推奨をそのまま使わず、自分でPolicyを選びます。

## まとめ

- 特殊制約を直接扱わないAdapterでも、loweringして解ける場合があります。
- `solve()`は入力のcopyに推奨Policyを適用するため、元のInstanceを変更しません。
- `INPUT_CLASS`は直接受け取れる範囲、`PreparationPolicy`はそこへ向かう変換の選択です。

自分で変換を選ぶ必要が生じたら、操作ガイド[Preparationで変換する制約族を選ぶ](../how_to/customize_preparation.md)へ進んでください。仕組みを理解するには[Adapterのexact input（INPUT_CLASS）](../user_guide/adapter_input_class.md)と[Instance PreparationとPreparationPolicy](../user_guide/preparation_policy.md)を参照してください。
