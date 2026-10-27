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

# Function：目的関数と制約の式

{class}`~ommx.Function` は、変数の値を数値へ写す式です。目的関数や通常制約の本体に使います。**式を作ること、その値を計算すること、その式を含む問題をソルバーが扱えることは別です。** このページでは式の構築・評価と、入力形式を選ぶ際の注意を説明します。

## 式を作り、値を計算する

決定変数に算術演算を適用して式を作れます。次は $f(x,y)=x+2y+3$ です。

```{code-cell} ipython3
from ommx import Function, Instance

instance = Instance.minimize()
x = instance.new_binary("x")
y = instance.new_binary("y")
f = Function(x + 2 * y + 3)

assert f.evaluate({x.id: 1, y.id: 0}) == 4.0
instance.objective = f
instance.add_constraint(x + y <= 1)
```

{meth}`~ommx.Function.evaluate` は変数IDと値の辞書、または {class}`~ommx.State` を受け取ります。式に必要なIDの値が足りなければ評価できません。Function単体は変数の名前・種類・boundを保持せず、IDを参照します。変数の定義域と制約も含めて候補を検査したい場合は、Functionではなく {meth}`~ommx.Instance.evaluate` を使います。

一部の変数の値だけを決めるには {meth}`~ommx.Function.partial_evaluate` を使います。残った変数の式を返し、元の式は変えません。

```{code-cell} ipython3
remaining = f.partial_evaluate({x.id: 1})
assert remaining.evaluate({y.id: 0}) == 4.0
assert remaining.evaluate({y.id: 1}) == 6.0
```

## 多項式として組み立てる

加算・減算・乗算を使い、二次式や、より高次の多項式も作れます。変数IDと係数から直接作る型もあります。

| 型 | 表す式 |
|---|---|
| {class}`~ommx.Linear` | 線形式 |
| {class}`~ommx.Quadratic` | 二次以下の多項式 |
| {class}`~ommx.Polynomial` | 任意次数の多項式 |
| {class}`~ommx.Function` | これらの多項式、または後述のスカラー演算を組み合わせた式 |

```{code-cell} ipython3
from ommx import Linear

by_coefficients = Linear(terms={x.id: 1, y.id: 2}, constant=3)
assert Function(by_coefficients).almost_equal(f)

quadratic = Function(x * x + 2 * x * y + y * y)
assert quadratic.degree() == 2
assert quadratic.evaluate({x.id: 1, y.id: 1}) == 4.0
```

多項式の次数や係数は、`degree()`、`terms`、`linear_terms`、`quadratic_terms`、`constant_term`などで調べられます。どの型にどの属性があるかは各型のAPI Referenceを参照してください。

{meth}`~ommx.Function.almost_equal` は多項式の係数を許容誤差内で比較します。一般の式では式構造の比較であり、任意の二つの式がすべての点で同じ値を取るかを判定する機能ではありません。

## スカラー演算を組み合わせる

Functionには絶対値、符号、点ごとの最小・最大、除算、整数べきも適用できます。これらは式を構築する操作で、この時点で変数の値を決めるものではありません。

```{code-cell} ipython3
fx = Function(x)
fy = Function(y)

absolute = abs(fx - 3)
minimum = fx.minimum(fy)
quotient = fx / (fy + 1)
power = fx**2

assert absolute.evaluate({x.id: 1}) == 2.0
assert minimum.evaluate({x.id: 1, y.id: 0}) == 0.0
assert quotient.evaluate({x.id: 1, y.id: 1}) == 0.5
assert power.evaluate({x.id: 1}) == 1.0
```

点ごとの最小値・最大値には`minimum()`・`maximum()`を使います。Python組み込みの`min()`・`max()`はその代わりにはなりません。整数べきには`**`または`powi()`を使います。

除算や負の整数べきは、値を計算できない入力を持つことがあります。評価の`atol`は、分母などをゼロとみなす判断にも使われます。異なる許容誤差で同じ結果になるとは限りません。演算ごとの定義域・数値条件・例外は[Functionの契約](../reference/function_contract.md)を参照してください。

## 数学的な次数と、保持する表現形式は別

ソルバー向けに使う場合は、数学的な式だけでなく、Functionが保持する表現形式も確認します。例えば、Functionの`g ** 2`は自動的に展開済み多項式にはなりません。多項式としての二乗が必要なら`g * g`を使います。

```{code-cell} ipython3
g = Function(x + y)
composed_square = g**2
polynomial_square = g * g

assert composed_square.degree() is None
assert polynomial_square.degree() == 2
assert polynomial_square.almost_equal(quadratic)
```

上の二つは同じ値を計算できますが、多項式表現を要求する入力条件への適合は異なります。複合式の`degree()`は`None`で、係数属性も多項式と同じようには使えません。

このBookで扱うSDKのInstanceClassは、目的関数や制約本体に多項式表現を要求します。したがって、Functionとして評価できる式が、そのまま掲載Adapterの入力に使えるとは限りません。[InstanceClassと入力条件](./instance_class.md)で、要求する表現へのmembershipを確認できます。Preparationは任意の複合式を自動的に多項式へ展開する仕組みではありません。

## 詳細の参照先

数値のゼロ判定、演算順序、部分評価時に残す演算、係数の正規化は[Functionの契約](../reference/function_contract.md)にまとめています。区間評価には {meth}`~ommx.Function.evaluate_bound` を使いますが、返る区間が実際の最小値・最大値に一致するとは限りません。引数・戻り値と区間評価の条件は各メソッドのAPI Referenceを参照してください。

他言語で式を保存・交換する場合のRPN命令列などのwire表現は、[Message仕様](https://github.com/Jij-Inc/ommx/tree/python-3.0.0b7/proto)を参照してください。表現形式の詳細は、Pythonで目的関数を作って候補を評価するための前提ではありません。
