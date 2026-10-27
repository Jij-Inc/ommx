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

# StateからSolutionへの評価

{class}`~ommx.State` は変数IDから値への対応、{class}`~ommx.Solution` はそれを特定のInstanceで評価した結果です。**値の割り当てだけでは、その問題の実行可能解かどうかは分かりません。** ソルバーの出力でも、手動で用意した候補でも、同じ {meth}`~ommx.Instance.evaluate` で検査できます。

例えば、二つの項目を同時には選べない問題に、候補 $x=1, y=0$ を与えます。

```{code-cell} ipython3
from ommx import Instance, Optimality, State

instance = Instance.maximize()
x = instance.new_binary("x")
y = instance.new_binary("y")
instance.objective = x + y
instance.add_constraint(x * y == 0, "exclusive")

state = State({x.id: 1, y.id: 0})
solution = instance.evaluate(state)

assert solution.state.entries == {x.id: 1.0, y.id: 0.0}
assert solution.feasible
assert solution.objective == 1
assert solution.optimality == Optimality.Unspecified
```

`State` の代わりに `{x.id: 1, y.id: 0}` のような辞書を直接渡すこともできます。

## 実行可能性・目的値・最適性を分けて読む

| 情報 | 分かること | それだけでは分からないこと |
|---|---|---|
| {attr}`~ommx.Solution.feasible` | 変数の取りうる値と制約を満たすか | 他にもっと良い候補があるか |
| {attr}`~ommx.Solution.objective` | この候補の目的値 | この候補が実行可能か、最適か |
| {attr}`~ommx.Solution.optimality` | 記録された最適性の状態 | `evaluate` 自体は最適性を証明しない |

上の候補は実行可能ですが、`evaluate` は他の候補を探索していないため、最適性は `Unspecified` です。ソルバーが最適性を報告した場合にはAdapterがその情報を付与しますが、評価だけで `Optimal` になることはありません。

実行不可能な候補でも、目的関数を計算できる場合は目的値を持ちます。

```{code-cell} ipython3
infeasible = instance.evaluate({x.id: 1, y.id: 1})
assert infeasible.objective == 2
assert not infeasible.feasible
```

最大化問題だからといって、この目的値2の候補を実行可能な目的値1の候補より良い解として選ぶことはできません。また、制約だけでなく、変数の値の種類（kind）や下限・上限（bound）も実行可能性の判定に含まれます。

浮動小数点による評価では許容誤差を使います。`evaluate(..., atol=...)` で指定でき、結果に用いた値は {attr}`~ommx.Solution.feasibility_atol` から取得できます。数値の正規化と検査の正確な規則は {meth}`~ommx.Instance.populate_state` のAPI Referenceを参照してください。

## どの条件を満たさなかったか調べる

Solutionは、Instanceの変数・制約の定義やメタデータに、評価値を加えて保持します。

```{code-cell} ipython3
infeasible.decision_variables_df()
```

決定変数のDataFrameでは、値の種類 `kind`・下限 `lower`・上限 `upper` に加えて、割り当てた値 `value` を確認できます。制約のDataFrameでは、式を評価した `value` と実行可能性を確認できます。

```{code-cell} ipython3
infeasible.constraints_df()
```

通常制約では、この値は制約の左辺 $f(x)$ の値です。等式なら0、不等式なら0以下であることが条件なので、`value` が常に違反量を表すわけではありません。Indicator・OneHot・SOS1の結果は `constraints_df(kind=...)` で選びます。各種類の意味と評価方法は[特殊制約](./special_constraints.md)を参照してください。

## 結果を後処理に使う

変数に名前と添字を付けている場合は、[結果を名前と添字で取り出す](../how_to/extract_named_values.md)手順で、商品・拠点・時点などに対応する値の辞書を得られます。変数IDを後処理に書き並べる必要はありません。

目的関数以外の指標をInstanceに名前付きの式として登録しておくと、Solutionからその評価値も読めます。[目的関数以外の指標を評価する](../how_to/evaluate_named_functions.md)では、登録から`named_functions_df()`や`extract_named_functions()`による取得までを説明します。これは候補に対する指標の値であり、新しい目的関数や実行可能性の判定条件を追加するものではありません。

(evaluating-transformed-instances)=
## 変換した問題の結果を読む

ソルバーに渡すためにInstanceを変換した場合は、**その変換済みInstanceで、返されたStateを評価します。** 変換済みInstanceは、元の変数の値を復元する情報や、確認すべき制約、出力する目的関数を持ちます。元のInstanceにソルバーのStateをそのまま渡しても、補助変数や依存関係を解釈できるとは限りません。

評価時には、Instanceに記録された固定値や、別の変数から値を計算する式（依存式）を使って、変数の値を復元します。それらがなく現在の問題にも使われない変数（irrelevant）は、省略すると既定値で補います。復元と補完の違いは{ref}`Instanceの変数の役割 <instance-variable-roles>`で説明しています。

この場合は、次の三つを区別してください。

- 値の復元・補完ができるか。
- 元の制約も含めて実行可能か。
- ソルバー側の目的値・最適性を、出力する問題の目的値・最適性として使えるか。

現在の問題で課す制約をactive、そこから外して評価のために保持する制約をremovedと呼びます。removedな制約を含む {attr}`~ommx.Solution.feasible` と、activeな制約だけを使う {attr}`~ommx.Solution.feasible_relaxed`、および出力目的関数との関係は[変換後の問題とRemoved constraints](./removed_constraints.md)で説明します。

複数の候補を同じ規則で評価する場合は[SampleSet](./sample_set.md)を使います。「得られた候補の中で最良」であることも、問題全体の最適性とは別です。
