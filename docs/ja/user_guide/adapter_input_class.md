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

各 OMMX Adapter は、変換せずに直接受け取れる {class}`~ommx.Instance` の集合を、必須の `INPUT_CLASS` として宣言します。`INPUT_CLASS` は「変換すれば扱える問題」まで含めた capability ではなく、**Adapter の厳格な実行経路へ今から渡す値そのもの**についての exact な条件です。

```{important}
`INPUT_CLASS`の判定はInstanceを変換しません。easy APIの`solve()`と`sample()`は先にcopyをPreparationするため、class外のsource Instanceも受け取れる場合があります。preparation-free APIへ渡す値は、呼び出し前からmemberでなければなりません。
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
- `output_objective` だけが参照する変数

同様に、removed constraint は relation、次数、特殊制約の有無の判定対象ではありません。したがって、特殊制約を通常制約へ lowering した Instance は、変換元を removed constraint として保持したまま、特殊制約を許さない `INPUT_CLASS` に入れます。active と removed の意味は [Removed constraints と実行可能性](./removed_constraints.md) を参照してください。

## INPUT_CLASS と Adapter applicability

`INPUT_CLASS` membershipがAdapter applicabilityの完全な条件です。

{meth}`~ommx.adapter.SolverAdapter.check_applicability`は、`INPUT_CLASS.check_membership()`の構造化されたreportを返します。{meth}`~ommx.adapter.SolverAdapter.require_applicable`は同じmembershipを検査し、non-memberなら{class}`~ommx.adapter.AdapterNotApplicableError`を送出します。

Membershipを満たしても、converterやBackendのすべての処理が成功するとは限りません。例えばBackendの実装上の上限や数値的な条件により、solver inputの構築または実行が失敗することがあります。これはconversionまたはBackendのerrorであり、追加のapplicability条件ではありません。

これらの検査に、次の処理は含まれません。

- lowering、encoding、penalty method などの Preparation
- Backend の実行
- Backend が解やサンプルを返すことの保証

通常の`solve()`と`sample()`は、入力のcopyを作って推奨PolicyでPreparationし、`INPUT_CLASS` membershipへ到達してから厳格な実行経路を呼びます。呼び出し元のInstanceは変更しません。

一方、`solve_without_preparation()`、`sample_without_preparation()`、Adapter constructorはPreparationを行いません。これらへ渡すInstanceは、呼び出し前から`INPUT_CLASS`に入っている必要があります。Policyをapplication固有に編集する場合の一連の操作は[Adapter向けにInstanceを準備する](../tutorial/prepare_instance_for_adapter.md)を参照してください。
