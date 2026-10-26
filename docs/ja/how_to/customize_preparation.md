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

# Preparationで変換する制約族を選ぶ

このページは、[自動PreparationのTutorial](../tutorial/prepare_instance_for_adapter.md)を読んだ利用者向けです。**アプリケーションで許可する変換を明示したい場合は、Policyを設定し、自分でPreparationしてから厳格なAPIへ渡します。** 通常の求解だけなら`solve()`を使ってください。

ここでは「OneHotだけを通常制約へloweringし、SOS1は専用の制約としてソルバーへ渡す」という方針を固定します。PySCIPOpt AdapterはSOS1を直接扱えますが、OneHotは直接扱いません。現在の推奨Policyも同じ選択ですが、ここでは推奨の内容に依存せず、許可する制約族をアプリケーションで指定します。

## モデルを作る

[Python SDKとAdapterのインストール](../getting_started/install.md)で用意したv3の環境を使います。OneHotは「ちょうど1つが1」、SOS1は「高々1つが非ゼロ」という制約です。

```{code-cell} ipython3
from ommx import Instance, OneHotConstraint, Sos1Constraint

source = Instance.maximize()
primary = [source.new_binary("primary", subscripts=[i]) for i in range(2)]
optional = [source.new_binary("optional", subscripts=[i]) for i in range(2)]
source.objective = (
    10 * primary[0] + 8 * primary[1]
    + 4 * optional[0] + 3 * optional[1]
)
source.add_one_hot_constraint(OneHotConstraint(variables=primary))
source.add_sos1_constraint(Sos1Constraint(variables=optional))
```

## Policyを選び、copyへ適用する

推奨Policyを取得しただけでは、Instanceは変わりません。`special_constraints`を設定し直すことで、loweringを許可する制約族をOneHotに限定します。

```{code-cell} ipython3
import copy

from ommx import SpecialConstraintKind, SpecialConstraintPreparation
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

policy = OMMXPySCIPOptAdapter.recommended_preparation_policy()
policy.special_constraints = SpecialConstraintPreparation.lower_special_constraints(
    kinds={SpecialConstraintKind.OneHot}
)

prepared = copy.copy(source)
prepared.prepare(OMMXPySCIPOptAdapter.INPUT_CLASS, policy)

assert OMMXPySCIPOptAdapter.INPUT_CLASS.contains(prepared)
assert not prepared.one_hot_constraints
assert prepared.sos1_constraints
assert source.one_hot_constraints  # 元のモデルは変えていない
```

OneHotは通常の線形等式に変わり、SOS1はそのまま残っています。変換元のOneHotは[removed constraint](../user_guide/removed_constraints.md)として保持されます。

Policyは「必ず実行する操作の一覧」ではありません。targetの`INPUT_CLASS`へ入るとPreparationは停止します。このモデルにはOneHotがあるためloweringが必要ですが、最初からtargetに入るモデルでは、設定した変換も実行されません。

## 自分で準備したInstanceを解く

`solve_without_preparation()`は追加のPreparationを行いません。先ほど設定したPolicyの結果を、そのままソルバーへ渡せます。

```{code-cell} ipython3
solution = OMMXPySCIPOptAdapter.solve_without_preparation(prepared)

assert solution.feasible
assert abs(solution.objective - 14.0) < 1e-8
solution.decision_variables_df()
```

この例で`solve(source)`を使えば、編集した`policy`ではなくAdapterから新たに取得した推奨Policyが使われます。独自のPolicyを使う経路では、`prepare()`と`solve_without_preparation()`を組にしてください。

Policyの各phaseや到達できない場合の扱いは[Instance PreparationとPreparationPolicy](../user_guide/preparation_policy.md)、各loweringの適用条件や数式は{meth}`~ommx.Instance.lower_special_constraints`からAPI Referenceを参照してください。
