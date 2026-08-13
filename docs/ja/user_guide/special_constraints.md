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

OMMX は通常の制約（{class}`~ommx.Constraint`、等式・不等式を持つ {class}`~ommx.Function`）に加えて、数理最適化で頻出するいくつかの特殊な制約を第一級の制約型として扱います。本ページでは以下の3種類の特殊制約型の定義と使い方、および PySCIPOpt Adapter を使って実際に解く手順を説明します。

- {class}`~ommx.IndicatorConstraint`: バイナリ変数による条件付き制約
- {class}`~ommx.OneHotConstraint`: バイナリ変数集合のうち丁度1つが1
- {class}`~ommx.Sos1Constraint`: 変数集合のうち高々1つが非ゼロ

以下の例では [PySCIPOpt Adapterで0-1ナップサック問題を解く](../tutorial/solve_with_ommx_adapter.md) と同様に PySCIPOpt Adapter を使うので、事前にインストールしてください。

```
pip install ommx-pyscipopt-adapter
```

PySCIPOpt Adapter は線形 Indicator と SOS1 を SCIP の `addConsIndicator` / `addConsSOS1` にそのまま渡します（等式 Indicator は上下2本の不等式 Indicator に分解されます）。OneHot は直接受け入れないため、標準 workflow では PySCIPOpt Adapter の推奨 Policy を取得し、呼び出し側が `Instance.prepare()` で通常の等式制約へ lowering します。詳しくは [Adapter の exact input と Instance preparation](./capability_model.md) を参照してください。

以下では各特殊制約の first-class な表現に加えて、個別の lowering API も説明します。Adapter 向けの標準 workflow では推奨 Policy と `Instance.prepare()` を使い、個々の変換を明示的に選択したい場合や変換結果を詳しく調べたい場合に、これらの API を直接使います。

## IndicatorConstraint

**Indicator Constraint** はバイナリ変数 $z$ に対し、$z = 1$ のときのみ制約 $f(x) \leq 0$ あるいは $f(x) = 0$ を課す条件付き制約です。$z = 0$ のときはこの制約は無条件に満たされると見なされます。

{class}`~ommx.IndicatorConstraint` は、既存の {class}`~ommx.Constraint` に対して {meth}`Constraint.with_indicator() <ommx.Constraint.with_indicator>` を呼ぶことで生成できます。Indicator の引数には変数 ID、detached な {class}`~ommx.DecisionVariable`、または {class}`~ommx.AttachedDecisionVariable` を渡せます。

```{code-cell} ipython3
from ommx import Instance, DecisionVariable, Equality

z = DecisionVariable.binary(0, name="z")
x = DecisionVariable.continuous(1, lower=0, upper=10, name="x")

# z = 1 => x <= 5
ic = (x <= 5).with_indicator(z)
assert ic.indicator_variable_id == 0
assert ic.equality == Equality.LessThanOrEqualToZero
```

{meth}`Instance.from_components <ommx.Instance.from_components>` の `indicator_constraints=` 引数に `dict[int, IndicatorConstraint]` を渡すことでインスタンスに追加できます。

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

PySCIPOpt Adapter はこの線形 Indicator 制約を受け取り、そのまま SCIP に渡します。

```{code-cell} ipython3
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

solution = OMMXPySCIPOptAdapter.solve(instance)
# z = 1 で x <= 5 が効くので x の最大値 5 が目的関数値
assert abs(solution.objective - 5.0) < 1e-6
```

### Indicator を Big-M 制約へ lowering する

{meth}`Instance.convert_indicator_to_constraint(indicator_id) <ommx.Instance.convert_indicator_to_constraint>` は、Indicator 制約 $y = 1 \Rightarrow f(x) \leq 0$ を、$f(x)$ の上下限から計算した Big-M を使って通常制約へ書き換えます。SOS1 の lowering と異なり、新しい indicator 変数は導入せず、`IndicatorConstraint` が保持する変数を $y$ として使います。

$$
f(x) + u y - u \leq 0, \qquad -f(x) - l y + l \leq 0
$$

ここで $u \geq \sup f(x)$、$l \leq \inf f(x)$ です。

- 不等式（$\leq$）の Indicator では上側だけを考慮し、$u > 0$ の場合だけ追加します。$u \leq 0$ なら変数境界だけで元の制約が成り立つため、通常制約は追加しません。
- 等式（$= 0$）の Indicator では上下を独立に判定し、$u > 0$ なら上側、$l < 0$ なら下側を追加します。

```{code-cell} ipython3
indicator_constraint_ids = instance.convert_indicator_to_constraint(0)
assert len(indicator_constraint_ids) == 1
assert instance.indicator_constraints == {}
assert set(instance.removed_indicator_constraints.keys()) == {0}
```

すべての active な Indicator 制約を一括変換するには {meth}`~ommx.Instance.convert_all_indicators_to_constraints` を使います。必要な $f(x)$ の境界が非有限、または $f(x)$ が semi-continuous / semi-integer 変数を参照する場合、変換は mutation の前に失敗します。一括 API はすべての対象を先に検証するため、どれか1つでも変換できなければ `Instance` 全体を変更しません。

## OneHotConstraint

**One-hot 制約** はバイナリ変数の集合 $\{x_1, \ldots, x_n\}$ に対して $\sum_i x_i = 1$、つまり丁度1つだけが $1$ であることを要求します。

```{code-cell} ipython3
from ommx import OneHotConstraint

xs = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
oh = OneHotConstraint(variables=xs)
assert oh.variables == [0, 1, 2]
```

`variables` の各要素には、変数 ID、detached な {class}`~ommx.DecisionVariable`、または {class}`~ommx.AttachedDecisionVariable` を渡せます。この入力は変数の identity だけを使うため、`VariableIDLike` type alias として公開されます。制約を host に追加するとき、enclosing instance は自身が保持する決定変数を source of truth として、参照 ID と制約固有の要件を検証します。OneHot で参照する変数は存在し、かつバイナリである必要があります。制約は各変数の ID を保持し、`oh.variables` から参照できます。数学的には通常の等式制約 $x_0 + x_1 + x_2 - 1 = 0$ と等価ですが、first-class の制約として保持することで、対応するソルバー（MIP系ソルバーの多くは one-hot 制約を直接受け付けます）に効率的に渡すことができます。

```{code-cell} ipython3
values = [5.0, 10.0, 3.0]
instance_oh = Instance.from_components(
    decision_variables=xs,
    objective=sum(v * x for v, x in zip(values, xs)),
    constraints={},
    one_hot_constraints={0: oh},
    sense=Instance.MAXIMIZE,
)
assert set(instance_oh.one_hot_constraints.keys()) == {0}
```

### OneHot を通常の等式制約へ lowering する

{meth}`Instance.convert_one_hot_to_constraint(one_hot_id) <ommx.Instance.convert_one_hot_to_constraint>` は、指定した OneHot を数学的に等価な通常の等式制約 $x_0 + x_1 + x_2 - 1 = 0$ へ書き換え、新しく生成した通常制約の ID を返します。ここでは個別 API の挙動を示すため、PySCIPOpt Adapter に渡す前にこの API を直接呼びます。変換後は別の exact input になるため、solve の前に applicability を確認します。

```{code-cell} ipython3
one_hot_constraint_id = instance_oh.convert_one_hot_to_constraint(0)
assert isinstance(one_hot_constraint_id, int)
assert OMMXPySCIPOptAdapter.check_applicability(instance_oh).is_applicable
solution = OMMXPySCIPOptAdapter.solve(instance_oh)
# 3 つのうち丁度 1 つを選ぶので、最大値 10 をもつ x_1 が選ばれる
assert abs(solution.objective - 10.0) < 1e-6
```

変換は `instance_oh` を in-place に変更します。明示的な lowering の後は OneHot 制約が除去され、変換履歴が `removed_one_hot_constraints` に残ります。`solve` はこの変換を行いません。すべての active な OneHot 制約を一括変換するには {meth}`~ommx.Instance.convert_all_one_hots_to_constraints` を使います。

```{code-cell} ipython3
assert instance_oh.one_hot_constraints == {}
assert len(instance_oh.constraints) == 1
assert set(instance_oh.removed_one_hot_constraints.keys()) == {0}
```

## Sos1Constraint

**SOS1 (Special Ordered Set type 1)** 制約は変数集合 $\{x_1, \ldots, x_n\}$ の**高々1個**しか非ゼロになれないことを要求します。One-hot との違いは以下の通りです。

- One-hot は $\sum x_i = 1$ を要求するため、丁度1個が非ゼロ。
- SOS1 は高々1個が非ゼロで、全て $0$ であることも許容。
- SOS1 の変数はバイナリとは限らない（連続変数でも良い）。

```{code-cell} ipython3
from ommx import Sos1Constraint

ys = [DecisionVariable.continuous(i, lower=0, upper=10, name="y", subscripts=[i]) for i in range(3, 6)]
s1 = Sos1Constraint(variables=ys)
assert s1.variables == [3, 4, 5]
```

`OneHotConstraint` と同様に、`variables` の各要素は `VariableIDLike`、すなわち変数 ID、detached な決定変数、または attached な決定変数を受け取ります。SOS1 制約は各変数の ID を保持し、`s1.variables` から参照できます。

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

PySCIPOpt Adapter は SOS1 制約を受け取り、そのまま SCIP に渡します。

```{code-cell} ipython3
solution = OMMXPySCIPOptAdapter.solve(instance_s1)
# 高々 1 つだけが非ゼロなので、1 つを上限 10 にして他を 0 にする
assert abs(solution.objective - 10.0) < 1e-6
```

### SOS1 を Big-M 制約へ lowering する

{meth}`Instance.convert_sos1_to_constraints(sos1_id) <ommx.Instance.convert_sos1_to_constraints>` は、SOS1 を Big-M 法で通常制約へ変換します。各変数 $x_i \in [l_i, u_i]$ に対して次の規則を適用します。

1. $x_i$ が境界 $[0, 1]$ の Binary 変数なら、その変数自体を indicator として再利用します。
2. それ以外では新しい Binary 変数 $y_i$ を導入し、$x_i - u_i y_i \leq 0$ と $l_i y_i - x_i \leq 0$ を追加します。$u_i = 0$ または $l_i = 0$ の自明な側は省略します。
3. 最後に cardinality 制約 $\sum_i y_i - 1 \leq 0$ を追加します。

```{code-cell} ipython3
sos1_constraint_ids = instance_s1.convert_sos1_to_constraints(0)
# 3本の上側 Big-M 制約と1本の cardinality 制約
assert len(sos1_constraint_ids) == 4
assert instance_s1.sos1_constraints == {}
assert set(instance_s1.removed_sos1_constraints.keys()) == {0}
```

すべての active な SOS1 制約を一括変換するには {meth}`~ommx.Instance.convert_all_sos1_to_constraints` を使います。非 Binary 変数の境界が非有限、domain が $0$ を含まない、または kind が semi-continuous / semi-integer の場合、変換は mutation の前に失敗します。一括 API はすべての対象を先に検証するため、どれか1つでも変換できなければ `Instance` 全体を変更しません。

## 制約種別ごとに独立したID空間

OMMX では、通常制約・Indicator・OneHot・SOS1 の4つはそれぞれ**独立したID空間**を持ちます。{meth}`Instance.from_components <ommx.Instance.from_components>` に渡す4つの dict はそれぞれ独立したキー空間として扱われるため、異なる制約型同士で同じ整数 ID を使っても衝突しません。

したがって、例えば「通常制約 ID=1」と「Indicator 制約 ID=1」は衝突せず、別々の制約として共存できます。

```{code-cell} ipython3
z2 = DecisionVariable.binary(10, name="z2")
x2 = DecisionVariable.continuous(11, lower=0, upper=10, name="x2")

instance_mix = Instance.from_components(
    decision_variables=[z2, x2] + xs + ys,
    objective=x2,
    constraints={1: z2 == 1},                              # 通常制約 ID=1
    indicator_constraints={1: (x2 <= 5).with_indicator(z2)}, # Indicator ID=1
    one_hot_constraints={1: OneHotConstraint(variables=xs)},        # OneHot ID=1
    sos1_constraints={1: Sos1Constraint(variables=ys)},             # SOS1 ID=1
    sense=Instance.MAXIMIZE,
)

# 4 種の dict それぞれが ID=1 の制約を独立に保持している
assert set(instance_mix.constraints.keys()) == {1}
assert set(instance_mix.indicator_constraints.keys()) == {1}
assert set(instance_mix.one_hot_constraints.keys()) == {1}
assert set(instance_mix.sos1_constraints.keys()) == {1}
```

ただし、特殊制約型を通常制約に lowering すると、新たに生成される通常制約は **`Constraint` 側の ID 空間**から割り当てられます。変換後に衝突する可能性があるのは通常制約の ID のみです。

## Lowering 結果を監査する

変換元の特殊制約は破棄されず、次の `removed_*_constraints` dict に removed entry として保持されます。

| 元の制約型 | Removed dict | DataFrame |
|---|---|---|
| OneHotConstraint | {attr}`~ommx.Instance.removed_one_hot_constraints` | `instance.constraints_df(kind="one_hot", removed=True)` |
| Sos1Constraint | {attr}`~ommx.Instance.removed_sos1_constraints` | `instance.constraints_df(kind="sos1", removed=True)` |
| IndicatorConstraint | {attr}`~ommx.Instance.removed_indicator_constraints` | `instance.constraints_df(kind="indicator", removed=True)` |

`removed=True` を指定すると、active と removed の行を同じ DataFrame で取得できます。`removed_reason` と `removed_reason.{key}` の列も自動的に追加されるため、変換された制約と active な制約を区別できます。

各 removed entry（{class}`~ommx.RemovedOneHotConstraint` / {class}`~ommx.RemovedSos1Constraint` / {class}`~ommx.RemovedIndicatorConstraint`）は、`removed_reason` と、生成した通常制約 ID を格納する `removed_reason_parameters` を保持します。

- **OneHot**: `constraint_id` に単一の ID
- **SOS1**: `constraint_ids` にカンマ区切りの ID リスト
- **Indicator**: `constraint_ids` にカンマ区切りの ID リスト。Big-M の両側が自明なら空文字列

```{code-cell} ipython3
removed = instance_oh.removed_one_hot_constraints
assert set(removed.keys()) == {0}
assert removed[0].removed_reason == "ommx.Instance.convert_one_hot_to_constraint"
```

生成された通常制約は、{attr}`Constraint.provenance <ommx.Constraint.provenance>` にも変換元への参照を保持します。各 {class}`~ommx.Provenance` は変換元の {attr}`~ommx.Provenance.kind` と {attr}`~ommx.Provenance.original_id` を記録するため、通常制約から元の特殊制約型と ID を辿れます。

```{code-cell} ipython3
from ommx import ProvenanceKind

generated = instance_oh.constraints[one_hot_constraint_id]
assert any(
    p.kind == ProvenanceKind.OneHotConstraint and p.original_id == 0
    for p in generated.provenance
)
```

## 評価結果の参照

インスタンスを解いて得られた {class}`~ommx.Solution` や {class}`~ommx.SampleSet` は、共通の {meth}`~ommx.Solution.constraints_df` を `kind=` で切り替えて使います。

| 制約型 | `kind=` の値 |
|---|---|
| 通常制約 | `"regular"`（デフォルト） |
| Indicator | `"indicator"` |
| OneHot | `"one_hot"` |
| SOS1 | `"sos1"` |

```python
solution.constraints_df()                  # regular（デフォルト）
solution.constraints_df(kind="indicator")  # Indicator
sample_set.constraints_df(kind="one_hot")  # OneHot
```

DataFrame の index 名は kind ごとに qualified（`regular_constraint_id` / `indicator_constraint_id` / `one_hot_constraint_id` / `sos1_constraint_id`）になっており、別 ID 空間どうしを誤って `df.join()` した場合に `df.head()` 等で気づきやすくなっています。

Indicator 制約の DataFrame には、`indicator_active` というカラムが含まれます。これにより「インジケータが OFF だった（制約は自明に満たされた）」ケースと「インジケータが ON で制約が本当に満たされた」ケースを区別できます。なお、Indicator 制約には双対変数の値は定義されない（条件付き制約に対する双対値は一般に well-defined ではない）ため、`dual_variable` は含まれません。

### `include=` で removed_reason カラムを追加する

`removed_reason` は {meth}`~ommx.Solution.constraints_df` のデフォルトカラムには含まれません。`include=` に `"removed_reason"` を渡すと、reason 名と `removed_reason.{key}` パラメータカラムがまとめて追加されます（評価前に削除された制約の行のみ値が入り、それ以外の行は NA）。

```python
df = solution.constraints_df(
    include=("label", "parameters", "removed_reason"),
)
```

Indicator / OneHot / SOS1 でも同じく、対応する `kind=` と一緒に `"removed_reason"` を `include=` に渡せば取得できます。long-format で（id, parameter_key）の組合せごとに 1 行を得たい場合は、`kind=` で切り替えられる {meth}`~ommx.Solution.constraint_removed_reasons_df` サイドカーを引き続き利用できます。

## Relax / Restore

{class}`~ommx.IndicatorConstraint` は、通常制約と同じ relax / restore ワークフローを持ちます。

- {meth}`Instance.relax_indicator_constraint() <ommx.Instance.relax_indicator_constraint>`: Indicator 制約を「緩和」（無効化）し、理由文字列を記録します。緩和された制約は `removed_indicator_constraints` に移動します。
- {meth}`Instance.restore_indicator_constraint() <ommx.Instance.restore_indicator_constraint>`: 緩和した Indicator 制約を元に戻します。インジケータ変数が既に値を代入されている・固定されている場合は失敗します。

OneHot / SOS1 については、`removed_one_hot_constraints` / `removed_sos1_constraints` への移動は本ページで扱った個別の lowering API によって行われます。
