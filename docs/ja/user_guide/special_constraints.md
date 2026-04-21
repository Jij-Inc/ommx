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

OMMX は通常の制約（{class}`~ommx.v1.Constraint`、等式・不等式を持つ {class}`~ommx.v1.Function`）に加えて、数理最適化で頻出するいくつかの特殊な制約を第一級の制約型として扱います。本ページでは以下の3種類の特殊制約型の定義と使い方を説明します。

- {class}`~ommx.v1.IndicatorConstraint`: バイナリ変数による条件付き制約
- {class}`~ommx.v1.OneHotConstraint`: バイナリ変数集合のうち丁度1つが1
- {class}`~ommx.v1.Sos1Constraint`: 変数集合のうち高々1つが非ゼロ

これらの制約型を通常制約に変換する方法や、ソルバー側の対応状況を扱う Capability モデルについては [Adapter Capability モデルと制約変換](./capability_model.md) を参照してください。

## IndicatorConstraint

**Indicator Constraint** はバイナリ変数 $z$ に対し、$z = 1$ のときのみ制約 $f(x) \leq 0$ あるいは $f(x) = 0$ を課す条件付き制約です。$z = 0$ のときはこの制約は無条件に満たされると見なされます。

{class}`~ommx.v1.IndicatorConstraint` は、既存の {class}`~ommx.v1.Constraint` に対して {meth}`Constraint.with_indicator() <ommx.v1.Constraint.with_indicator>` を呼ぶことで生成できます。

```{code-cell} ipython3
from ommx.v1 import Instance, DecisionVariable, Equality

z = DecisionVariable.binary(0, name="z")
x = DecisionVariable.continuous(1, lower=0, upper=10, name="x")

# z = 1 => x <= 5
ic = (x <= 5).with_indicator(z)
assert ic.indicator_variable_id == 0
assert ic.equality == Equality.LessThanOrEqualToZero
```

{meth}`Instance.from_components <ommx.v1.Instance.from_components>` の `indicator_constraints=` 引数に `dict[int, IndicatorConstraint]` を渡すことでインスタンスに追加できます。

```{code-cell} ipython3
instance = Instance.from_components(
    decision_variables=[z, x],
    objective=x,
    constraints={0: z == 1},       # z を 1 に固定
    indicator_constraints={0: ic}, # z = 1 => x <= 5
    sense=Instance.MAXIMIZE,
)
assert set(instance.indicator_constraints.keys()) == {0}
```

## OneHotConstraint

**One-hot 制約** はバイナリ変数の集合 $\{x_1, \ldots, x_n\}$ に対して $\sum_i x_i = 1$、つまり丁度1つだけが $1$ であることを要求します。

```{code-cell} ipython3
from ommx.v1 import OneHotConstraint

xs = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
oh = OneHotConstraint(variables=[0, 1, 2])
assert oh.variables == [0, 1, 2]
```

`variables` に渡す ID のバイナリ変数はインスタンス構築時の `decision_variables` に含まれている必要があります。数学的には通常の等式制約 $x_0 + x_1 + x_2 - 1 = 0$ と等価ですが、first-class の制約として保持することで、対応するソルバー（MIP系ソルバーの多くは one-hot 制約を直接受け付けます）に効率的に渡すことができます。

```{code-cell} ipython3
instance_oh = Instance.from_components(
    decision_variables=xs,
    objective=sum(xs),
    constraints={},
    one_hot_constraints={0: oh},
    sense=Instance.MAXIMIZE,
)
assert set(instance_oh.one_hot_constraints.keys()) == {0}
```

## Sos1Constraint

**SOS1 (Special Ordered Set type 1)** 制約は変数集合 $\{x_1, \ldots, x_n\}$ の**高々1個**しか非ゼロになれないことを要求します。One-hot との違いは以下の通りです。

- One-hot は $\sum x_i = 1$ を要求するため、丁度1個が非ゼロ。
- SOS1 は高々1個が非ゼロで、全て $0$ であることも許容。
- SOS1 の変数はバイナリとは限らない（連続変数でも良い）。

```{code-cell} ipython3
from ommx.v1 import Sos1Constraint

ys = [DecisionVariable.continuous(i, lower=0, upper=10, name="y", subscripts=[i]) for i in range(3, 6)]
s1 = Sos1Constraint(variables=[3, 4, 5])
assert s1.variables == [3, 4, 5]
```

```{code-cell} ipython3
instance_s1 = Instance.from_components(
    decision_variables=ys,
    objective=sum(ys),
    constraints={},
    sos1_constraints={0: s1},
    sense=Instance.MAXIMIZE,
)
assert set(instance_s1.sos1_constraints.keys()) == {0}
```

## 制約種別ごとに独立したID空間

OMMX では、通常制約・Indicator・OneHot・SOS1 の4つはそれぞれ**独立したID空間**を持ちます。Rust SDK では `ConstraintID`, `IndicatorConstraintID`, `OneHotConstraintID`, `Sos1ConstraintID` という別の型として定義されており、Python SDK でも {meth}`Instance.from_components <ommx.v1.Instance.from_components>` に渡す4つの dict のキーはそれぞれ別物として扱われます。

したがって、例えば「通常制約 ID=1」と「Indicator 制約 ID=1」は衝突せず、別々の制約として共存できます。

```{code-cell} ipython3
z2 = DecisionVariable.binary(10, name="z2")
x2 = DecisionVariable.continuous(11, lower=0, upper=10, name="x2")

instance_mix = Instance.from_components(
    decision_variables=[z2, x2] + xs + ys,
    objective=x2,
    constraints={1: z2 == 1},                              # 通常制約 ID=1
    indicator_constraints={1: (x2 <= 5).with_indicator(z2)}, # Indicator ID=1
    one_hot_constraints={1: OneHotConstraint(variables=[0, 1, 2])}, # OneHot ID=1
    sos1_constraints={1: Sos1Constraint(variables=[3, 4, 5])},      # SOS1 ID=1
    sense=Instance.MAXIMIZE,
)

# 4 種の dict それぞれが ID=1 の制約を独立に保持している
assert set(instance_mix.constraints.keys()) == {1}
assert set(instance_mix.indicator_constraints.keys()) == {1}
assert set(instance_mix.one_hot_constraints.keys()) == {1}
assert set(instance_mix.sos1_constraints.keys()) == {1}
```

ただし、特殊制約型を通常制約に変換する（[Capability モデルと変換](./capability_model.md) 参照）と、新たに生成される通常制約は **`Constraint` 側の ID 空間**から割り当てられます。変換後に衝突する可能性があるのは通常制約の ID のみです。

## 評価結果の参照

インスタンスを解いて得られた {class}`~ommx.v1.Solution` や {class}`~ommx.v1.SampleSet` は、通常制約と同様に特殊制約それぞれに対する DataFrame アクセサを提供します。

| 制約型 | アクセサ（Solution） |
|---|---|
| 通常制約 | {attr}`~ommx.v1.Solution.constraints_df` |
| Indicator | {attr}`~ommx.v1.Solution.indicator_constraints_df` |
| OneHot | {attr}`~ommx.v1.Solution.one_hot_constraints_df` |
| SOS1 | {attr}`~ommx.v1.Solution.sos1_constraints_df` |

Indicator 制約の DataFrame には、`indicator_active` というカラムが含まれます。これにより「インジケータが OFF だった（制約は自明に満たされた）」ケースと「インジケータが ON で制約が本当に満たされた」ケースを区別できます。なお、Indicator 制約には双対変数の値は定義されない（条件付き制約に対する双対値は一般に well-defined ではない）ため、`dual_variable` は含まれません。

### removed_reasons_df の分離

通常制約の `removed_reason` は {attr}`~ommx.v1.Solution.constraints_df` のカラムとしては持たず、{attr}`~ommx.v1.Solution.removed_reasons_df` という別テーブルとして提供されます。必要なら join して使います。

```python
df = solution.constraints_df.join(solution.removed_reasons_df)
```

Indicator・OneHot・SOS1 についても、それぞれ対応する `indicator_removed_reasons_df` / `one_hot_removed_reasons_df` / `sos1_removed_reasons_df` が {class}`~ommx.v1.Solution` および {class}`~ommx.v1.SampleSet` で利用できます。

## Relax / Restore

{class}`~ommx.v1.IndicatorConstraint` は、通常制約と同じ relax / restore ワークフローを持ちます。

- {meth}`Instance.relax_indicator_constraint() <ommx.v1.Instance.relax_indicator_constraint>`: Indicator 制約を「緩和」（無効化）し、理由文字列を記録します。緩和された制約は `removed_indicator_constraints` に移動します。
- {meth}`Instance.restore_indicator_constraint() <ommx.v1.Instance.restore_indicator_constraint>`: 緩和した Indicator 制約を元に戻します。インジケータ変数が既に値を代入されている・固定されている場合は失敗します。

OneHot / SOS1 については、`removed_one_hot_constraints` / `removed_sos1_constraints` への移動は [Capability モデルと制約変換](./capability_model.md) で扱う変換 API によって行われます。
