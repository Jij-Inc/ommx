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

```{warning}
このドキュメントはOMMX Python SDK 1.8.0のリリース時のものであり、Python SDK 2.0.0以降では動作しません。
```

+++

# OMMX Python SDK 1.8.0

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_1.8.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-1.8.0)

個々の変更についてはGitHub Releaseを参照してください。

⚠️ `SolverAdapter`の追加による破壊的変更が含まれています。

Summary
--------
- ソルバー用の共通インターフェイスとして、新しく`SolverAdapter`基底クラスが追加されました base class to serve as a common interface for adapters to different solvers.
- `ommx-python-mip-adapter`と`ommx-pyscipopt-adapter`は　[Adapter実装ガイド](https://jij-inc.github.io/ommx/ja/ommx_ecosystem/solver_adapter_guide.html)に基づいて、`SolverAdapter`を使うように更新されました。
  - ⚠️　破壊的変更です。このadapterを使用しているコードは更新が必要となります。
  - 他のadapterは今後更新予定

+++

# Solver Adapter 

`SolverAdapter`は各adapterのAPIの一貫性を高めるために追加された抽象基底クラスです。`ommx-python-mip-adapter`と`ommx-pyscipopt-adapter`は`SolverAdapter`を使うように修正されました。

新しいAdapterインターフェイスで簡単な解き方の例を見てみましょう。

```{code-cell} ipython3
from ommx.v1 import Instance, DecisionVariable
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

p = [10, 13, 18, 32, 7, 15]
w = [11, 15, 20, 35, 10, 33]
x = [DecisionVariable.binary(i) for i in range(6)]
instance = Instance.from_components(
    decision_variables=x,
    objective=sum(p[i] * x[i] for i in range(6)),
    constraints=[sum(w[i] * x[i] for i in range(6)) <= 47],
    sense=Instance.MAXIMIZE,
)

solution = OMMXPySCIPOptAdapter.solve(instance)
solution.objective
```

このリリースによって、`OMMXPythonMIPAdapter`クラスを使った場合でも上記と同じ書き方ができるようになりました。

以前の`instance_to_model()`を使ったコードを書き換える場合は, Adapterのインスタンスを作って`solver_input`を利用できます。 ソルバーのパラメータ変更などして、手動でを最適化を行ってから、`decode()`でOMMXの`Solution`を取得できます。

```{code-cell} ipython3
adapter = OMMXPySCIPOptAdapter(instance)
model = adapter.solver_input # OMMXPySCIPOptAdapterの場合、これは`pyscipopt.Model`です
# パラメータの変更
model.optimize() 
solution = adapter.decode(model)
solution.objective
```
