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

# 結果を名前と添字で取り出す

商品・拠点・時点などに対応する値を後処理するときは、変数IDを列挙する代わりに、
**変数の名前と添字をキーにして評価済みの値を取り出せます。** 名前と添字はモデル上の
ラベルであり、変数自体を識別するIDとは別です。

ここでは{ref}`SDKだけの環境 <install-python-sdk>`を使い、
小さなInstanceと候補をこのページだけで作ります。AdapterやTSPの実行結果は使いません。
既にSolutionを持っている場合も、抽出方法は同じです。

[Instance](../user_guide/instance.md)と、候補を評価して[Solution](../user_guide/solution.md)を得る操作を知っている読者向けです。名前・添字による抽出は、このページで説明します。

## 一つの名前に対応する値を取り出す

添字付きの変数$x_0, x_1$と、添字のない変数$y$を作ります。候補を評価したあと、
{meth}`~ommx.Solution.extract_decision_variables`で名前`"x"`の値だけを取り出します。

```{code-cell} ipython3
from ommx import Instance

instance = Instance.maximize()
x = [instance.new_binary("x", subscripts=[i]) for i in range(2)]
y = instance.new_integer("y", lower=0, upper=3)
instance.objective = 3 * x[0] + 5 * x[1] + y
instance.add_constraint(x[0] + x[1] <= 1)

solution = instance.evaluate({x[0].id: 1, x[1].id: 0, y.id: 2})
assert solution.feasible

x_values = solution.extract_decision_variables("x")
assert x_values == {(0,): 1.0, (1,): 0.0}
assert x_values[(0,)] == 1.0
```

辞書のキーは添字を並べたtupleです。1次元は`(i,)`、2次元は`(i, j)`となり、
添字のない変数は空のtuple`()`を使います。`(0,)`の末尾のカンマは、Pythonで
要素一つのtupleを表すために必要です。

## 名前付きの変数をまとめて取り出す

{meth}`~ommx.Solution.extract_all_decision_variables`は、名前ごとに同じ形式の辞書を
まとめます。例えば、後処理へ渡すデータを一度に作れます。

```{code-cell} ipython3
named_values = solution.extract_all_decision_variables()
assert named_values == {
    "x": {(0,): 1.0, (1,): 0.0},
    "y": {(): 2.0},
}
assert named_values["y"][()] == 2.0
```

名前を付けていない変数は、この辞書には含まれません。また、同じ名前・同じ添字の
変数が複数ある場合は、一つの値を勝手に選ばずエラーになります。名前・添字が同じでも
IDが異なれば別の変数です。区別が必要な場合は、{attr}`~ommx.Solution.state`の
IDごとの値を使います。存在しない名前や重複をどう扱うかは、
{meth}`~ommx.Solution.extract_decision_variables`と
{meth}`~ommx.Solution.extract_all_decision_variables`のAPI Referenceを参照してください。

## 表として調べる

名前のない変数も含めて確認したい場合や、条件を指定して絞り込みたい場合は、
{meth}`~ommx.Solution.decision_variables_df`を使えます。

```{code-cell} ipython3
variables = solution.decision_variables_df()
x_rows = variables.loc[variables["name"] == "x", ["name", "subscripts", "value"]]
assert x_rows["value"].to_dict() == {x[0].id: 1.0, x[1].id: 0.0}
x_rows
```

DataFrameの行のindexは変数IDです。`subscripts`列は`[0]`のような添字のリスト、
`value`列は評価済みの値です。辞書形式では添字をtupleのキーとして使うため、
この列をそのまま辞書のキーにする必要はありません。

複数候補を扱う場合は、[SampleSet](../user_guide/sample_set.md)で対象候補を選びます。
{meth}`~ommx.SampleSet.extract_decision_variables`と
{meth}`~ommx.SampleSet.extract_all_decision_variables`では、それぞれ
`(name, sample_id)`、`(sample_id)`を指定します。

(reuse-named-values)=
## 別のInstanceへ値を対応付ける場合

```{note}
以下の`state_from_named_values()`は開発版の機能です。上の結果抽出はBookの公開SDK環境で
使えますが、この変換は含まれていません。以下は、SDKとReferenceのリビジョンを揃えた
**開発版の環境を既に用意している読者向けの参照例**です。公開SDKのインストール手順から
そのまま実行する例ではありません。[実行環境とAPI Referenceの差](../getting_started/compatibility.md)も
参照してください。結果抽出だけのために環境を変更する必要はありません。
```

Instanceを作り直すと、同じ名前の変数でもIDが同じとは限りません。
{meth}`~ommx.Instance.state_from_named_values`は、受け取る側のInstanceの名前・添字に
照合して、IDをキーとするStateを作ります。次は変数の作成順と$y$の上限を変えた例です。

```python
target = Instance.maximize()
target_y = target.new_integer("y", lower=0, upper=1)
target_x1 = target.new_binary("x", subscripts=[1])
target_x0 = target.new_binary("x", subscripts=[0])
target.objective = 3 * target_x0 + 5 * target_x1 + target_y
target.add_constraint(target_x0 + target_x1 <= 1)

state = target.state_from_named_values(named_values)
assert state.entries == {target_x0.id: 1.0, target_x1.id: 0.0, target_y.id: 2.0}
assert not target.evaluate(state).feasible
```

名前と添字が対応しても、元の候補が新しい問題で実行可能とは限りません。この例では
$y=2$が新しい上限1を超えます。**照合は値の妥当性や実行可能性の検査ではない**ため、
必要な値がそろった候補を対象Instanceで評価します。名前なしの変数や、抽出から省かれた
値を自動で移す操作でもありません。

照合できない名前・添字や曖昧なラベルはエラーになります。一方、入力にない変数の値は
埋めません。部分的な入力とエラーの正確な条件は
{meth}`~ommx.Instance.state_from_named_values`のAPI Referenceで確認してください。

探索の初期候補として使う場合は、[Adapterへ初期候補を渡す手順](./use_initial_state.md)へ
進んでください。初期候補を受け取れるかどうかは、各Adapterの契約に従います。
