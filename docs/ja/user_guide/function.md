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

# ommx.Function

数理最適化では目的関数や制約条件を表現するために（数学的な意味での）関数を扱う必要があります。OMMX は多項式をコンパクトに保持し、それらに絶対値、最小値、除算、整数べきなどのスカラー演算を組み合わせられます。

| データ構造 | 説明 |
| --- | --- |
| {class}`~ommx.Linear` | 線形の関数。決定変数のIDとその係数のペアを持つ |
| {class}`~ommx.Quadratic` | 二次の関数。決定変数のIDのペアとその係数のペアを持つ |
| {class}`~ommx.Polynomial` | 多項式。決定変数のIDの組とその係数のペアを持つ |
| {class}`~ommx.Function` | 多項式またはスカラー演算を組み合わせた式 |


## ommx.Function の作成
Python SDKでこれらのデータ構造を作る場合、大きく分けて二つの方法があります。まずひとつ目は、各データ構造のコンストラクタを直接呼び出す方法です。たとえば、次のようにして`ommx.Linear`を作ることができます。

```{code-cell} ipython3
from ommx import Linear

linear = Linear(terms={1: 1.0, 2: 2.0}, constant=3.0)
print(linear)
```

このように決定変数はIDで識別され、係数は実数で表されます。係数や定数値にアクセスするには `terms` や `linear_terms` および `constant_term` プロパティを使います。

```{code-cell} ipython3
print(f"{linear.terms=}")
print(f"{linear.linear_terms=}")
print(f"{linear.constant_term=}")
```

もう一つの方法は `ommx.DecisionVariable` から作る方法です。`ommx.DecisionVariable` は決定変数のIDを持つだけのデータ構造です。`ommx.Linear` などの多項式を作る際には、`ommx.DecisionVariable` を使って決定変数を作り、それを使って多項式を作ることができます。

```{code-cell} ipython3
from ommx import DecisionVariable

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

`Linear`, `Quadratic`, `Polynomial` はそれぞれ固有のデータの保持方法を持っているため別のMessageになっていますが、目的関数や制約条件として共通に扱うための `Function` が用意されています。`Function` は、これらの多項式に加えてスカラー演算を組み合わせた式も保持します。

```{code-cell} ipython3
from ommx import Function

# Constant
print(Function(1.0))
# Linear
print(Function(linear))
# Quadratic
print(Function(q))
# Polynomial
print(Function(p))
```

## スカラー関数の組み合わせ

複合演算を適用する前に、算術式を `Function` に変換します。Python の `abs` と算術演算子は複合式を構築し、点ごとの最小値・最大値には `minimum`・`maximum` メソッドを使います。

```{code-cell} ipython3
fx = Function(x)
fy = Function(y)

absolute = abs(fx - 3)
sign = (fx - 1).signum()
minimum = fx.minimum(fy)
maximum = fx.maximum(fy)
quotient = fx / (fy + 1)
power = fx**2
same_power = fx.powi(2)

print(absolute)
print(minimum)
print(quotient)
print(power)
print(same_power)
```

Python 組み込みの `min(fx, fy)` と `max(fx, fy)` は比較演算を行うため、点ごとの演算式を構築しません。代わりに `fx.minimum(fy)` と `fx.maximum(fy)` を使ってください。

`fx**n` と `fx.powi(n)` は同じ演算であり、`n` は符号付き 32 bit 整数の範囲で指定します。浮動小数や関数値の指数、および `2**fx` のような反転累乗はサポートしません。

複合式ではオペランドを左から右へ処理し、結合的な演算も一つの適用ごとにちょうど 2 個の値を組み合わせます。そのため、`(abs(a) + abs(b)) + abs(c)` と `abs(a) + (abs(b) + abs(c))` はどちらも括弧と演算順序を保持します。オーバーフローや未定義演算のエラーは、その表現された順序で発生します。protobuf の wire format は、この複合式を再帰的にネストした tree ではなく、flat な逆ポーランド記法（RPN）の命令列として保存します。多項式だけからなる算術式は、引き続きコンパクトな多項式表現へ正規化されます。

複合関数は実数値として次の規約で評価されます。

- `signum(0)` は符号付きゼロも含めて `0` です。
- 分母がゼロの除算は未定義です。
- ゼロを負の整数で累乗する演算は未定義です。`0**0` は `1` と定義します。
- 指数は常に整数なので、負の底もサポートします。
- 未定義の演算や非有限の中間結果は、Python では `ValueError` になります。

除算と負の整数べきによって `Function` は部分関数になり得ます。代数的な簡約でも定義域は保存されるため、例えば `0 * (1 / fx)` は `fx == 0` の点で引き続き未定義です。

```{code-cell} ipython3
try:
    (1 / fx).evaluate({1: 0})
except ValueError as e:
    print(f"Error: {e}")
```

多項式のメタデータは、`Function` がコンパクトな多項式表現を使っている場合に限って利用できます。複合式では `degree()` と `num_terms()` は `None` を返し、`terms` などの係数プロパティは `TypeError` を送出します。恒等演算や定数の畳み込みを除き、指数が非負でも整数べきは複合式のまま保持されます。展開済みのコンパクトな多項式が必要な場合は、明示的な乗算を使ってください。ソルバーアダプターも、宣言した input class が複合式をサポートしない場合は受け付けません。

## 決定変数の代入・部分評価

`Function` と各多項式型は決定変数の値を代入する `evaluate` メソッドを持ちます。例えば上で作った線形関数 $x_1 + 2x_2 + 3$ に $x_1 = 1, x_2 = 0$ を代入すると $1 + 2 \times 0 + 3 = 4$ となります。

```{code-cell} ipython3
value = linear.evaluate({1: 1, 2: 0})
print(f"{value=}")
```

引数は `dict[int, float]` の形式と `ommx.State` をサポートしています。`evaluate` は評価に必要な決定変数のIDが足りない場合はエラーを返します。

```{code-cell} ipython3
try:
    linear.evaluate({1: 1})
except ValueError as e:
    print(f"Error: {e}")
```

一部の決定変数にだけ値を代入したい場合は `partial_evaluate` メソッドを使います。これは `evaluate` と同じ引数を受け取りますが、値が代入されていない決定変数については評価せずにそのまま返します。

```{code-cell} ipython3
linear2 = linear.partial_evaluate({1: 1})
print(f"{linear2=}")
```

`Linear`、`Quadratic`、`Polynomial` の部分評価では元の Python 型が保たれます。複合 `Function` では未代入の変数に依存する式構造を保持し、すべてのオペランドの値が決まると compact な定数 `Function` に畳み込まれます。

+++

## 係数の比較

`Function` と各多項式型には `almost_equal` 関数が用意されています。多項式では各係数が指定された誤差で一致するかを判定します。複合式では同じ式構造を比較するものであり、数学的な大域同値性を証明するものではありません。例えば $ (x + 1)^2 = x^2 + 2x + 1 $ であることを確認するには次のように書きます。

```{code-cell} ipython3
xx = (x + 1) * (x + 1)
xx.almost_equal(x * x + 2 * x + 1)
```
