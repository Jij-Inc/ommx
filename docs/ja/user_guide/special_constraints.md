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

# 特殊制約型

OMMXは通常の等式・不等式制約に加えて、Indicator、OneHot、SOS1を独立した制約型として保持できます。これにより、対応するソルバーにはその意味を保ったまま専用APIへ渡し、対応しないソルバー向けには通常制約へ変換できます。

```{important}
このページで扱うのは、各制約型の数学的な意味とInstance内の扱いです。loweringで生成される式、成立条件、戻り値、例外、atomicityなどの正確な契約は、各変換メソッドのAPI Referenceを参照してください。
```

## 3つの特殊制約

| 制約型 | 数学的な意味 | 主な用途 |
|---|---|---|
| {class}`~ommx.IndicatorConstraint` | Binary変数 $z$ が $1$ のときだけ、等式または不等式制約を有効にする | 条件付き制約 |
| {class}`~ommx.OneHotConstraint` | Binary変数集合のうち、丁度1つが $1$ | 選択肢から1つを選ぶ |
| {class}`~ommx.Sos1Constraint` | 変数集合のうち、高々1つが非ゼロ | 複数変数の同時利用を禁止する |

OneHotとSOS1は似ていますが、OneHotは「丁度1つ」であり全変数が $0$ の状態を許しません。SOS1は「高々1つ」なので全変数が $0$ でも構いません。また、OneHotの構成変数はBinaryですが、SOS1の構成変数はBinaryに限りません。

## Instanceに追加する

Indicatorは通常の{class}`~ommx.Constraint`に{meth}`Constraint.with_indicator() <ommx.Constraint.with_indicator>`を適用して作ります。OneHotとSOS1は、構成変数を指定して専用の制約型を作ります。

```{code-cell} ipython3
from ommx import Instance, OneHotConstraint, Sos1Constraint

instance = Instance.maximize()
enabled = instance.new_binary("enabled")
choices = [instance.new_binary("choice", subscripts=[i]) for i in range(3)]

instance.objective = sum(choices)

# enabled = 1 のときだけ choices[0] + choices[1] <= 1
instance.add_indicator_constraint(
    (choices[0] + choices[1] <= 1).with_indicator(enabled)
)

# 3つのうち丁度1つが1
instance.add_one_hot_constraint(OneHotConstraint(variables=choices))

# choices[1]とchoices[2]のうち高々1つが非ゼロ
instance.add_sos1_constraint(Sos1Constraint(variables=choices[1:]))
```

Instanceは制約の追加時に、参照された決定変数が存在することと、Indicatorのindicator変数やOneHotの構成変数がBinaryであることなどを検査します。

## 制約型ごとのID空間

通常制約、Indicator、OneHot、SOS1は、それぞれ独立したID空間を持ちます。例えば、通常制約のID `1`とIndicator制約のID `1`は別の制約です。制約を参照するときは、整数IDだけでなく制約型も区別してください。

各制約型のactiveなentryは次のpropertyから参照できます。

| 制約型 | Property |
|---|---|
| 通常制約 | {attr}`~ommx.Instance.constraints` |
| Indicator | {attr}`~ommx.Instance.indicator_constraints` |
| OneHot | {attr}`~ommx.Instance.one_hot_constraints` |
| SOS1 | {attr}`~ommx.Instance.sos1_constraints` |

## Native supportとlowering

どの特殊制約を変換なしで受け取れるかはAdapterごとに異なり、Adapterの`INPUT_CLASS`がそのexactな条件を宣言します。例えばPySCIPOpt Adapterは線形IndicatorとSOS1をSCIPの専用APIへ渡しますが、OneHotは直接受け取りません。

- 対応Adapterに特殊制約をそのまま渡す例は[PySCIPOpt Adapterで特殊制約をそのまま解く](../tutorial/solve_special_constraints_with_pyscipopt_adapter.md)を参照してください。
- 特殊制約を受け取らないAdapter向けに通常制約へ変換する例は[Adapter向けにInstanceを準備する](../tutorial/prepare_instance_for_adapter.md)を参照してください。
- `INPUT_CLASS`とPreparationの責任境界は[Adapterのexact input（INPUT_CLASS）](./adapter_input_class.md)と[Instance PreparationとPreparationPolicy](./preparation_policy.md)を参照してください。

個別のloweringを明示的に実行するAPIは次の通りです。

| 制約型 | 1制約の変換 | activeな全制約の変換 |
|---|---|---|
| Indicator | {meth}`~ommx.Instance.convert_indicator_to_constraint` | {meth}`~ommx.Instance.convert_all_indicators_to_constraints` |
| OneHot | {meth}`~ommx.Instance.convert_one_hot_to_constraint` | {meth}`~ommx.Instance.convert_all_one_hots_to_constraints` |
| SOS1 | {meth}`~ommx.Instance.convert_sos1_to_constraints` | {meth}`~ommx.Instance.convert_all_sos1_to_constraints` |

各変換で生成される制約の式、変数の追加・再利用、成立条件、返されるID、失敗時のmutation contractは、上のAPI Referenceが所有します。Adapterの標準workflowでは個別APIを直接組み合わせず、推奨Policyと{meth}`~ommx.Instance.prepare`を使います。

loweringされた元の制約はremovedなentryとしてInstanceに残ります。active/removedのlifecycle、変換理由、provenance、変換前の制約を含むfeasibilityは[Removed constraintsと実行可能性](./removed_constraints.md)で説明しています。

## 評価結果を参照する

{class}`~ommx.Solution`と{class}`~ommx.SampleSet`では、`constraints_df()`の`kind=`を切り替えて制約型ごとの評価結果を参照できます。

```python
solution.constraints_df()                  # regular
solution.constraints_df(kind="indicator")
solution.constraints_df(kind="one_hot")
solution.constraints_df(kind="sos1")
```

Indicatorの評価結果は、indicatorがOFFで制約が無条件に満たされた場合と、ONで制約本体が満たされた場合を`indicator_active`で区別します。
