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

# 変数の役割と値の復元・補完

Instanceが保持する変数のすべてについて、候補の値を毎回渡す必要があるとは限りません。**現在使う変数には値を与え、固定・依存する変数は復元し、無関係な変数を省略した場合だけ既定値で補います。** この違いは、変換後の候補を元の変数で読むときや、使わない変数が結果に現れるときに重要です。

このページは、[Instance](./instance.md)が変数・目的関数・制約をまとめた問題であり、`evaluate()`が候補を評価する操作だと知っている読者向けです。変換が元の最適化問題を保存する条件は、[変換が何を保存するか](./transformation_guarantees.md)で別に扱います。

(retained-and-used-variables)=
## 保持される変数とusedな変数

変数を固定したり、別の変数で表現したりすると、ソルバーが値を選ぶ必要のない変数が生まれます。OMMXは結果を元の変数で読めるよう、その定義も保持します。

```{code-cell} ipython3
from ommx import Instance

instance = Instance.maximize()
x = instance.new_binary("x")
y = instance.new_binary("y")
instance.objective = x + y
instance.add_constraint(x * y == 0)

# partial_evaluateは元のInstanceを変更せず、固定した問題を返す。
fixed = instance.partial_evaluate({x.id: 1})

assert {v.id for v in fixed.decision_variables} == {x.id, y.id}
assert {v.id for v in fixed.used_decision_variables} == {y.id}

fixed_solution = fixed.evaluate({y.id: 0})
assert fixed_solution.state.entries == {x.id: 1.0, y.id: 0.0}
assert fixed_solution.objective == 1
```

`x`の定義は残っていますが、固定した問題で値を選ぶ必要があるのは`y`だけです。`x`の値をもう一度渡さなくても、Instanceが記録した1が使われます。

{attr}`~ommx.Instance.decision_variables` は保持されるすべての変数、{attr}`~ommx.Instance.used_decision_variables` は現在の目的関数とactiveな制約に現れる変数です。activeとは現在課している制約で、通常制約に加えてIndicator・OneHot・SOS1も含みます。Indicatorを制御するBinary変数もusedです。

(instance-variable-roles)=
## 四つの役割

Instance内の変数は、次のいずれかの役割を持ちます。BinaryやIntegerといった値の種類とは別の分類です。

| 役割 | 意味 | 評価に必要な値 |
|---|---|---|
| used | 現在の目的関数やactiveな制約に現れる | 呼び出し側が与える |
| fixed | Instanceに固定値が記録されている | 記録された値を使う |
| dependent | 別の変数の関数として表されている | 記録された式から計算する |
| irrelevant | 上記のどれでもなく、現在の問題では値を選ぶ必要がない | 省略された場合は既定値で補う |

{meth}`~ommx.Instance.decision_variable_role` で一つの変数、{meth}`~ommx.Instance.decision_variable_roles` ですべての変数の役割を確認できます。役割はInstanceに対して決まり、変換によって変わります。

## fixedとdependentの値はInstanceから復元する

fixedには記録された固定値を使います。dependentには、記録された依存式を使います。例えば、整数変数`count`をBinary変数`flag`の2倍で置き換えます。

```{code-cell} ipython3
encoded = Instance.minimize()
count = encoded.new_integer("count", lower=0, upper=2)
flag = encoded.new_binary("flag")
encoded.objective = count + flag

encoded.substitute({count.id: 2 * flag})
assert {v.id for v in encoded.used_decision_variables} == {flag.id}

restored = encoded.evaluate({flag.id: 1})
assert restored.state.entries[count.id] == 2.0
assert restored.objective == 3.0
```

この例で`count=2*flag`から得られる値は、`count`の定義域内です。一方で`count=1`は表せないため、この置換は元の整数問題全体と等価ではありません。また、{meth}`~ommx.Instance.substitute`が、置換する変数の定義域を新しい制約へ自動変換するわけではありません。**値を計算して戻せることだけでは、置換前後の問題が等価だとはいえません。** 独自の置換を選ぶときの判断は[変換の保証](./transformation_guarantees.md)を参照してください。

fixed・dependentの値も入力した場合、それは記録された値・依存式との整合性を確認するための指定であり、復元規則の上書きではありません。丸め、許容誤差、不整合時の扱いは {meth}`~ommx.Instance.populate_state` のAPI Referenceにあります。

## irrelevantの既定値は元の候補を復元した値ではない

目的関数にも制約にも使わない変数も宣言できます。

```{code-cell} ipython3
reporting = Instance.minimize()
chosen = reporting.new_binary("chosen")
unused = reporting.new_integer("unused", lower=2, upper=5)
reporting.objective = chosen

completed = reporting.evaluate({chosen.id: 0})
assert completed.state.entries[unused.id] == 2.0

# 手元の候補に値があるなら、省略せずに渡す。
explicit = reporting.evaluate({chosen.id: 0, unused.id: 4})
assert explicit.state.entries[unused.id] == 4.0
```

`unused`の値は現在の目的値や制約には影響しませんが、一意には決まりません。省略した場合の2は、ソルバーが選んだ値でも、元の候補から復元した値でもありません。

変数の種類やboundによって、補う既定値は異なります。正確な選択規則は{meth}`~ommx.Instance.populate_state`のAPI Referenceを参照してください。ここで重要なのは、補われた値を元の候補の値とみなさず、特定の候補を調べるときには値を明示することです。

## 復元・補完した後、どの情報を評価するか

{meth}`~ommx.Instance.populate_state` は値の復元・補完を行い、{meth}`~ommx.Instance.evaluate` はその後に目的関数・制約を評価します。usedな変数の不足を既定値で埋めることはありません。復元・補完できたことも、それだけでは候補がすべての制約を満たす保証にはなりません。

名前付き関数、removedな制約（現在は課さず、結果確認用に保持する制約）、出力目的関数（現在の目的関数とは別に評価用に保持する目的関数）だけに現れる変数は、それだけではusedになりません。それらにirrelevantな変数が現れ、値を省略した場合は既定値で評価されます。特定の候補について確認したいなら、その候補の値を渡してください。

評価結果を読む通常の入口は[StateからSolutionへの評価](./solution.md)、保持された制約や目的関数を読む場合は[変換後の問題とRemoved constraints](./removed_constraints.md)です。
