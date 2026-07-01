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

# ommx.ParametricInstance

{class}`~ommx.ParametricInstance` は {class}`~ommx.Instance` と同じように数理モデルを表現するクラスですが、決定変数に加えてパラメータ（{class}`~ommx.Parameter`）を持つことができます。パラメータの値を決めるたびに `ommx.Instance` を生成することができるため、例えば目的関数や制約条件の一部の係数が異なる一連の数理モデル群を扱いたい場合などに便利です。同じ `ommx.ParametricInstance` から生成された `ommx.Instance` は決定変数や制約条件のIDを共有しているため、解の比較などが行いやすくなっています。

次のナップザック問題を考えましょう。

$$
\begin{aligned}
\text{maximize} \quad & \sum_{i=1}^{N} p_i x_i \\
\text{subject to} \quad & \sum_{i=1}^{N} w_i x_i \leq W \\
& x_i \in \{0, 1\} \quad (i=1, 2, \ldots, N)
\end{aligned}
$$

ここで、$N$はアイテムの数、$p_i$はアイテム$i$の価値、$w_i$はアイテム$i$の重さ、$W$はナップザックの容量です。$x_i$はアイテム$i$をナップザックに入れるかどうかを表すバイナリ変数です。`ommx.Instance` では $p_i$ や $w_i$ は固定値を使いましたが、ここではこれらをパラメータとして扱います。

```{code-cell} ipython3
from ommx import ParametricInstance, DecisionVariable, Parameter, Instance

N = 6
x = [DecisionVariable.binary(id=i, name="x", subscripts=[i]) for i in range(N)]

p = [Parameter(i +   N, name="Profit", subscripts=[i]) for i in range(N)]
w = [Parameter(i + 2*N, name="Weight", subscripts=[i]) for i in range(N)]
W =  Parameter(    3*N, name="Capacity")
```

`ommx.Parameter` もIDを持ちますが、これは `ommx.DecisionVariable` のIDと共通なので、重複しないようにする必要があります。決定変数と同じようにパラメータにも名前や添え字を付与できます。これらは決定変数と同じように `+` や `<=` で演算して `ommx.Function` や `ommx.Constraint` を作成することができます。

```{code-cell} ipython3
objective = sum(p[i] * x[i] for i in range(N))
constraint = sum(w[i] * x[i] for i in range(N)) <= W
```

これらを組み合わせてナップザック問題を表現する `ommx.ParametricInstance` を作りましょう。

```{code-cell} ipython3
parametric_instance = ParametricInstance.from_components(
    decision_variables=x,
    parameters=p + w + [W],
    objective=objective,
    constraints={0: constraint},
    sense=Instance.MAXIMIZE,
)
```

`ommx.Instance`と同様に `decision_variables_df()` 及び `constraints_df()` メソッドで決定変数と制約条件をDataFrameとして取得できますが、加えて `ommx.ParametricInstance` には `parameters_df()` メソッドがあります。これはパラメータの情報をDataFrameとして取得できます。

```{code-cell} ipython3
parametric_instance.parameters_df()
```

さて具体的なパラメータを指定してみましょう。それには `ParametricInstance.with_parameters` を使います。これは `ommx.Parameter` のIDをキー、値を値とする辞書を引数に取ります。

```{code-cell} ipython3
p_values = { x.id: value for x, value in zip(p, [10, 13, 18, 31, 7, 15]) }
w_values = { x.id: value for x, value in zip(w, [11, 15, 20, 35, 10, 33]) }
W_value = { W.id: 47 }

instance = parametric_instance.with_parameters({**p_values, **w_values, **W_value})
```

```{note}
`ommx.ParametricInstance` では $N$ のように決定変数やパラメータの数が変化するようなパラメータは扱えません。[JijModeling](https://jij-inc.github.io/JijModeling-Tutorials/ja/introduction.html)などのより高度なモデラーを使ってください。
```
