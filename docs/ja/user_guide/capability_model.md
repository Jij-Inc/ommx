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

# Adapter Capability モデルと明示的な preparation

Adapter Capability が答えるのは、**この Adapter がどの完全な model
shape を backend solver へ直接変換できるか**、という問いです。Solver
入力で実際に使われる変数の kind、目的関数と制約 family ごとの
多項式次数上限、制約の関係、最適化 sense を含みます。したがって、
通常制約が常にサポートされるとは限りません。

以下の3種類の API は似ていますが、責務が明確に分かれています。

| API | 責務 |
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

requirements = instance.solver_requirements()
assert requirements.used_variable_ids == {0, 1, 2}
assert requirements.objective_degree == 1
assert requirements.one_hot_constraint_ids == {0}
```

## 一貫した native profile を宣言する

Adapter は `CAPABILITIES` に1つ以上の完全な
{class}`~ommx.CapabilityProfile` を宣言します。1つの profile 内ではすべての field
が論理積です。複数の profile は各 field を和集合にせず、択一的な model
shape を表します。たとえば continuous QP と MILP の profile を分けることで、
意図せず MIQP support を宣言するのを防げます。

```{code-cell} ipython3
from ommx import (
    AdapterCapabilities,
    CapabilityProfile,
    DegreeLimit,
    Equality,
    Kind,
    Sense,
)
from ommx.adapter import SolverAdapter

linear_profile = CapabilityProfile(
    name="binary-linear",
    variable_kinds={Kind.Binary},
    objective_degree=DegreeLimit.at_most(1),
    regular_constraints={
        Equality.EqualToZero: DegreeLimit.at_most(1),
        Equality.LessThanOrEqualToZero: DegreeLimit.at_most(1),
    },
    senses={Sense.Minimize, Sense.Maximize},
)

class MyLinearAdapter(SolverAdapter):
    CAPABILITIES = AdapterCapabilities([linear_profile])
```

制約 family を省略すると、その profile はその family の active な制約を
1つもサポートしません。`DegreeLimit.at_most(n)` は累積的で、0から
`n` までの次数を受理します。`DegreeLimit.any()` は OMMX で表現できるすべての
多項式次数を受理します。Portable に表現できない adapter 固有の数値制限は、
profile ではなく adapter の `_check_preconditions` hook で検査します。

## 入力を変更しない compatibility check

{meth}`~ommx.adapter.SolverAdapter.check_compatibility` は model 全体の requirements を
各 native profile と比較し、その後 adapter 固有の前提条件を検査します。
入力を lower、encode、relax したり、その他の変更を加えたりしません。
{meth}`~ommx.adapter.SolverAdapter.require_compatible` は成功時に同じ report を返し、
失敗時に {class}`~ommx.adapter.AdapterCompatibilityError` を送出します。

```{code-cell} ipython3
from ommx import SpecialConstraintKind

report = MyLinearAdapter.check_compatibility(instance)
assert not report.compatible
assert instance.active_special_constraint_kinds == {SpecialConstraintKind.OneHot}
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

`lower_special_constraints` は選択した instance を in-place に変更し、
実際に lower した family を返します。対象を直接指定するため、集合を「ある
Adapter がサポートするもの」と誤解する余地がありません。この操作は後述する
family 別の変換 API を組み合わせ、監査のために removed constraint と provenance を
保持します。

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
| Active な solver-facing model shape を導出する | {meth}`Instance.solver_requirements <ommx.Instance.solver_requirements>` |
| Translator が直接扱える範囲を宣言する | {class}`~ommx.AdapterCapabilities` を持つ `SolverAdapter.CAPABILITIES` |
| 入力を変更せず互換性を検査する | `check_compatibility` / `require_compatible` |
| Active な特殊制約 family を調べる | {attr}`Instance.active_special_constraint_kinds <ommx.Instance.active_special_constraint_kinds>` |
| 選択した特殊制約を明示的に lower する | {meth}`Instance.lower_special_constraints <ommx.Instance.lower_special_constraints>` |
| 個別に通常制約に変換する | `convert_*_to_constraint(s)` / `convert_all_*_to_constraints` |
| 変換履歴を確認する | `instance.constraints_df(kind=..., removed=True)` / `solution.constraints_df(kind=..., include=("...","removed_reason"))` |
| Serialized format の Forward Compatibility を検査する | `ommx.v2.Feature` / `required_features` |
