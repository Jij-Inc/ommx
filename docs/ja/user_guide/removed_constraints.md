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

(removed-constraints)=
# 変換後の問題とRemoved constraints

Instanceを変換すると、ソルバーが解く問題の形が変わります。一方でユーザーは、元の変数の値や目的値、元の制約を満たすかを知りたいことがあります。OMMXは、**現在解く問題と、結果を解釈するために保持する情報を分けて**扱います。

このページでは{ref}`Instanceの変数の役割 <instance-variable-roles>`と{class}`~ommx.Solution`の基本的な読み方を前提に、変換後の実行可能性と目的値の読み方を説明します。変換ごとの数式や適用条件はAPI Referenceに委ねます。

## Activeから外した制約も評価できる

例えば、$x \in \{0,1\}$ を最大化する問題から、一時的に $x \leq 0$ という制約を外します。

```{code-cell} ipython3
from ommx import Instance

instance = Instance.maximize()
x = instance.new_binary("x")
instance.objective = x
limit = instance.add_constraint(x <= 0, "limit")
limit_id = limit.constraint_id

instance.relax_constraint(limit_id, "example.relax_limit")

assert limit_id not in instance.constraints
assert limit_id in instance.removed_constraints

solution = instance.evaluate({x.id: 1})
assert solution.feasible_relaxed
assert not solution.feasible
```

`constraints` にある制約をactive、`removed_constraints` に保持される制約をremovedと呼びます。removedは「もう検査しない制約」ではありません。現在ソルバーに課す条件からは外れますが、OMMXで結果を評価するときには、元の制約を満たすかも確認できます。

| 判定 | 検査する条件 |
|---|---|
| {attr}`~ommx.Solution.feasible_relaxed` | activeな制約と、変数のkind・bound |
| {attr}`~ommx.Solution.feasible` | activeとremovedの両方の制約と、変数のkind・bound |

{attr}`~ommx.Solution.feasible_unrelaxed` は `feasible` と同じ判定です。この例では $x=1$ が現在の問題では許されても、removedに保持した $x\leq0$ は満たしません。

変数の値は、{ref}`Instanceの規則 <instance-variable-roles>`で復元・補完されたものを使います。removedにしか現れずirrelevantになった変数の値を省略すると、既定値での検査になります。特定の候補が元の制約を満たすか確認したい場合は、その候補の値を明示的に渡してください。

## Removedは「緩和した」とは限らない

制約をactiveからremovedへ移す操作は、いくつかあります。

| 操作の例 | 現在の問題での扱い | Removedを保持する理由 |
|---|---|---|
| 手動のrelax | 制約を課さない | 外した条件を候補が満たすか確認する |
| 特殊制約のlowering | 通常制約で同じ条件を表す | 元の特殊制約として結果を確認する |
| Penalty法 | 制約を目的関数の項に置き換える | penaltyを含む候補が元の制約を満たすか確認する |
| 自明な制約の除去 | 不要になった条件をactiveから外す | どの条件を除いたか確認する |

したがって、removedがあることだけから、変換が等価か、緩和か、最適解を保存するかは分かりません。例えば有限なpenalty weightでは、元の制約を破る候補が選ばれる可能性があります。値を復元できることも、元の問題で実行可能であることや、最適であることを意味しません。各変換の保証は、変数のdomainや適用条件も含めて確認する必要があります。

通常制約だけでなく、Indicator・OneHot・SOS1にもactiveとremovedがあります。

| 制約の種類 | Active | Removed |
|---|---|---|
| 通常制約 | {attr}`~ommx.Instance.constraints` | {attr}`~ommx.Instance.removed_constraints` |
| Indicator | {attr}`~ommx.Instance.constraints` | {attr}`~ommx.Instance.removed_constraints` |
| OneHot | {attr}`~ommx.Instance.constraints` | {attr}`~ommx.Instance.removed_constraints` |
| SOS1 | {attr}`~ommx.Instance.constraints` | {attr}`~ommx.Instance.removed_constraints` |

例えば {meth}`~ommx.Instance.convert_one_hot_to_constraint` はOneHotを通常の等式制約に置き換え、元のOneHotをremovedとして保持します。生成された制約には、変換元を示すprovenanceが付きます。生成物や削除理由の正確な形式は各変換メソッドのAPI Referenceを参照してください。

(active-and-output-objective)=
## ソルバー向けの目的値と、出力する目的値

目的関数も、ソルバー向けの表現と、ユーザーに返す表現が異なることがあります。例えば、最大化を受け付けないソルバーのために目的関数の符号を反転しても、結果は元の最大化問題の値で読みたいはずです。

```{code-cell} ipython3
from ommx import Sense

objective_example = Instance.maximize()
reward = objective_example.new_binary("reward")
objective_example.objective = 3 * reward
objective_example.convert_active_objective(Sense.Minimize)

# 現在解くのは -3 * reward の最小化。
assert objective_example.sense == Sense.Minimize
assert objective_example.objective.evaluate({reward.id: 1}) == -3

# 評価結果は元の目的関数と最大化・最小化で返る。
reported = objective_example.evaluate({reward.id: 1})
assert reported.sense == Sense.Maximize
assert reported.objective == 3
```

{attr}`~ommx.Instance.objective` と {attr}`~ommx.Instance.sense` はactiveな問題を表します。一方、変換が評価用の目的関数を保存した場合、{attr}`~ommx.Instance.output_objective` にその情報が入り、`evaluate` はそちらを使います。新しく構築した通常のInstanceでは出力目的関数はなく、activeな目的関数がそのまま評価されます。

Penalty法でも、ソルバーが最小化するpenalty込みの値と、Solutionに返す目的値を区別します。`Solution.objective` が小さいだけでは、penaltyが十分小さいことや、元の制約を満たすことは分かりません。実行可能性を別に確認してください。

最適性も別の情報です。等価な目的関数の符号反転と、有限重みのpenalty化では保証が異なります。ソルバーがactiveな問題について報告した最適性の扱いは {meth}`~ommx.Instance.map_active_optimality` の契約に従います。出力目的関数を保持すること自体は、最適性の証明ではありません。

## 何が外されたか調べる

Instanceでは、`constraints_df(kind="regular", removed=True)` でactiveとremovedの両方を表示できます。`removed_reason` は、制約を外した操作や、呼び出し側が付けた理由を表します。

```{code-cell} ipython3
instance.constraints_df(kind="regular", removed=True)
```

Solutionでは両方の制約が評価済みです。`include=("removed_reason",)` を指定すると、removedの行に理由を追加して確認できます。

```{code-cell} ipython3
solution.constraints_df(kind="regular", include=("removed_reason",))
```

特殊制約は `kind="indicator"`、`kind="one_hot"`、`kind="sos1"` で選びます。複数の候補を評価する {class}`~ommx.SampleSet` でも同じ区別があり、`feasible` と `feasible_relaxed` はサンプルIDごとの判定を返します。

通常制約とIndicatorの手動のrelax・restoreは、{meth}`~ommx.Instance.relax_constraint` / {meth}`~ommx.Instance.restore_constraint` と {meth}`~ommx.Instance.relax_indicator_constraint` / {meth}`~ommx.Instance.restore_indicator_constraint` を参照してください。restoreは制約をactiveに戻す操作であり、変換全体の逆操作ではありません。生成された補助変数や別の制約、目的関数の変更まで取り消すものではありません。
