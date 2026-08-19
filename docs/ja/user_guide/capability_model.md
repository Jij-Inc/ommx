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

- {class}`~ommx.InstanceClass` は、具体的な `Instance` 値の集合です。Adapter は、その集合を `INPUT_CLASS` として宣言することで applicability を定義します。
- {meth}`Instance.lower_special_constraints() <ommx.Instance.lower_special_constraints>` は、Instance 上で選択した特殊制約 family を明示的に lowering します。入力 class の宣言でも、Adapter applicability の証明でもありません。

本ページでは以下を説明します。

- `InstanceClass` の membership と Adapter applicability
- 特殊制約 family selector としての {class}`~ommx.SpecialConstraintKind` と {attr}`Instance.active_special_constraint_kinds <ommx.Instance.active_special_constraint_kinds>`
- {meth}`Instance.lower_special_constraints() <ommx.Instance.lower_special_constraints>` による明示的な lowering
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

Adapterはapplicabilityの完全な条件を`INPUT_CLASS`として宣言します。構造化された
membership resultを得るには`check_applicability()`、membershipが満たされない場合に
例外を送出するには`require_applicable()`を使います。easyな`solve()`と`sample()`は
呼び出し元のInstanceをcopyし、そのprivate copyを`INPUT_CLASS`向けにPrepareするため、
呼び出し元の値を変更しません。preparation-freeな`solve_strict()`と`sample_strict()`は
exactなmemberを要求します。customなPreparation Policyを使うapplicationは、入力を
明示的にcopyしてPrepareした後にstrict APIを呼びます。

Applicability と solver input の構築は別の境界です。Membership は、その後の全ての converter または backend operation の成功を保証しません。Converter は local helper が受け取る表現を検証でき、backend は solver input の構築中に数値や実装上の上限を拒否できます。これらは conversion または backend の error であり、追加の applicability 条件ではありません。また、`AdapterNotApplicableError` として報告してはいけません。

### Active objectiveとoutput objective

Preparationでは、exactなAdapter inputを作るためにInstanceのobjectiveやsenseを書き換える
ことがあります。その場合、{attr}`Instance.objective <ommx.Instance.objective>`と
{attr}`Instance.sense <ommx.Instance.sense>`はbackendが受け取るactive formulationを表し、
read-onlyな{attr}`Instance.output_objective <ommx.Instance.output_objective>`はユーザーへ
提示するobjective functionとsenseを保持します。

{meth}`Instance.evaluate <ommx.Instance.evaluate>`と
{meth}`Instance.evaluate_samples <ommx.Instance.evaluate_samples>`は、output pairがあれば
それを使います。例えば最大化modelをOpenJij向けに最小化へ正規化してsampleしても、返る
`SampleSet`はsource側の最大化senseとobjective valueを持ちます。active pairをそのまま
出力する場合、`output_objective`は`None`です。

active objectiveまたはsenseを最初に変更した変換がoutput pairを確立します。それ以降の
変換はactive formulationだけを書き換え、既存のoutput pairを置換したり符号反転したり
しません。新しい`Instance.objective`の代入はmodelの明示的な再定義なので、保持済みの
pairをclearし、現在のactive senseと新しいobjectiveを再び出力意味論にします。

backendのoptimality proofとdual valueはactive formulationに属します。output pairがある
場合、genericなAdapterはそれらの主張がproject後のobjectiveにも有効か推論できません
（fixed penaltyが基本的な反例です）。そのためbuilt-in solver Adapterはこの場合の
optimalityをunspecifiedのままにし、dual valueも出力しません。変換固有の証明を持つ
Adapterは明示的にtransportできます。

これは出力意味論であり、Preparationのevent logではありません。easyなAdapter APIは
一時的なPrepare後Instanceを破棄し、`Solution`または`SampleSet`だけを返します。
Experimentは元の入力とその出力を記録します。`Instance`と`Solution` / `SampleSet`が
引き続きserialization rootであり、別のsolve outcomeやPreparation history objectは
導入しません。

## SpecialConstraintKind と active_special_constraint_kinds

{class}`~ommx.SpecialConstraintKind` は、通常制約への明示的な lowering 対象として選択できる active な特殊制約 family を列挙します。Adapter の入力宣言でも serialization feature でもありません。

| Kind | 対応する制約型 |
|---|---|
| `SpecialConstraintKind.Indicator` | {class}`~ommx.IndicatorConstraint` |
| `SpecialConstraintKind.OneHot` | {class}`~ommx.OneHotConstraint` |
| `SpecialConstraintKind.Sos1` | {class}`~ommx.Sos1Constraint` |

{attr}`Instance.active_special_constraint_kinds <ommx.Instance.active_special_constraint_kinds>` は、その {class}`~ommx.Instance` が **現在保持している active な特殊制約** に対応する `SpecialConstraintKind` の集合を返します。通常制約しか使っていない場合は空集合です。

```{code-cell} ipython3
from ommx import Instance, DecisionVariable, OneHotConstraint, SpecialConstraintKind

xs = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]

instance = Instance.from_components(
    decision_variables=xs,
    objective=sum(xs),
    constraints={},
    one_hot_constraints={0: OneHotConstraint(variables=xs)},
    sense=Instance.MAXIMIZE,
)
assert instance.active_special_constraint_kinds == {SpecialConstraintKind.OneHot}
assert binary_linear_with_one_hot.contains(instance)
```

## lower_special_constraints による明示的な lowering

{meth}`Instance.lower_special_constraints() <ommx.Instance.lower_special_constraints>` は、明示的に呼び出す mutating operation です。このメソッドは `kinds_to_lower` として選択された特殊制約 family が active な場合に、対応する変換 API（後述）を使って通常制約に変換します。集合に含めなかった family は active なまま残り、空集合は no-op です。

```{code-cell} ipython3
lowered = instance.lower_special_constraints({SpecialConstraintKind.OneHot})
assert lowered == {SpecialConstraintKind.OneHot}
```

```{code-cell} ipython3
assert instance.active_special_constraint_kinds == set()
assert instance.one_hot_constraints == {}
assert len(instance.constraints) == 1
```

One-hot 制約が除去され、その代わりに通常の等式制約 $x_0 + x_1 + x_2 - 1 = 0$ が1つ追加されたことが分かります。`lower_special_constraints` はインスタンスを in-place に変更し、選択され、active で、実際に lowering された family だけを返します。選択した family が active でなければ空集合を返します。得られた値に対して Adapter applicability、すなわち `INPUT_CLASS` membership を再評価してください。

## 手動変換 API

`lower_special_constraints` は内部的に、制約型別の以下の変換 API を組み合わせて実装されています。ユーザーがこれらを直接呼ぶことも可能です。

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
| Adapter applicability を定義する | `INPUT_CLASS` |
| `INPUT_CLASS` membership を report または強制する | `check_applicability()` / `require_applicable()` |
| 保持された出力objectiveの意味を確認する | {attr}`~ommx.Instance.output_objective` |
| active な特殊制約 family を調べる | {attr}`Instance.active_special_constraint_kinds <ommx.Instance.active_special_constraint_kinds>` |
| 選択した特殊制約を明示的に lowering する | {meth}`Instance.lower_special_constraints <ommx.Instance.lower_special_constraints>` |
| 個別に通常制約に変換する | `convert_*_to_constraint(s)` / `convert_all_*_to_constraints` |
| 変換履歴を確認する | `instance.constraints_df(kind=..., removed=True)` / `solution.constraints_df(kind=..., include=("...","removed_reason"))` |
