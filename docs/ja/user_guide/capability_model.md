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

# Adapter の入力 class と legacy な特殊制約 lowering

OMMX では、従来 Adapter Capability として一緒に説明されていた次の2つの概念を分けて扱います。

- {class}`~ommx.InstanceClass` は、具体的な `Instance` 値の集合です。Adapter は構造的な入力条件を `INPUT_CLASS` で宣言し、その後に Adapter 固有の precondition を評価して applicability を判定します。
- legacy な `AdditionalCapability` API は、明示的な lowering で維持する特殊制約 family を選びます。入力 class の宣言でも、Adapter applicability の証明でもありません。

本ページでは以下を説明します。

- `InstanceClass` の membership と Adapter applicability
- legacy な特殊制約 family selector としての {class}`~ommx.AdditionalCapability` と {attr}`Instance.required_capabilities <ommx.Instance.required_capabilities>`
- {meth}`Instance.reduce_capabilities() <ommx.Instance.reduce_capabilities>` による明示的な lowering
- 手動で通常制約に変換するための API
- 変換結果の監査

## Instance class と Adapter applicability

`InstanceClass` は、条件を論理積でまとめた完全な {class}`~ommx.InstanceClassClause` の有限和です。membership は入力値そのものに対して評価され、入力の変更や preparation は行いません。

```{code-cell} ipython3
from ommx import DegreeBound, InstanceClass, InstanceClassClause, Kind, Sense

binary_linear_with_one_hot = InstanceClass(
    [
        InstanceClassClause(
            label="binary-linear-with-one-hot",
            allowed_variable_kinds={Kind.Binary},
            objective_degree_bound=DegreeBound.at_most(1),
            allowed_senses={Sense.Maximize},
            allows_one_hot=True,
        )
    ]
)
```

Adapter は applicability の最初の条件を `INPUT_CLASS` として宣言します。構造化された結果を得るには `check_applicability()`、membership または Adapter 固有の precondition が満たされない場合に例外を送出するには `require_applicable()` を使います。明示的な preparation で別の入力値を作った場合は、その値で applicability を再評価します。

## AdditionalCapability と required_capabilities

{class}`~ommx.AdditionalCapability` は、通常制約を超えた「追加の制約型サポート」を表す列挙型です。

| Capability | 対応する制約型 |
|---|---|
| `AdditionalCapability.Indicator` | {class}`~ommx.IndicatorConstraint` |
| `AdditionalCapability.OneHot` | {class}`~ommx.OneHotConstraint` |
| `AdditionalCapability.Sos1` | {class}`~ommx.Sos1Constraint` |

{attr}`Instance.required_capabilities <ommx.Instance.required_capabilities>` は、その {class}`~ommx.Instance` が **現在保持している特殊制約** に対応する `AdditionalCapability` の集合を返します。通常制約しか使っていない場合は空集合です。

```{code-cell} ipython3
from ommx import Instance, DecisionVariable, OneHotConstraint, AdditionalCapability

xs = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]

instance = Instance.from_components(
    decision_variables=xs,
    objective=sum(xs),
    constraints={},
    one_hot_constraints={0: OneHotConstraint(variables=xs)},
    sense=Instance.MAXIMIZE,
)
assert instance.required_capabilities == {AdditionalCapability.OneHot}
assert binary_linear_with_one_hot.contains(instance)
```

## Adapter の legacy lowering selector

legacy な基底 class の lowering path を使う Adapter は、維持する active な特殊制約 family を `ADDITIONAL_CAPABILITIES` で選びます。

```python
from ommx import AdditionalCapability
from ommx.adapter import SolverAdapter

class LegacyLoweringAdapter(SolverAdapter):
    ADDITIONAL_CAPABILITIES = frozenset({AdditionalCapability.Indicator})
```

Adapter のコンストラクタで `super().__init__(instance)` が呼ばれると、`ADDITIONAL_CAPABILITIES` に含まれない active な特殊制約 family は通常制約へ変換されます。この mutating operation は lowering にすぎず、変換後の instance が `INPUT_CLASS` に属することや Adapter 固有の precondition を満たすことは保証しません。

デフォルトでは `ADDITIONAL_CAPABILITIES = frozenset()` なので、active な特殊制約 family はすべて lowering されます。既存 Adapter は、backend path が直接扱う family を維持する場合があります。

## reduce_capabilities による明示的な lowering

`super().__init__` の内部で呼ばれているのが {meth}`Instance.reduce_capabilities() <ommx.Instance.reduce_capabilities>` です。このメソッドは `preserved` として渡された集合に含まれない特殊制約 family を、対応する変換 API（後述）を使って通常制約に変換します。

```{code-cell} ipython3
converted = instance.reduce_capabilities(preserved=set())
assert converted == {AdditionalCapability.OneHot}
```

```{code-cell} ipython3
assert instance.required_capabilities == set()
assert instance.one_hot_constraints == {}
assert len(instance.constraints) == 1
```

One-hot 制約が除去され、その代わりに通常の等式制約 $x_0 + x_1 + x_2 - 1 = 0$ が1つ追加されたことが分かります。`reduce_capabilities` はインスタンスを in-place に変更します。成功時、`required_capabilities` は `preserved` の部分集合になります。変換が必要なかった場合は空集合を返します。得られた値に対して `INPUT_CLASS` の membership または Adapter applicability を再評価してください。

## 手動変換 API

`reduce_capabilities` は内部的に、制約型別の以下の変換 API を組み合わせて実装されています。ユーザーがこれらを直接呼ぶことも可能です。

### One-hot → 等式制約

{meth}`Instance.convert_one_hot_to_constraint(one_hot_id) <ommx.Instance.convert_one_hot_to_constraint>` は、OneHot 制約を数学的に等価な等式制約 $x_1 + \ldots + x_n - 1 = 0$ に書き換えます。

```{code-cell} ipython3
instance2 = Instance.from_components(
    decision_variables=xs,
    objective=sum(xs),
    constraints={},
    one_hot_constraints={1: OneHotConstraint(variables=xs)},
    sense=Instance.MAXIMIZE,
)
new_id = instance2.convert_one_hot_to_constraint(1)
assert isinstance(new_id, int)
assert set(instance2.constraints.keys()) == {new_id}
assert instance2.one_hot_constraints == {}
```

全ての OneHot 制約を一括変換するには {meth}`~ommx.Instance.convert_all_one_hots_to_constraints` を使います。

### SOS1 → Big-M 制約

{meth}`Instance.convert_sos1_to_constraints(sos1_id) <ommx.Instance.convert_sos1_to_constraints>` は、SOS1 制約を Big-M 法による通常制約に変換します。各変数 $x_i \in [l_i, u_i]$ に対して、以下のルールで変換されます。

1. $x_i$ が $[0, 1]$ のバイナリ変数ならそのままインジケータとして再利用する。
2. そうでなければ新たなバイナリ変数 $y_i$ を導入し、$x_i - u_i y_i \leq 0$ および $l_i y_i - x_i \leq 0$ を追加する（$u_i = 0$ や $l_i = 0$ の自明側は省略）。
3. 最後に濃度制約 $\sum_i y_i - 1 \leq 0$ を追加する。

```{code-cell} ipython3
from ommx import Sos1Constraint

ys = [DecisionVariable.binary(i, name="y", subscripts=[i]) for i in range(3)]
instance3 = Instance.from_components(
    decision_variables=ys,
    objective=sum(ys),
    constraints={},
    sos1_constraints={1: Sos1Constraint(variables=ys)},
    sense=Instance.MAXIMIZE,
)
new_ids = instance3.convert_sos1_to_constraints(1)
# バイナリ変数のみの SOS1 は濃度制約 sum(x_i) - 1 <= 0 1本に変換される
assert len(new_ids) == 1
assert set(instance3.constraints.keys()) == set(new_ids)
assert instance3.sos1_constraints == {}
```

全 SOS1 の一括変換は {meth}`~ommx.Instance.convert_all_sos1_to_constraints` です。変数の境界が非有限だったり、$0$ を含まない場合は変換前にエラーを返し、インスタンスは変更されません。

### Indicator → Big-M 制約

{meth}`Instance.convert_indicator_to_constraint(indicator_id) <ommx.Instance.convert_indicator_to_constraint>` は、Indicator 制約 $y = 1 \Rightarrow f(x) \leq 0$ を、$f(x)$ の上下限から計算した Big-M を用いて書き換えます。SOS1 と違い新しいインジケータ変数は導入されず、`IndicatorConstraint` が元々持っているインジケータ変数がそのまま $y$ として使われます。

$$
f(x) + u y - u \leq 0, \qquad -f(x) - l y + l \leq 0
$$

ここで $u \geq \sup f(x)$, $l \leq \inf f(x)$ です。

- 不等式 $\leq$ のみの Indicator では上側だけを考慮し、$u > 0$ の時のみ追加します（$u \leq 0$ なら変数境界だけで自明に満たされるため省略）。
- 等式 $= 0$ の Indicator では上下両側を独立に判定し、$u > 0$ なら上側、$l < 0$ なら下側を追加します。

全 Indicator の一括変換は {meth}`~ommx.Instance.convert_all_indicators_to_constraints` です。$f(x)$ の必要な側の境界が非有限だったり、semi-continuous / semi-integer 変数を含む場合は変換前にエラーを返し、インスタンスは変更されません。

## 変換結果の監査

変換元の特殊制約は破棄されず、以下の `removed_*_constraints` に「除去済」として保持されます。

| 元の制約型 | 除去先 | DataFrame |
|---|---|---|
| OneHotConstraint | {attr}`~ommx.Instance.removed_one_hot_constraints` | `instance.constraints_df(kind="one_hot", removed=True)` |
| Sos1Constraint | {attr}`~ommx.Instance.removed_sos1_constraints` | `instance.constraints_df(kind="sos1", removed=True)` |
| IndicatorConstraint | {attr}`~ommx.Instance.removed_indicator_constraints` | `instance.constraints_df(kind="indicator", removed=True)` |

`removed=True` を付けると、active と removed が同じ DataFrame に並び、`removed_reason` / `removed_reason.{key}` カラムが自動的に追加されるので、active 行と removed 行を見分けることができます。

それぞれのエントリ（{class}`~ommx.RemovedOneHotConstraint` / {class}`~ommx.RemovedSos1Constraint` / {class}`~ommx.RemovedIndicatorConstraint`）には `removed_reason` 文字列（例: `"ommx.Instance.convert_one_hot_to_constraint"`）が記録され、`removed_reason_parameters` に変換で新しく生成された通常制約の ID が格納されます。ID のキー名と形式は制約型ごとに異なります:

- **OneHot**: `constraint_id` キーに単一の ID
- **SOS1**: `constraint_ids` キーにカンマ区切りの ID リスト
- **Indicator**: `constraint_ids` キーにカンマ区切りの ID リスト（Big-M 両側が省略された場合は空）

```{code-cell} ipython3
removed = instance2.removed_one_hot_constraints
assert set(removed.keys()) == {1}
```

さらに、各変換で生成された通常制約は {attr}`Constraint.provenance <ommx.Constraint.provenance>` プロパティで変換元の情報を保持します。各 {class}`~ommx.Provenance` エントリは変換元の種別を表す {attr}`~ommx.Provenance.kind`（{class}`~ommx.ProvenanceKind`）と元の ID {attr}`~ommx.Provenance.original_id` を持つので、得られた通常制約がどの特殊制約型の何番から変換されたかを後から辿ることができます。

```{code-cell} ipython3
from ommx import ProvenanceKind

# 先ほど convert_one_hot_to_constraint(1) で生成された通常制約
for cid, c in instance2.constraints.items():
    for p in c.provenance:
        assert p.kind == ProvenanceKind.OneHotConstraint
        assert p.original_id == 1
```

## まとめ

| やりたいこと | 使う API |
|---|---|
| Adapter 入力の構造的な集合を記述する | {class}`~ommx.InstanceClass` |
| Adapter applicability の最初の条件を宣言する | `INPUT_CLASS` |
| membership と Adapter 固有の precondition を検査する | `check_applicability()` / `require_applicable()` |
| active な legacy 特殊制約 family を調べる | {attr}`Instance.required_capabilities <ommx.Instance.required_capabilities>` |
| 維持しない特殊制約を明示的に lowering する | {meth}`Instance.reduce_capabilities <ommx.Instance.reduce_capabilities>` |
| 個別に通常制約に変換する | `convert_*_to_constraint(s)` / `convert_all_*_to_constraints` |
| 変換履歴を確認する | `instance.constraints_df(kind=..., removed=True)` / `solution.constraints_df(kind=..., include=("...","removed_reason"))` |
