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

{class}`~ommx.Function` は、決定変数のIDを参照し、その値から数値を計算する式です。Instanceの目的関数や通常制約の本体を表します。**式の値を計算することと、候補が問題の条件を満たすかを調べることは別です。**

このページは、[Instance](./instance.md)の変数・目的関数・制約を知っている読者向けです。OMMXが受け取った式を読むときにも、自分で式を作るときにも、まずこの評価の違いを押さえます。

## 式の評価と、候補の評価を分ける

次は、式$2x+3$を目的関数に持ち、制約として$x=0$を課す問題です。$x=1$を与えると式の値は5ですが、この候補は制約を満たしません。

```{code-cell} ipython3
from ommx import Function, Instance

instance = Instance.minimize()
x = instance.new_binary("x")
f = Function(2 * x + 3)
instance.objective = f
instance.add_constraint(x == 0)

candidate = {x.id: 1}
assert f.evaluate(candidate) == 5.0
assert not instance.evaluate(candidate).feasible
```

Function単体は変数の名前・種類・上下限を保持せず、IDを参照します。{meth}`~ommx.Function.evaluate`は式の数値を計算しますが、別に課した制約や変数の定義域までは検査しません。問題全体に対する候補の検査には{meth}`~ommx.Instance.evaluate`を使います。評価結果の読み方は[StateからSolutionへの評価](./solution.md)を参照してください。

## 評価できる式が、そのまま求解に使えるとは限らない

Functionで式を構築・評価できることと、Adapterがその表現を直接受け取れることも別です。ソルバーへ渡す問題の表現条件は[InstanceClass](./instance_class.md)、条件に合わせた変換の選択は[PreparationPolicy](./preparation_policy.md)で説明します。

モデラーやファイルから受け取ったInstanceをそのまま解く場合、Functionの構築方法を一通り学ぶ必要はありません。式を調べたり変更したりするときに、必要な操作を確認できます。

## 操作と数値条件はAPI Referenceへ

{class}`FunctionのAPI Reference <ommx.Function>`に、式の作成方法、対応する演算、保持する表現と数値評価の契約をまとめています。用途ごとの入口は次のとおりです。

| 調べたいこと | API Reference |
|---|---|
| 式の作成方法・対応演算・表現形式 | {class}`~ommx.Function` |
| 値の計算・許容誤差・値を計算できない入力 | {meth}`~ommx.Function.evaluate` |
| 一部の変数に値を代入した式を作る | {meth}`~ommx.Function.partial_evaluate` |
| 次数・係数を調べる、式を比較する | {meth}`~ommx.Function.degree`、{attr}`~ommx.Function.terms`、{meth}`~ommx.Function.almost_equal` |
| 変数の範囲から式の値の範囲を見積もる | {meth}`~ommx.Function.evaluate_bound` |
