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

# ommx.v1.Function

数理最適化では目的関数や制約条件を表現するために（数学的な意味での）関数を扱う必要があります。OMMXでは特に多項式を中心に扱い、OMMX Messageには多項式を表すためのデータ構造として以下のものが存在します。

| データ構造 | 説明 |
| --- | --- |
| [ommx.v1.Linear](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Linear) | 線形の関数。決定変数のIDとその係数のペアを持つ |
| [ommx.v1.Quadratic](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Quadratic) | 二次の関数。決定変数のIDのペアとその係数のペアを持つ |
| [ommx.v1.Polynomial](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Polynomial) | 多項式。決定変数のIDの組とその係数のペアを持つ |
| [ommx.v1.Function](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Function) | 上記のいずれかあるいは定数 |


## ommx.v1.Function の作成
Python SDKでこれらのデータ構造を作る場合、大きく分けて二つの方法があります。まずひとつ目は、各データ構造のコンストラクタを直接呼び出す方法です。たとえば、次のようにして`ommx.v1.Linear`を作ることができます。

```{code-cell} ipython3
from ommx.v1 import Linear

linear = Linear(terms={1: 1.0, 2: 2.0}, constant=3.0)
print(linear)
```

このように決定変数はIDで識別され、係数は実数で表されます。係数や定数値にアクセスするには `terms` や `linear_terms` および `constant_term` プロパティを使います。

```{code-cell} ipython3
print(f"{linear.terms=}")
print(f"{linear.linear_terms=}")
print(f"{linear.constant_term=}")
```

もう一つの方法は `ommx.v1.DecisionVariable` から作る方法です。`ommx.v1.DecisionVariable` は決定変数のIDを持つだけのデータ構造です。`ommx.v1.Linear` などの多項式を作る際には、`ommx.v1.DecisionVariable` を使って決定変数を作り、それを使って多項式を作ることができます。

```{code-cell} ipython3
from ommx.v1 import DecisionVariable

x = DecisionVariable.binary(1, name="x")
y = DecisionVariable.binary(2, name="y")

linear = x + 2.0 * y + 3.0
print(linear)
```

このとき多項式のデータ型は決定変数に関するID以外の情報を保持しないことに注意してください。上の例で言えば `x` や `y` といった `DecisionVariable.binary` に渡した情報は `Linear` には伝わりません。この二つ目の方法はどの次数の多項式も作ることができます。

```{code-cell} ipython3
q = x * x + x * y + y * y
print(q)
```

```{code-cell} ipython3
p = x * x * x + y * y
print(p)
```

`Linear`, `Quadratic`, `Polynomial` はそれぞれ固有のデータの保持方法を持っているため別のMessageになっていますが、目的関数や制約条件としてはどれを使ってもいいので、それらのいずれかあるいは定数である `Function` というMessageが用意されています。

```{code-cell} ipython3
from ommx.v1 import Function

# Constant
print(Function(1.0))
# Linear
print(Function(linear))
# Quadratic
print(Function(q))
# Polynomial
print(Function(p))
```

## 決定変数の代入・部分評価

`Function` 及び他の多項式は決定変数の値を代入する `evaluate` メソッドを持ちます。例えば上で作った線形関数 $x_1 + 2x_2 + 3$ に $x_1 = 1, x_2 = 0$ を代入すると $1 + 2 \times 0 + 3 = 4$ となります。

```{code-cell} ipython3
value = linear.evaluate({1: 1, 2: 0})
print(f"{value=}")
```

引数は `dict[int, float]` の形式と `ommx.v1.State` をサポートしています。`evaluate` は評価に必要な決定変数のIDが足りない場合はエラーを返します。

```{code-cell} ipython3
try:
    linear.evaluate({1: 1})
except RuntimeError as e:
    print(f"Error: {e}")
```

一部の決定変数にだけ値を代入したい場合は `partial_evaluate` メソッドを使います。これは `evaluate` と同じ引数を受け取りますが、値が代入されていない決定変数については評価せずにそのまま返します。

```{code-cell} ipython3
linear2 = linear.partial_evaluate({1: 1})
print(f"{linear2=}")
```

部分評価された結果は多項式になるため、元の多項式と同じ型で返されます。

+++

## 係数の比較

`Function` や他の多項式型には `almost_equal` 関数が用意されています。これは多項式の各係数が指定された誤差で一致するかどうかを判定するための関数です。例えば $ (x + 1)^2 = x^2 + 2x + 1 $ であることを確認するには次のように書きます

```{code-cell} ipython3
xx = (x + 1) * (x + 1)
xx.almost_equal(x * x + 2 * x + 1)
```
