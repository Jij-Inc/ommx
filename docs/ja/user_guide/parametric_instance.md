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

# ParametricInstance：固定構造の問題を再利用する

{class}`~ommx.ParametricInstance` は、係数や定数の一部をパラメータとして残した問題です。**変数や制約の構造を固定したまま、数値を変えたInstanceを作る**ときに使います。ここでは、Instanceの変数・目的関数・制約の意味を知っている読者向けに、問題族と具体的な問題の違いを説明します。

## パラメータを決めてInstanceを作る

二つの品目から得点が高くなるように選び、選ぶ個数の上限だけを変える問題を考えます。`capacity`はソルバーが選ぶ決定変数ではなく、問題を作る側が指定するパラメータです。

```{code-cell} ipython3
from ommx import DecisionVariable, Parameter, ParametricInstance, Sense

xs = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(2)]
capacity = Parameter(2, name="capacity")

family = ParametricInstance.from_components(
    decision_variables=xs,
    parameters=[capacity],
    objective=3 * xs[0] + 2 * xs[1],
    constraints={0: xs[0] + xs[1] <= capacity},
    sense=Sense.Maximize,
)

one_item = family.with_parameters({capacity.id: 1})
two_items = family.with_parameters({capacity.id: 2})
```

{meth}`~ommx.ParametricInstance.with_parameters` に渡す辞書は、パラメータIDをキー、具体的な値を値とします。結果は通常の {class}`~ommx.Instance` なので、評価・Preparation・Adapter実行に使えます。元のParametricInstanceは残り、別の値を指定して再利用できます。

```{code-cell} ipython3
candidate = {xs[0].id: 1, xs[1].id: 1}
assert not one_item.evaluate(candidate).feasible
assert two_items.evaluate(candidate).feasible
assert two_items.evaluate(candidate).objective == 5.0

assert {v.id for v in one_item.decision_variables} == {
    v.id for v in two_items.decision_variables
}
```

同じ候補を評価しても、上限の値によって実行可能性が変わります。変数・制約のIDを共有するため、異なる数値条件での結果を同じ品目・制約に対応付けて読めます。ただし、IDが同じことは実行可能性や最適解が同じであるという意味ではありません。

## パラメータもIDで識別する

Parameterは決定変数と同じ式の中で使えますが、役割が異なります。パラメータはInstanceを作る前に指定する値、決定変数は候補に値を割り当てる対象です。両者のIDは同じ空間を使うため、重複させません。上の例では決定変数が0・1、パラメータが2です。

名前や添字などは {meth}`~ommx.ParametricInstance.parameters_df` で確認できます。指定すべきパラメータや値の検査の詳細は、{meth}`~ommx.ParametricInstance.with_parameters`のAPI Referenceを参照してください。

## モデラーとの境界

この例では、品目数を2に固定しています。品目数が変わる入力に応じて変数や制約を増減させるような、構造自体の生成はParametricInstanceのパラメータでは表せません。その場合は、Pythonでモデルを組み立て直すか、構造を生成するモデラーを使います。外部モデラーからの受け取り口は[既存の問題をInstanceとして読み込む](../how_to/load_existing_instance.md)を参照してください。

ParametricInstanceを使うためにExperimentは必要ありません。生成した問題の条件と結果を後から整理したい場合だけ、[試行の記録](../tutorial/experiment_management)へ進めます。
