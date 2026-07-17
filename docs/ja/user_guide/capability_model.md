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

# Adapter の入力 class と明示的な特殊制約 lowering

OMMX では、従来 Adapter Capability として一緒に説明されていた次の2つの概念を分けて扱います。

- {class}`~ommx.InstanceClass` は、具体的な `Instance` 値の集合です。Adapter は構造的な入力条件を `INPUT_CLASS` で宣言し、その後に Adapter 固有の precondition を評価して applicability を判定します。
- {meth}`Instance.reduce_capabilities() <ommx.Instance.reduce_capabilities>` は、Instance 上で維持対象に選ばれなかった特殊制約 family を明示的に lowering します。入力 class の宣言でも、Adapter applicability の証明でもありません。

以下の3種類の API は似ていますが、責務が明確に分かれています。

- `InstanceClass` の membership と Adapter applicability
- 特殊制約 family selector としての {class}`~ommx.AdditionalCapability` と {attr}`Instance.required_capabilities <ommx.Instance.required_capabilities>`
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
| {class}`~ommx.AdapterCapabilities` / {class}`~ommx.CapabilityProfile` | Adapter の native 入力の宣言と model との比較 |
| {class}`~ommx.SpecialConstraintKind` / {meth}`~ommx.Instance.lower_special_constraints` | 明示的に実行する特殊制約 lowering の選択 |
| `ommx.v2.Feature` / `required_features` | serialized semantics を reader が安全に deserialize できるかの判定 |

制約の lowering、Integer の Binary encoding、sense の反転、有限 penalty
の追加で model を受理可能にできる場合でも、それらは native capability
ではなく preparation step です。また `ommx.v2.Feature` は wire-format の
Forward Compatibility のための仕組みであり、deserialize した model を
solver が最適化できるかは表しません。

## Model 全体の requirements を導出する

{meth}`Instance.solver_requirements() <ommx.Instance.solver_requirements>` は、active な
solver-facing model shape を導出します。次の情報が含まれます。

- {class}`~ommx.Kind` ごとにまとめた used variable ID
- 目的関数の次数と {class}`~ommx.Sense`
- active な通常制約ごとの relation と次数
- active な Indicator 制約ごとの relation と body の次数
- active な OneHot と SOS1 の制約 ID

Fixed、dependent、irrelevant、removed constraint だけで使われる変数、
named function だけで使われる変数は Adapter profile を制限しません。
Requirements は呼び出しのたびに導出されるため、working copy を変更する
明示的な preparation の結果が反映されます。

```{code-cell} ipython3
from ommx import DecisionVariable, Instance, OneHotConstraint

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

## reduce_capabilities による明示的な lowering

{meth}`Instance.reduce_capabilities() <ommx.Instance.reduce_capabilities>` は、明示的に呼び出す mutating operation です。このメソッドは `preserved` として渡された集合に含まれない特殊制約 family を、対応する変換 API（後述）を使って通常制約に変換します。

```{code-cell} ipython3
converted = instance.reduce_capabilities(preserved=set())
assert converted == {AdditionalCapability.OneHot}
```

この instance は profile が native OneHot support を宣言していないため失敗します。
検査は source instance を変更しません。

## 特殊制約を明示的に lower して再検査する

{attr}`Instance.active_special_constraint_kinds <ommx.Instance.active_special_constraint_kinds>`
は active な特殊制約 family の一覧であり、Adapter support の宣言ではありません。
特定の family を lower するには working copy を用意し、対応する
{class}`~ommx.SpecialConstraintKind` を
{meth}`Instance.lower_special_constraints <ommx.Instance.lower_special_constraints>` に渡します。

```{code-cell} ipython3
import copy

prepared = copy.deepcopy(instance)
lowered = prepared.lower_special_constraints({SpecialConstraintKind.OneHot})
assert lowered == {SpecialConstraintKind.OneHot}
assert prepared.active_special_constraint_kinds == set()
assert len(prepared.constraints) == 1

# preparation で model shape が変わったため、requirements を再導出して再検査する
MyLinearAdapter.require_compatible(prepared)

# source model は変換用 workspace として使っていない
assert instance.active_special_constraint_kinds == {SpecialConstraintKind.OneHot}
```

One-hot 制約が除去され、その代わりに通常の等式制約 $x_0 + x_1 + x_2 - 1 = 0$ が1つ追加されたことが分かります。`reduce_capabilities` はインスタンスを in-place に変更します。成功時、`required_capabilities` は `preserved` の部分集合になります。変換が必要なかった場合は空集合を返します。得られた値に対して `INPUT_CLASS` の membership または Adapter applicability を再評価してください。

より一般的な preparation には exact reformulation、近似、relaxation、有限
penalty 変換が含まれる場合があります。このような workflow では semantics を
明示的に記録し、変換後の solver model を native profile に対して必ず再検査します。
Backend の integer width のような adapter 固有の条件は portable profile に加えて
検査しますが、新しい OMMX capability field ではなく、`ommx.v2.Feature` とも
無関係です。

## Family 別の変換 API

個別の変換 API を直接呼び出すこともできます。

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
| active な特殊制約 family を調べる | {attr}`Instance.required_capabilities <ommx.Instance.required_capabilities>` |
| 維持しない特殊制約を明示的に lowering する | {meth}`Instance.reduce_capabilities <ommx.Instance.reduce_capabilities>` |
| 個別に通常制約に変換する | `convert_*_to_constraint(s)` / `convert_all_*_to_constraints` |
| 変換履歴を確認する | `instance.constraints_df(kind=..., removed=True)` / `solution.constraints_df(kind=..., include=("...","removed_reason"))` |
| Serialized format の Forward Compatibility を検査する | `ommx.v2.Feature` / `required_features` |
