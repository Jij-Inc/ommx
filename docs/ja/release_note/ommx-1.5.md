---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: .venv
  language: python
  name: python3
---

```{warning}
このドキュメントはOMMX Python SDK 1.5.0のリリース時のものであり、Python SDK 2.0.0以降では動作しません。
```

+++

# OMMX Python SDK 1.5.0

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_1.5.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-1.5.0)

このノートブックでは新機能について説明します。詳細についてはGitHubのリリースノートを参照してください。

+++

## 評価と部分評価

OMMXの最初のリリースから、`ommx.v1.Instance`は`evaluate`メソッドをサポートしており、`Solution`メッセージを生成します。

```{code-cell} ipython3
from ommx.v1 import Instance, DecisionVariable

# OMMX APIのインスタンスを作成
x = DecisionVariable.binary(1)
y = DecisionVariable.binary(2)

instance = Instance.from_components(
    decision_variables=[x, y],
    objective=x + y,
    constraints=[x + y <= 1],
    sense=Instance.MINIMIZE
)
solution = instance.evaluate({1: 1, 2: 0})
```

```{code-cell} ipython3
solution.decision_variables
```

Python SDK 1.5.0から、`Function`とその基本クラスである`Linear`、`Quadratic`、`Polynomial`も`evaluate`メソッドをサポートします。

```{code-cell} ipython3
f = 2*x + 3*y
value, used_ids = f.evaluate({1: 1, 2: 0})
print(f"{value=}, {used_ids=}")
```

これにより、関数の評価値と使用された意思決定変数のIDが返されます。いくつかの意思決定変数が不足している場合、`evaluate`メソッドは例外を発生させます。

```{code-cell} ipython3
try:
    f.evaluate({3: 1})
except RuntimeError as e:
    print(e)
```

さらに、`partial_evaluate`メソッドもあります。

```{code-cell} ipython3
f2, used_ids = f.partial_evaluate({1: 1})
print(f"{f2=}, {used_ids=}")
```

これにより、`x = 1`を代入することで新しい関数が作成されます。`partial_evaluate`は`ommx.v1.Instance`クラスにも追加されています。

```{code-cell} ipython3
new_instance = instance.partial_evaluate({1: 1})
new_instance.objective
```

このメソッドは、特定の決定変数を固定して `ommx.v1.Instance` を作成するのに役立ちます。
