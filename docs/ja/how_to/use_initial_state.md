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

# Adapterに初期候補を渡す

以前の計算結果や手元の候補を、探索の開始に利用したい場合の手順です。
ここでは[SDKとPySCIPOpt Adapter](../getting_started/install.md)を使い、
小さな問題と初期候補をこのページだけで作ります。ほかのTutorialの実行は不要です。

## 候補を評価してから渡す

`initial_state`には、変数IDをキー、値を要素とする辞書、または
{py:class}`~ommx.State`を渡せます。これは最終結果を指定する引数ではなく、
backendに提案する候補です。

```{code-cell} ipython3
from ommx import Instance
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

instance = Instance.maximize()
x = instance.new_integer("x", lower=0, upper=5)
y = instance.new_integer("y", lower=0, upper=5)
instance.objective = x - y
instance.add_constraint(x + y <= 5)

initial_state = {x.id: 3.0, y.id: 2.0}
initial = instance.evaluate(initial_state)
assert initial.feasible
assert initial.objective == 1

solution = OMMXPySCIPOptAdapter.solve(instance, initial_state=initial_state)
assert solution.feasible
assert solution.objective == 5
```

初期候補の目的値は1、返った候補の目的値は5です。返った値は`Solution`で読みます。
候補の提示が必ず探索を速めるとは限らず、この例は速度比較ではありません。

別のAdapterでは初期候補への対応や省略値の扱いが異なります。利用するAdapterの
`initial_state`のAPI契約（例：{py:meth}`~ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter.solve`）を確認してください。特に部分的な候補が、OMMXの
`Instance.evaluate()`で必要な値をすべて含むとは限りません。

## Stateとして渡す場合

```{code-cell} ipython3
from ommx import State

state = State(entries=initial_state)
solution = OMMXPySCIPOptAdapter.solve(instance, initial_state=state)
assert solution.feasible
```

(named-initial-state)=
## 名前と添字から値を対応付ける

モデルを作り直すとIDが変わる場合は、{py:meth}`~ommx.Instance.state_from_named_values`で
名前と添字から`State`を作れます。上の例と同じSDK・Adapterの環境を使います。
スカラーの添字は`()`、1次元は`(i,)`です。

```python
named_values = {"x": {(): 3.0}, "y": {(): 2.0}}
state = instance.state_from_named_values(named_values)
solution = OMMXPySCIPOptAdapter.solve(instance, initial_state=state)
```

同じ名前・添字が複数の変数に一致する場合はIDで指定します。対応する変数が見つからない
場合も自動的に別の変数へ割り当てられるわけではありません。照合の正確な規則や例外は
{py:meth}`~ommx.Instance.state_from_named_values`を参照してください。

大きな問題でも手順は同じです。まず候補の出所と対象Instanceを対応させ、評価できる場合は
評価してから渡します。大きな辞書をページに貼る代わりに、候補のファイルからの読み込みや
{py:attr}`~ommx.Solution.state`の再利用を検討してください。
