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

# Adapter Capability モデルと制約変換

OMMX は [特殊制約型](./special_constraints.md) として IndicatorConstraint, OneHotConstraint, Sos1Constraint を第一級で扱いますが、すべてのソルバーがこれらを直接受け付けるわけではありません。ソルバー毎の対応状況の違いを統一的に扱うため、OMMX は **Adapter Capability モデル** を提供しています。

本ページでは以下を説明します。

- {class}`~ommx.v1.AdditionalCapability` と {attr}`Instance.required_capabilities <ommx.v1.Instance.required_capabilities>` による必要機能の表現
- Adapter が `ADDITIONAL_CAPABILITIES` でサポート機能を宣言する仕組み
- {meth}`Instance.reduce_capabilities() <ommx.v1.Instance.reduce_capabilities>` による自動変換
- 手動で通常制約に変換するための API
- 変換結果の監査

## AdditionalCapability と required_capabilities

{class}`~ommx.v1.AdditionalCapability` は、通常制約を超えた「追加の制約型サポート」を表す列挙型です。

| Capability | 対応する制約型 |
|---|---|
| `AdditionalCapability.Indicator` | {class}`~ommx.v1.IndicatorConstraint` |
| `AdditionalCapability.OneHot` | {class}`~ommx.v1.OneHotConstraint` |
| `AdditionalCapability.Sos1` | {class}`~ommx.v1.Sos1Constraint` |

{attr}`Instance.required_capabilities <ommx.v1.Instance.required_capabilities>` は、その {class}`~ommx.v1.Instance` が **現在保持している非標準制約型** に対応する `AdditionalCapability` の集合を返します。通常制約しか使っていない場合は空集合です。

```{code-cell} ipython3
from ommx.v1 import Instance, DecisionVariable, OneHotConstraint

xs = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]

instance = Instance.from_components(
    decision_variables=xs,
    objective=sum(xs),
    constraints={},
    one_hot_constraints={0: OneHotConstraint(variables=[0, 1, 2])},
    sense=Instance.MAXIMIZE,
)
instance.required_capabilities
```

## Adapter 側の宣言

各 OMMX Adapter は、サポートする Capability を `ADDITIONAL_CAPABILITIES` クラス属性で宣言します。

```python
from ommx.v1 import AdditionalCapability
from ommx.adapter import SolverAdapter

class MySolverAdapter(SolverAdapter):
    ADDITIONAL_CAPABILITIES = frozenset({AdditionalCapability.Indicator})
```

このとき、Adapter のコンストラクタで `super().__init__(instance)` が呼ばれると、**`ADDITIONAL_CAPABILITIES` に含まれない制約型は自動的に通常制約へ変換** されます。つまり Adapter の実装者は、`ADDITIONAL_CAPABILITIES` で宣言した制約型と通常制約さえ扱えれば、どんなインスタンスも受け付けられるようになります。

デフォルトでは `ADDITIONAL_CAPABILITIES = frozenset()` なので、全ての特殊制約型が自動変換されます。逆に全てサポートを宣言することもできます（例えば PySCIPOpt Adapter は現在 Indicator と SOS1 をサポート宣言しています）。

## reduce_capabilities による自動変換

`super().__init__` の内部で呼ばれているのが {meth}`Instance.reduce_capabilities() <ommx.v1.Instance.reduce_capabilities>` です。このメソッドは `supported` として渡された Capability 集合に含まれない制約型を、対応する変換 API（後述）を使って通常制約に変換します。

```{code-cell} ipython3
from ommx.v1 import AdditionalCapability

converted = instance.reduce_capabilities(supported=frozenset())
converted
```

```{code-cell} ipython3
instance.required_capabilities, instance.one_hot_constraints, instance.constraints
```

One-hot 制約が除去され、その代わりに通常の等式制約 $x_0 + x_1 + x_2 - 1 = 0$ が追加されたことが分かります。

`reduce_capabilities` はインスタンスを in-place に変更します。成功時、`required_capabilities` は `supported` の部分集合になります。変換が必要なかった場合は空集合を返します。

## 手動変換 API

`reduce_capabilities` は内部的に、制約型別の以下の変換 API を組み合わせて実装されています。ユーザーがこれらを直接呼ぶことも可能です。

### One-hot → 等式制約

{meth}`Instance.convert_one_hot_to_constraint(one_hot_id) <ommx.v1.Instance.convert_one_hot_to_constraint>` は、OneHot 制約を数学的に等価な等式制約 $x_1 + \ldots + x_n - 1 = 0$ に書き換えます。

```{code-cell} ipython3
instance2 = Instance.from_components(
    decision_variables=xs,
    objective=sum(xs),
    constraints={},
    one_hot_constraints={1: OneHotConstraint(variables=[0, 1, 2])},
    sense=Instance.MAXIMIZE,
)
new_id = instance2.convert_one_hot_to_constraint(1)
new_id, instance2.constraints
```

全ての OneHot 制約を一括変換するには {meth}`~ommx.v1.Instance.convert_all_one_hots_to_constraints` を使います。

### SOS1 → Big-M 制約

{meth}`Instance.convert_sos1_to_constraints(sos1_id) <ommx.v1.Instance.convert_sos1_to_constraints>` は、SOS1 制約を Big-M 法による通常制約に変換します。各変数 $x_i \in [l_i, u_i]$ に対して、以下のルールで変換されます。

1. $x_i$ が $[0, 1]$ のバイナリ変数ならそのままインジケータとして再利用する。
2. そうでなければ新たなバイナリ変数 $y_i$ を導入し、$x_i - u_i y_i \leq 0$ および $l_i y_i - x_i \leq 0$ を追加する（$u_i = 0$ や $l_i = 0$ の自明側は省略）。
3. 最後に濃度制約 $\sum_i y_i - 1 \leq 0$ を追加する。

```{code-cell} ipython3
from ommx.v1 import Sos1Constraint

ys = [DecisionVariable.binary(i, name="y", subscripts=[i]) for i in range(3)]
instance3 = Instance.from_components(
    decision_variables=ys,
    objective=sum(ys),
    constraints={},
    sos1_constraints={1: Sos1Constraint(variables=[0, 1, 2])},
    sense=Instance.MAXIMIZE,
)
new_ids = instance3.convert_sos1_to_constraints(1)
new_ids, instance3.constraints
```

全 SOS1 の一括変換は {meth}`~ommx.v1.Instance.convert_all_sos1_to_constraints` です。変数の境界が非有限だったり、$0$ を含まない場合は変換前にエラーを返し、インスタンスは変更されません。

### Indicator → Big-M 制約

{meth}`Instance.convert_indicator_to_constraint(indicator_id) <ommx.v1.Instance.convert_indicator_to_constraint>` は、Indicator 制約 $y = 1 \Rightarrow f(x) \leq 0$ を、$f(x)$ の上下限から計算した Big-M を用いて書き換えます。SOS1 と違い新しいインジケータ変数は導入されず、`IndicatorConstraint` が元々持っているインジケータ変数がそのまま $y$ として使われます。

$$
f(x) + u y - u \leq 0, \qquad -f(x) - l y + l \leq 0
$$

ここで $u \geq \sup f(x)$, $l \leq \inf f(x)$ です。

- 不等式 $\leq$ のみの Indicator では上側だけを考慮し、$u > 0$ の時のみ追加します（$u \leq 0$ なら変数境界だけで自明に満たされるため省略）。
- 等式 $= 0$ の Indicator では上下両側を独立に判定し、$u > 0$ なら上側、$l < 0$ なら下側を追加します。

全 Indicator の一括変換は {meth}`~ommx.v1.Instance.convert_all_indicators_to_constraints` です。$f(x)$ の必要な側の境界が非有限だったり、semi-continuous / semi-integer 変数を含む場合は変換前にエラーを返し、インスタンスは変更されません。

## 変換結果の監査

変換元の特殊制約は破棄されず、以下の `removed_*_constraints` に「除去済」として保持されます。

| 元の制約型 | 除去先 | DataFrame |
|---|---|---|
| OneHotConstraint | {attr}`~ommx.v1.Instance.removed_one_hot_constraints` | {attr}`~ommx.v1.Instance.removed_one_hot_constraints_df` |
| Sos1Constraint | {attr}`~ommx.v1.Instance.removed_sos1_constraints` | {attr}`~ommx.v1.Instance.removed_sos1_constraints_df` |
| IndicatorConstraint | {attr}`~ommx.v1.Instance.removed_indicator_constraints` | {attr}`~ommx.v1.Instance.removed_indicator_constraints_df` |

それぞれのエントリには `reason` 文字列（例: `"ommx.Instance.convert_one_hot_to_constraint"`）と、変換で新しく生成された通常制約の ID が記録されます。

```{code-cell} ipython3
instance2.removed_one_hot_constraints
```

さらに、各変換で生成された通常制約には `Provenance::OneHotConstraint(original_id)` のようなプロヴェナンス情報がメタデータに記録されます。したがって、得られた通常制約がどの特殊制約型から変換されて来たかを後から辿ることができます。

## まとめ

| やりたいこと | 使う API |
|---|---|
| インスタンスが必要とする機能を調べる | {attr}`Instance.required_capabilities <ommx.v1.Instance.required_capabilities>` |
| Adapter でサポート機能を宣言する | `ADDITIONAL_CAPABILITIES` クラス属性 |
| 未サポートの特殊制約を一括で通常制約に変換する | {meth}`Instance.reduce_capabilities <ommx.v1.Instance.reduce_capabilities>` |
| 個別に通常制約に変換する | `convert_*_to_constraint(s)` / `convert_all_*_to_constraints` |
| 変換履歴を確認する | `removed_*_constraints` / `*_df` |
