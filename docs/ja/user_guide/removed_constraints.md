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

# Removed constraints と実行可能性

OMMX では、制約をモデルから単に消去する代わりに、active から removed へ lifecycle を移すことができます。removed constraint は Backend に渡す現在の数学的問題からは除外されますが、変換前の制約と削除理由を保持し、{class}`~ommx.Solution` や {class}`~ommx.SampleSet` の評価に使われます。

この仕組みにより、Preparation 後の Instance は Adapter の exact input だけを active に持ちながら、得られた解が変換前の制約も満たすかを検査できます。Preparation 全体の責任境界は [Instance Preparation と PreparationPolicy](./preparation_policy.md) を参照してください。

## Active と removed

通常制約、Indicator、OneHot、SOS1 の各制約族は、それぞれ active と removed の collection を持ちます。同じ制約族の中で active ID と removed ID は重複せず、制約の label や provenance などの context は lifecycle を移っても保持されます。

| 制約族 | Active | Removed |
|---|---|---|
| 通常制約 | {attr}`~ommx.Instance.constraints` | {attr}`~ommx.Instance.removed_constraints` |
| Indicator | {attr}`~ommx.Instance.indicator_constraints` | {attr}`~ommx.Instance.removed_indicator_constraints` |
| OneHot | {attr}`~ommx.Instance.one_hot_constraints` | {attr}`~ommx.Instance.removed_one_hot_constraints` |
| SOS1 | {attr}`~ommx.Instance.sos1_constraints` | {attr}`~ommx.Instance.removed_sos1_constraints` |

removed entry は元の制約と `removed_reason`、`removed_reason_parameters` を持ちます。reason は制約を removed にした操作や application を識別する名前、parameters はその操作に固有の文字列情報です。

次の例では、通常制約を手動で relax し、OneHot 制約を数学的に等価な通常制約へ lowering します。

```{code-cell} ipython3
from ommx import DecisionVariable, Instance, OneHotConstraint, ProvenanceKind

x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(2)]
instance = Instance.from_components(
    decision_variables=x,
    objective=x[0] + 2 * x[1],
    constraints={10: x[0] <= 0},
    one_hot_constraints={20: OneHotConstraint(variables=x)},
    sense=Instance.MAXIMIZE,
)

instance.relax_constraint(
    10,
    "example.capacity_relaxation",
    ticket="OPS-42",
)
generated_id = instance.convert_one_hot_to_constraint(20)

assert 10 not in instance.constraints
assert instance.removed_constraints[10].removed_reason == "example.capacity_relaxation"
assert instance.removed_constraints[10].removed_reason_parameters == {"ticket": "OPS-42"}

assert 20 not in instance.one_hot_constraints
assert instance.removed_one_hot_constraints[20].removed_reason == (
    "ommx.Instance.convert_one_hot_to_constraint"
)
```

lowering で生成された通常制約には、変換元への {attr}`Constraint.provenance <ommx.Constraint.provenance>` も記録されます。removed entry から生成先を、生成された制約から変換元を辿れるため、変換の両方向を監査できます。特殊制約ごとの lowering、生成物、reason parameter の正確な契約は、[特殊制約型](./special_constraints.md)からリンクしている各変換メソッドの API Reference を参照してください。

```{code-cell} ipython3
generated = instance.constraints[generated_id]
assert any(
    p.kind == ProvenanceKind.OneHotConstraint and p.original_id == 20
    for p in generated.provenance
)
```

Penalty method や自明な制約の除去など、通常制約を変換する操作も removed lifecycle を使います。{class}`~ommx.ParametricInstance` も通常制約について active / removed collection を保持します。

## Instance で監査する

{meth}`~ommx.Instance.constraints_df` は `kind=` で制約族を選びます。`removed=True` にすると active と removed の行を同じ DataFrame で取得し、`removed_reason` と `removed_reason.{key}` の列で区別できます。

| `kind=` | 制約族 | Index |
|---|---|---|
| `"regular"` | 通常制約 | `regular_constraint_id` |
| `"indicator"` | Indicator | `indicator_constraint_id` |
| `"one_hot"` | OneHot | `one_hot_constraint_id` |
| `"sos1"` | SOS1 | `sos1_constraint_id` |

```{code-cell} ipython3
instance.constraints_df(removed=True)
```

```{code-cell} ipython3
instance.constraints_df(kind="one_hot", removed=True)
```

reason と parameter を long format で扱う場合は、{meth}`~ommx.Instance.constraint_removed_reasons_df` を使います。

```{code-cell} ipython3
instance.constraint_removed_reasons_df()
```

## Solution の feasible と feasible_relaxed

{meth}`~ommx.Instance.evaluate` は active と removed の両方を評価します。{attr}`~ommx.Solution.feasible` は次のすべてを満たす場合だけ `True` です。

- active と removed を含む4制約族のすべての制約
- 決定変数の kind と bound

一方、{attr}`~ommx.Solution.feasible_relaxed` は removed constraint を除外しますが、active constraint と決定変数の domain は引き続き検査します。

先ほどの Instance で `x[0] = 1, x[1] = 0` を評価すると、OneHot から生成された active な通常の等式制約は満たしますが、removed にした `x[0] <= 0` は破ります。そのため、現在の relaxed problem では実行可能でも、変換前の制約を含めると実行不可能です。

```{code-cell} ipython3
solution = instance.evaluate({0: 1, 1: 0})

assert not solution.feasible
assert solution.feasible_relaxed
```

{meth}`~ommx.Solution.constraints_df` では active / removed の行をともに評価済みの制約として返すため、`removed=` 引数はありません。`include=` に `"removed_reason"` を指定すると、removed だった行に reason と parameters が追加されます。

```{code-cell} ipython3
solution.constraints_df(include=("removed_reason",))
```

Indicator、OneHot、SOS1 についても `kind=` を切り替えて同じ API を使います。reason を long format で取得する {meth}`~ommx.Solution.constraint_removed_reasons_df` も利用できます。

## SampleSet でサンプルごとに評価する

{meth}`~ommx.Instance.evaluate_samples` も各サンプルについてactive / removed制約の区別を保持します。{attr}`~ommx.SampleSet.feasible` と {attr}`~ommx.SampleSet.feasible_relaxed` は、sample IDから4制約族についての判定結果へのdictです。前者はactiveとremoved、後者はactiveだけを検査します。`best_feasible` と `best_feasible_relaxed` も、この制約だけに基づく判定でsampleを選びます。

{class}`~ommx.SampleSet` のこれらの判定は、決定変数のkindとboundを検査しません。この点は、それらも検査する {attr}`~ommx.Solution.feasible` / {attr}`~ommx.Solution.feasible_relaxed` と異なります。選んだsampleを最終的なSolutionとして扱う場合は、Solution側の実行可能性も確認してください。

```{code-cell} ipython3
from ommx import Samples

sample_set = instance.evaluate_samples(
    Samples(
        [
            {0: 1, 1: 0},  # removed な x[0] <= 0 だけを破る
            {0: 0, 1: 1},  # active / removed の両方を満たす
        ]
    )
)

assert sample_set.feasible == {0: False, 1: True}
assert sample_set.feasible_relaxed == {0: True, 1: True}
```

{meth}`~ommx.SampleSet.constraints_df` と {meth}`~ommx.SampleSet.constraint_removed_reasons_df` でも、`Solution` と同様に `kind=` と `include=("removed_reason",)` を使えます。

## Relax / Restore は rollback ではない

通常制約と Indicator 制約は、次の owner operation で手動の lifecycle 変更ができます。

- {meth}`~ommx.Instance.relax_constraint` / {meth}`~ommx.Instance.restore_constraint`
- {meth}`~ommx.Instance.relax_indicator_constraint` / {meth}`~ommx.Instance.restore_indicator_constraint`

restore は、removed に移った1本の制約を active に戻す操作です。現在の Instance に fixed value や decision-variable dependency がある場合、それらを適用して制約を正規化してから戻します。Indicator の indicator variable 自体が fixed または dependent なら restore できません。

restore は lowering や Preparation 全体の逆変換ではありません。生成された通常制約、補助変数、目的関数へ追加された penalty、decision-variable dependency など、別の副作用は取り除きません。OneHot と SOS1 には public な restore operation はなく、lowering により removed へ移った履歴として保持されます。

```{code-cell} ipython3
instance.restore_constraint(10)

assert 10 in instance.constraints
assert generated_id in instance.constraints  # lowering で生成された制約は残る
assert 20 in instance.removed_one_hot_constraints

restored_solution = instance.evaluate({0: 1, 1: 0})
assert not restored_solution.feasible_relaxed
```

Preparation が途中で失敗した場合の部分的な変更も、restore で一括して巻き戻すことはできません。変換前の Instance を保持する必要がある場合は、Preparation の前に明示的にコピーしてください。
