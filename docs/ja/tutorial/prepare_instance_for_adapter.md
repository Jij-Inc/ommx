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

このチュートリアルでは、同じモデルをHiGHS Adapter向けに変換して解きます。覚えておく流れは次の3つです。

1. Adapterの `INPUT_CLASS` で、Instanceをそのまま渡せるか確認する。
2. Adapterの推奨Policyを取得し、変換するInstanceをユーザーが選ぶ。
3. ユーザーが `Instance.prepare()` を呼び、変換後のInstanceをAdapterへ渡す。

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

## Adapterが直接受け取れる入力を確認する

各Adapterの `INPUT_CLASS` は、そのAdapterが**変換なしで直接受け取れるInstanceの集合**です。`contains()` を使うと、手元のInstanceがその集合に入っているか確認できます。

```{code-cell} ipython3
from ommx_highs_adapter import OMMXHighsAdapter

highs_input_class = OMMXHighsAdapter.INPUT_CLASS
assert highs_input_class is not None
assert not highs_input_class.contains(source)
```

HiGHS Adapterは通常の線形制約を受け取れますが、Indicator制約やSOS1制約は直接受け取りません。したがって、この時点の `source` はHiGHS Adapterの入力にはなりません。

## 推奨PolicyでPreparationする

Adapterの `recommended_preparation_policy()` は、その `INPUT_CLASS` に入るために一般的に有用な変換をPolicyとして提案します。HiGHS Adapterの推奨Policyは、Indicator制約とSOS1制約を通常の線形制約へ変換します。

推奨Policyを取得しただけではInstanceは変わりません。どのInstanceを変換するかを決め、`prepare()` を呼ぶのはユーザーです。元のモデルを残すため、ここでは `copy.copy()` で作ったコピーをPreparationします。

```{code-cell} ipython3
import copy

policy = OMMXHighsAdapter.recommended_preparation_policy()

prepared = copy.copy(source)
prepared.prepare(highs_input_class, policy)

# 変換後のInstanceはHiGHS Adapterへ直接渡せる
assert highs_input_class.contains(prepared)

# コピー元のsourceは変えず、preparedだけが変換されている
assert source.indicator_constraints
assert source.sos1_constraints
assert not prepared.indicator_constraints
assert not prepared.sos1_constraints

prepared_solution = OMMXHighsAdapter.solve(prepared)
assert prepared_solution.feasible
assert abs(prepared_solution.objective - 20.0) < 1e-8
prepared_solution.decision_variables_df()
```

## まとめ

- Adapterが直接受け取れるInstanceは、そのまま `solve()` に渡せます。
- `INPUT_CLASS` は、Adapterが変換なしで直接受け取れる入力の集合です。
- 推奨Policyは変換の提案であり、Preparationの実行ではありません。
- 変換するInstanceを選び、`Instance.prepare()` を呼ぶのはユーザーです。
- 元のモデルを残したい場合は、コピーをPreparationしてからAdapterへ渡します。

`INPUT_CLASS` の詳しい読み方は[Adapter の exact input（INPUT_CLASS）](../user_guide/adapter_input_class.md)、Policyの選択とPreparationの詳しい動作は[Instance Preparation と PreparationPolicy](../user_guide/preparation_policy.md)を参照してください。Indicator、OneHot、SOS1の数学的な意味は[特殊制約型](../user_guide/special_constraints.md)、Preparationで取り除いた制約を使って元のモデルを評価できる仕組みは[Removed constraints と実行可能性](../user_guide/removed_constraints.md)で説明しています。
