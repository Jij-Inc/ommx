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

# 目的関数以外の指標を評価する

費用を最小化しながら、移動距離や稼働時間も候補ごとに確認したい場合は、{class}`~ommx.NamedFunction`として評価用の式をInstanceに登録します。**登録するだけでは、その式は目的関数や制約になりません。** 候補を評価すると、目的値とは別の指標として値を取り出せます。

## 実行環境と前提

{ref}`Python SDKのインストール <install-python-sdk>`までを済ませてください。このページは[Instance](../user_guide/instance.md)と、候補を`evaluate()`で評価するとSolutionを得ることを知っている読者向けです。コードはこのページだけで実行できます。

## 1. 評価用の式を登録し、候補を評価する

二つの選択肢から一つを選びます。目的関数は費用、`distance`は後で確認する移動距離です。ここでは既存の変数や式をまとめる{meth}`~ommx.Instance.from_components`の`named_functions`へ、名前付きの式を渡します。

```{code-cell} ipython3
from ommx import DecisionVariable, Instance, NamedFunction, Sense

x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(2)]
instance = Instance.from_components(
    sense=Sense.Minimize,
    objective=x[0] + 2 * x[1],
    decision_variables=x,
    constraints={0: sum(x) == 1},
    named_functions=[
        NamedFunction(id=0, function=10 * x[0] + 3 * x[1], name="distance"),
    ],
)

solution = instance.evaluate({x[0].id: 1, x[1].id: 0})
assert solution.feasible
assert solution.objective == 1
assert solution.extract_named_functions("distance") == {(): 10.0}
```

この候補の費用は1、距離は10です。距離が短いかどうかは、目的値や実行可能性の判定には使っていません。`id=0`は名前付き評価式のIDで、変数や制約のIDとは別です。式が参照する変数も、`decision_variables`に含めます。

{meth}`~ommx.Solution.extract_named_functions`は、指定した名前を持つ評価式の値を、添字のtupleをキーとする辞書にします。この例は添字を付けていないため、キーは空のtuple`()`です。例えば設備ごとの指標なら、同じ名前に異なる`subscripts`を付けて登録できます。

## 2. 表で指標を確認する

{meth}`~ommx.Solution.named_functions_df`では、評価値と名前などをDataFrameで確認できます。

```{code-cell} ipython3
metrics = solution.named_functions_df()
assert metrics.loc[0, "name"] == "distance"
assert metrics.loc[0, "value"] == 10.0
metrics[["name", "value"]]
```

`value`列が指標の評価値で、行のindexは評価式のIDです。後処理の計算へ渡すなら名前・添字の辞書、複数の指標を一覧するならDataFrameを使えます。

## 複数候補でも同じ指標を使う

{meth}`~ommx.Instance.evaluate_samples`でまとめて評価する場合も、登録した式が評価されます。指標を取り出すときは、対象の候補をサンプルIDで指定します。

```{code-cell} ipython3
sample_set = instance.evaluate_samples({
    0: {x[0].id: 1, x[1].id: 0},
    1: {x[0].id: 0, x[1].id: 1},
})
assert sample_set.extract_named_functions("distance", 0) == {(): 10.0}
assert sample_set.extract_named_functions("distance", 1) == {(): 3.0}
```

費用を最小にする候補と距離を最小にする候補は、この例では異なります。指標を登録しても、候補の並べ替えや{attr}`~ommx.SampleSet.best_feasible`の選択基準は変わりません。複数候補の読み方は[SampleSet](../user_guide/sample_set.md)を参照してください。

## 評価式だけに現れる変数の値は明示する

**名前付き評価式だけが参照する変数は、それだけではusedな変数になりません。** usedとは、現在の目的関数や現在課している制約に現れる変数です。例えば、目的関数・制約には現れないInteger変数を、指標にだけ使う場合を考えます。

```{code-cell} ipython3
recorded = DecisionVariable.integer(0, lower=2, upper=5, name="recorded")
reporting = Instance.from_components(
    sense=Sense.Minimize,
    objective=0,
    decision_variables=[recorded],
    constraints={},
    named_functions=[NamedFunction(id=0, function=recorded, name="recorded")],
)

omitted = reporting.evaluate({})
explicit = reporting.evaluate({recorded.id: 4})
assert omitted.extract_named_functions("recorded") == {(): 2.0}
assert explicit.extract_named_functions("recorded") == {(): 4.0}
```

この変数は、固定値やほかの変数への依存式も持たないためirrelevantです。値を省略した最初の評価では既定値2が補われ、その値で指標を計算しています。これは元の候補から復元した値ではありません。手元の候補が4を持つなら、二つ目のようにその値を渡してください。

指標にしか使わない変数でも、値を渡せば、その変数の種類や上下限に照らして検査されます。「指標は制約ではない」ことと「変数の定義域を無視できる」ことは別です。used・fixed・dependent・irrelevantの違いと補完規則は[変数の役割](../user_guide/variable_roles.md)、候補全体の検査は{meth}`~ommx.Instance.evaluate`で説明します。

添字の重複や存在しない名前の扱い、複数候補からの抽出など、正確な操作条件は{class}`~ommx.NamedFunction`、{meth}`~ommx.Solution.extract_named_functions`、{meth}`~ommx.SampleSet.extract_named_functions`のAPI Referenceを参照してください。
