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

# 制約から変数の上下限を絞る

変数の上下限（bound）を、制約から分かる範囲に狭めたい場合は、{meth}`~ommx.Instance.tighten_bounds_simultaneously_once`を使います。**Instanceを直接更新し、変更された変数のIDと新しいboundを返す、一回分の操作です。** 変数の値を求めたり、収束まで自動で繰り返したりする操作ではありません。

このページは、変数の上下限と通常の等式・不等式を知っている読者向けです。{ref}`Python SDKのインストール <install-python-sdk>`を済ませてください。ほかのTutorialの実行状態やAdapterは使いません。

## 1. 変更前のboundを確認する

整数変数$x,y$を、それぞれ$0$以上$10$以下で作ります。制約は$x\leq 2$と$y\leq x$です。

```{code-cell} ipython3
from ommx import Bound, Instance

instance = Instance.minimize()
x = instance.new_integer("x", lower=0, upper=10)
y = instance.new_integer("y", lower=0, upper=10)
instance.add_constraint(x <= 2)
instance.add_constraint(y <= x)

assert x.bound == Bound(0, 10)
assert y.bound == Bound(0, 10)
```

## 2. 一回実行し、変更されたboundを読む

戻り値は、変更のあった変数だけを含む辞書です。Instanceが保持するboundも同時に更新されます。

```{code-cell} ipython3
changed = instance.tighten_bounds_simultaneously_once()

assert changed == {x.id: Bound(0, 2)}
assert x.bound == Bound(0, 2)
assert y.bound == Bound(0, 10)
```

$x$の上限は$2$になりましたが、$y$の上限はまだ$10$です。各制約は**呼出し開始時のbound**を参照し、得られた更新を最後にまとめて適用します。そのため、この呼出しで得た$x$の新しい上限は、同じ呼出し内の$y\leq x$には使われません。

## 3. 必要なら、更新後のboundでもう一回実行する

もう一回呼ぶと、今度は$x$の上限$2$が使われ、$y$の上限も$2$になります。

```{code-cell} ipython3
changed_again = instance.tighten_bounds_simultaneously_once()

assert changed_again == {y.id: Bound(0, 2)}
assert y.bound == Bound(0, 2)
```

ここまでで、各呼出しが何を変更したかを確認できました。この例では二回で更新が伝わりますが、一般に何回で十分かを、この操作が決めるわけではありません。

## 対象と、更新がない場合の読み方

現在課されている通常制約だけを使います。対象は一次式で、既定では変数項が32個以下の式です。二次式・特殊制約などは直接扱わず、数学的に一次式でもFunctionが保持する表現によっては対象外になります。

更新対象はContinuous・Integer・Binaryの変数で、固定済みの変数や他の変数から値を計算する変数は除きます。ほかの種類の変数や、無限の範囲・数値条件により、更新候補を得られない場合もあります。

`atol`は絶対許容誤差で、省略するとSDKの既定値を使います。更新の判定や整数の丸めにも関わります。また、浮動小数点の丸めや計算順序により、数値境界付近の候補の判定が変わる場合があります。同じ`atol`で評価しても、境界付近の判定が必ず保存されるわけではありません。数値条件は{meth}`~ommx.Instance.tighten_bounds_simultaneously_once_using_constraints`を確認してください。

戻り値が空の辞書`{}`でも、分かるのは**この一回の操作ではboundが更新されなかった**ことだけです。別の操作でもう絞れないことや、実行可能な候補が存在することの証明ではありません。この操作は、すべての実行不可能性を検出するものでもありません。

制約の選択、項数上限、許容誤差、数値境界、失敗時の契約は{meth}`~ommx.Instance.tighten_bounds_simultaneously_once_using_constraints`で確認できます。ほかの変換との保証の違いは[問題を変換して解法へ渡す](../user_guide/transform_and_solve.md)を参照してください。
