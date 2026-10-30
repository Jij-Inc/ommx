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

# InstanceClassとAdapterの入力条件

{class}`~ommx.InstanceClass` は、指定した表現の条件を満たす {class}`~ommx.Instance` の集合です。例えば「Binary変数と線形等式だけを使う最大化問題」を表せます。**いまのInstanceがその条件を満たすかを調べるもので、Instanceを変換するものではありません。**

InstanceClassは、表現の検査や変換先の指定に使います。Adapterの`INPUT_CLASS`は、その用途の一つです。このページではclassの条件と判定方法を説明し、後半でAdapterへの適用を扱います。

## InstanceClass が表す集合

InstanceClassは、複数の {class}`~ommx.InstanceClassClause` の有限合併、つまりいずれか1つのclauseを満たすInstanceの集合です。1つのclauseの中では、次の条件をすべて同時に満たす必要があります。

- 使用中の決定変数の種類（kind）
- 目的関数の多項式表現と次数
- 通常制約の関係（等式・不等式）、多項式表現、次数
- Indicator 制約本体の関係、多項式表現、次数
- 現在の問題で課す（activeな）OneHot制約、SOS1制約を許すか
- 最大化・最小化の向き（sense）

例えば「連続変数だけを使う二次計画」と「整数変数を使える線形計画」を別々の clause で宣言しても、その条件を clause 間で組み合わせた混合整数二次計画が自動的に含まれるわけではありません。

## Membership を調べる

Instanceがclassの条件を満たし、その集合に含まれることをmembershipと呼びます。{meth}`~ommx.InstanceClass.contains`はその判定を`bool`で返します。含まれない理由も必要なら、{meth}`~ommx.InstanceClass.check_membership`がclauseごとの{class}`~ommx.InstanceClassMembershipReport`を返します。どちらも副作用を持ちません。

{ref}`SDKの導入手順 <install-python-sdk>`で用意した環境を使います。次はBinary変数、線形目的関数、通常の線形等式を許すclassです。OneHotなどの特殊制約は許しません。

```{code-cell} ipython3
from ommx import (
    Equality, InstanceClass, InstanceClassClause, Kind,
    PolynomialRequirement, Sense,
)

linear = PolynomialRequirement.at_most(1)
target = InstanceClass([
    InstanceClassClause(
        label="binary-linear-equality",
        allowed_variable_kinds={Kind.Binary},
        objective_polynomial_requirement=linear,
        regular_constraint_polynomial_requirements={Equality.EqualToZero: linear},
        allowed_senses={Sense.Maximize},
    )
])
```

OneHotは「ちょうど一つを選ぶ」という条件です。数学的には線形等式でも表せますが、OneHotという専用の特殊制約として保持している間は、通常の等式だけを許すこのclassには入りません。membershipを調べてもOneHotは変換されません。

```{code-cell} ipython3
from ommx import Instance, OneHotConstraint, OneHotConstraintID

instance = Instance.maximize()
xs = [instance.new_binary("x", subscripts=[i]) for i in range(2)]
instance.objective = 3 * xs[0] + 2 * xs[1]
instance.add_one_hot_constraint(OneHotConstraint(variables=xs))

assert not target.contains(instance)

report = target.check_membership(instance)
assert not report.is_member
assert set(instance.constraints) == {OneHotConstraintID(0)}
print(report)
```

`check_membership()` の mismatch の型や属性は、{class}`~ommx.InstanceClassMismatch` と {class}`~ommx.InstanceClassClauseReport` の API Reference を参照してください。

このInstanceを通常の等式へ変換する操作は、[Instanceを変換するHow-to](../how_to/transform_instance_without_solver.md)で実行できます。変換先のclassと、使ってよい変換を指定するPolicyの関係は[Instance PreparationとPreparationPolicy](./preparation_policy.md)で説明します。

## 数学的な条件と表現形式は別

`Function`の表現形式も判定対象です。数学的に同じ式でも、保持する表現が異なるとclassへの適合結果が変わる場合があります。整数冪と多項式展開の具体例は、{meth}`~ommx.Function.powi`のAPI Referenceを参照してください。

Membershipは要求した表現への適合であり、変換前後の数学的な等価性や最適性を証明するものではありません。それらは、選んだ変換操作とその適用条件によって決まります。

## Classの例：QUBO

QUBO（Quadratic Unconstrained Binary Optimization）は、Binary変数を使い、activeな制約を持たず、二次以下の多項式を最小化する問題です。{meth}`~ommx.InstanceClass.qubo` はこの表現条件を持つclassを返します。線形や定数の目的関数も含まれます。

```{code-cell} ipython3
qubo_instance = Instance.minimize()
a = qubo_instance.new_binary("a")
b = qubo_instance.new_binary("b")
qubo_instance.objective = a + 2 * a * b + 3

qubo_class = InstanceClass.qubo()
assert qubo_class.contains(qubo_instance)

# classに入ったInstanceのactive objectiveを係数と定数項にする。
coefficients, offset = qubo_instance.as_qubo_format()
assert coefficients == {(a.id, a.id): 1.0, (a.id, b.id): 2.0}
assert offset == 3.0
```

QUBOはclassの一例で、OMMX Instanceとは別の基礎モデルではありません。また、Adapter名とclassは一対一ではありません。例えばOpenJij SA Adapterの直接入力は、二次に限らないBinary多項式を許します。利用時は名前からQUBOだと仮定せず、そのAdapterの`INPUT_CLASS`を使います。

**classへの変換と、Backend用の係数への整形も別です。** `as_qubo_format()`はPreparationをせず、activeな目的関数の係数と定数項を取り出します。値の復元規則やremovedな制約を含むInstance全体の代わりになるデータではありません。整形の正確な条件は {meth}`~ommx.Instance.as_qubo_format`、Preparationも行う便利な経路は[PreparationPolicy](./preparation_policy.md)を参照してください。

## 詳細：判定対象はactiveな数学的内容

Membership は、目的関数と active な4種類の制約族（通常制約、Indicator、OneHot、SOS1）で実際に使われる決定変数から判定されます。{attr}`~ommx.Instance.used_decision_variables` に含まれない次の変数は、variable kind の判定対象になりません。

- fixed、dependent、irrelevant な変数
- removed constraint だけが参照する変数
- named function だけが参照する変数
- `output_objective` だけが参照する変数

Instanceが保持する変数と、現在の問題で使う変数の区別は{attr}`~ommx.Instance.used_decision_variables`を参照してください。

同様に、removed constraintはrelation、次数、特殊制約の有無の判定対象ではありません。特殊制約を通常制約へloweringしたInstanceは、変換元をremovedとして保持したまま、特殊制約を許さないclassに入れます。activeとremovedの意味は[候補を評価して結果を判断する](./evaluate_and_interpret.md)を参照してください。

## Adapterへの適用：INPUT_CLASSとapplicability

各OMMX Adapterは、Preparationなしで直接受け取れるInstanceの集合を、必須の`INPUT_CLASS`として宣言します。`INPUT_CLASS`の型もInstanceClassです。これは「変換すれば扱える問題」まで含めたcapabilityではなく、厳格な実行経路へ渡すInstanceそのものの条件です。

`INPUT_CLASS` membershipがAdapter applicabilityの完全な条件です。

{meth}`~ommx.adapter.SolverAdapter.check_applicability`は、`INPUT_CLASS.check_membership()`の構造化されたreportを返します。{meth}`~ommx.adapter.SolverAdapter.require_applicable`は同じmembershipを検査し、non-memberなら{class}`~ommx.adapter.AdapterNotApplicableError`を送出します。

Membershipを満たしても、converterやBackendのすべての処理が成功するとは限りません。例えばBackendの実装上の上限や数値的な条件により、solver inputの構築または実行が失敗することがあります。これはconversionまたはBackendのerrorであり、追加のapplicability条件ではありません。

これらの検査に、次の処理は含まれません。

- lowering、encoding、penalty method などの Preparation
- Backend の実行
- Backend が解やサンプルを返すことの保証

通常の`solve()`と`sample()`は、入力のcopyを作って推奨PolicyでPreparationし、`INPUT_CLASS` membershipへ到達してから厳格な実行経路を呼びます。呼び出し元のInstanceは変更しません。

一方、`solve_without_preparation()`、`sample_without_preparation()`、Adapter constructorはPreparationを行いません。これらへ渡すInstanceは、呼び出し前から`INPUT_CLASS`に入っている必要があります。Policyをapplication固有に編集する場合の一連の操作は[Preparationで変換する制約族を選ぶ](../how_to/customize_preparation.md)を参照してください。
