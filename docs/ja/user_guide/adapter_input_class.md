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

# Adapter の exact input（INPUT_CLASS）

各 OMMX Adapter は、変換せずに直接受け取れる {class}`~ommx.Instance` の集合を `INPUT_CLASS` として宣言します。`INPUT_CLASS` は「変換すれば扱える問題」まで含めた capability ではなく、**Adapter に今から渡す値そのもの**についての exact な条件です。

```{important}
`INPUT_CLASS` の判定は Instance を変換しません。判定した Instance と Preparation 後の Instance は別の入力なので、それぞれについて membership を確認します。
```

## InstanceClass が表す集合

`INPUT_CLASS` の型である {class}`~ommx.InstanceClass` は、複数の {class}`~ommx.InstanceClassClause` の有限合併、つまりいずれか1つの clause を満たす Instance の集合です。1つの clause の中では、次の条件をすべて同時に満たす必要があります。

- 使用中の決定変数の kind
- 目的関数の次数
- 通常制約の relation と次数
- Indicator 制約本体の relation と次数
- active な OneHot 制約、SOS1 制約を許すか
- 最適化 sense

例えば「連続変数だけを使う二次計画」と「整数変数を使える線形計画」を別々の clause で宣言しても、その条件を clause 間で組み合わせた混合整数二次計画が自動的に含まれるわけではありません。

## Membership を調べる

{meth}`~ommx.InstanceClass.contains` は membership を `bool` で返します。含まれない理由も必要なら、{meth}`~ommx.InstanceClass.check_membership` が clause ごとの {class}`~ommx.InstanceClassMembershipReport` を返します。どちらも副作用を持ちません。

次の Instance には active な OneHot 制約があるため、OneHot を exact input として受け取らない PySCIPOpt Adapter の `INPUT_CLASS` には入りません。

```{code-cell} ipython3
from ommx import DecisionVariable, Instance, OneHotConstraint
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

xs = [DecisionVariable.binary(i) for i in range(3)]
instance = Instance.from_components(
    decision_variables=xs,
    objective=sum(xs),
    constraints={},
    one_hot_constraints={0: OneHotConstraint(variables=xs)},
    sense=Instance.MAXIMIZE,
)

input_class = OMMXPySCIPOptAdapter.INPUT_CLASS
assert input_class is not None
assert not input_class.contains(instance)

report = input_class.check_membership(instance)
assert not report.is_member
print(report)
```

`check_membership()` の mismatch の型や属性は、{class}`~ommx.InstanceClassMismatch` と {class}`~ommx.InstanceClassClauseReport` の API Reference を参照してください。

## 判定対象は active な数学的内容

Membership は、目的関数と active な4種類の制約族（通常制約、Indicator、OneHot、SOS1）で実際に使われる決定変数から判定されます。{attr}`~ommx.Instance.used_decision_variables` に含まれない次の変数は、variable kind の判定対象になりません。

- fixed、dependent、irrelevant な変数
- removed constraint だけが参照する変数
- named function だけが参照する変数

同様に、removed constraint は relation、次数、特殊制約の有無の判定対象ではありません。したがって、特殊制約を通常制約へ lowering した Instance は、変換元を removed constraint として保持したまま、特殊制約を許さない `INPUT_CLASS` に入れます。active と removed の意味は [Removed constraints と実行可能性](./removed_constraints.md) を参照してください。

## INPUT_CLASS と Adapter applicability

`INPUT_CLASS` membership は Adapter applicability の最初の条件ですが、両者は同じではありません。

| 判定 | 所有者 | API |
|---|---|---|
| OMMX で表現された exact な入力構造 | `InstanceClass` | `contains()` / `check_membership()` |
| Backend 固有の追加条件を含む適用可能性 | Adapter | `check_applicability()` / `require_applicable()` |

{meth}`~ommx.adapter.SolverAdapter.check_applicability` は、最初に `INPUT_CLASS` membership を調べます。Instance が member の場合だけ、Backend が扱える ID の範囲などの Adapter 固有 precondition を検査します。{meth}`~ommx.adapter.SolverAdapter.require_applicable` は同じ検査を行い、適用不能なら {class}`~ommx.adapter.AdapterNotApplicableError` を送出します。

これらの検査に、次の処理は含まれません。

- lowering、encoding、penalty method などの Preparation
- Backend の実行
- Backend が解やサンプルを返すことの保証

`solve()`、`sample()`、Adapter constructor も入力を暗黙には準備しません。`INPUT_CLASS` に入らない Instance を変換する場合は、[Instance Preparation と PreparationPolicy](./preparation_policy.md) に従って呼び出し側が別の入力を準備します。実際の一連の操作は [Adapter 向けに Instance を準備する](../tutorial/prepare_instance_for_adapter.md) を参照してください。
