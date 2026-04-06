---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: .venv
  language: python
  name: python3
---

```{warning}
このドキュメントはOMMX Python SDK 1.9.0のリリース時のものであり、Python SDK 2.0.0以降では動作しません。
```

+++

# OMMX Python SDK 1.9.0

+++

このリリースでは、`ommx.v1.Instance` からQUBOへの変換機能が大幅に強化され、**不等式制約**と**整数変数**のサポートが追加されました。また、QUBO変換プロセスを簡単にするための新しいDriver API `to_qubo` が導入されました。

+++

## ✨ 新機能

+++

### 整数変数のlog-encoding ([#363](https://github.com/Jij-Inc/ommx/pull/363), [#260](https://github.com/Jij-Inc/ommx/pull/260))

整数変数 $x$ を、バイナリ変数 $b_i$ を用いて次のようにエンコードします。

$$
x = \sum_{i=0}^{m-2} 2^i b_i + (u - l - 2^{m-1} + 1) b_{m-1} + l
$$

これにより、整数変数を直接扱えないQUBOソルバーでも整数を使った最適化問題を扱うことができるようになります。

またQUBOソルバーなどはバイナリ変数だけを返してくるはずですが、`Instance.evaluate` や `evaluate_samples` が自動的にこの整数変数を復元して `ommx.v1.Solution` や `ommx.v1.SampleSet` として返します。

```{code-cell} ipython3
# 整数変数のログエンコーディング例
from ommx.v1 import Instance, DecisionVariable

# 3つの整数変数を持つ問題を定義
x = [
    DecisionVariable.integer(i, lower=0, upper=3, name="x", subscripts=[i])
    for i in range(3)
]
instance = Instance.from_components(
    decision_variables=x,
    objective=sum(x),
    constraints=[],
    sense=Instance.MAXIMIZE,
)
print("変換前の目的関数:", instance.objective)

# x0とx2のみをログエンコード
instance.log_encode({0, 2})
print("\n変換後の目的関数:", instance.objective)

# 生成されたバイナリ変数を確認
print("\n決定変数一覧:")
print(instance.decision_variables[["kind", "lower", "upper", "name", "subscripts"]])

# バイナリ変数から整数変数の復元
print("\n整数変数の復元:")
solution = instance.evaluate({
    1: 2,          # x1 = 2
    3: 0, 4: 1,    # x0 = x3 + 2*x4 = 0 + 2*1 = 2
    5: 0, 6: 0     # x2 = x5 + 2*x6 = 0 + 2*0 = 0
})
print(solution.extract_decision_variables("x"))
```

### 不等式制約のサポート

不等式制約 $ f(x) \leq 0 $ を含む問題をQUBOに変換するために、以下の二つの方法が実装されました。

+++

#### 整数スラック変数による等式制約化 ([#366](https://github.com/Jij-Inc/ommx/pull/366))

この方法では、まず不等式制約の係数を有理数で表現し、適切な有理数 $a > 0$ を全体にかけることで $a f(x)$ の係数を全て整数に変換します。その後、整数のスラック変数 $s$ を導入することで、不等式制約を等式制約 $ f(x) + s/a = 0$ に変換します。変換された等式制約は、既存の手法を用いてペナルティ項としてQUBOの目的関数に追加されます。

この方法は常に適用できますが、多項式の係数の間に割り切れないものがある場合は `a` が非常に大きくなり、合わせて `s` の範囲も広がるため実用的ではなくなる可能性があります。なので `s` の範囲の上限をユーザーが入力するAPIになっています。後述する `to_qubo` ではこの方法がデフォルトで適用されます。

```{code-cell} ipython3
# 不等式制約の等式制約への変換例
from ommx.v1 import Instance, DecisionVariable

# 不等式制約 x0 + 2*x1 <= 5 を持つ問題
x = [
    DecisionVariable.integer(i, lower=0, upper=3, name="x", subscripts=[i])
    for i in range(3)
]
instance = Instance.from_components(
    decision_variables=x,
    objective=sum(x),
    constraints=[
        (x[0] + 2*x[1] <= 5).set_id(0)   # 制約IDを設定
    ],
    sense=Instance.MAXIMIZE,
)
print("変換前の制約:", instance.get_constraints()[0])

# 不等式制約を等式制約に変換
instance.convert_inequality_to_equality_with_integer_slack(
    constraint_id=0,
    max_integer_range=32
)
print("\n変換後の制約:", instance.get_constraints()[0])

# 追加されたスラック変数を確認
print("\n決定変数一覧:")
print(instance.decision_variables[["kind", "lower", "upper", "name", "subscripts"]])
```

#### 不等式制約のまま整数スラック変数を追加する ([#369](https://github.com/Jij-Inc/ommx/pull/369), [#368](https://github.com/Jij-Inc/ommx/pull/368))

上述の方法が適用できない場合、等式制約にすることを諦めて不等式制約のまま整数slack変数 $s$ を $f(x) + b s \leq 0$ の形で追加し、QUBOにするときはこれを等式制約のように $|f(x) + b s|^2$ の形でペナルティとして追加します。単に $|f(x)|^2$ として追加する場合に比べて、これにより不当に $f(x) = 0$ が優遇されることなくなります。

合わせて `Instance.penalty_method` や `uniform_penalty_method` が不等式を受け取るようになり、等式制約と同じように単に $|f(x)|^2$ として追加するようになりました。

```{code-cell} ipython3
# 不等式制約へのスラック変数追加例
from ommx.v1 import Instance, DecisionVariable

# 不等式制約 x0 + 2*x1 <= 4 を持つ問題
x = [
    DecisionVariable.integer(i, lower=0, upper=3, name="x", subscripts=[i])
    for i in range(3)
]
instance = Instance.from_components(
    decision_variables=x,
    objective=sum(x),
    constraints=[
        (x[0] + 2*x[1] <= 4).set_id(0)   # 制約IDを設定
    ],
    sense=Instance.MAXIMIZE,
)
print("変換前の制約:", instance.get_constraints()[0])

# 不等式制約にスラック変数を追加
b = instance.add_integer_slack_to_inequality(
    constraint_id=0,
    slack_upper_bound=2
)
print(f"\nスラック変数の係数: {b}")
print("変換後の制約:", instance.get_constraints()[0])

# 追加されたスラック変数を確認
print("\n決定変数一覧:")
print(instance.decision_variables[["kind", "lower", "upper", "name", "subscripts"]])
```

### QUBO変換 Driver API `to_qubo` の追加 ([#370](https://github.com/Jij-Inc/ommx/pull/370))

`ommx.v1.Instance` からQUBOへの変換に必要な一連の操作（整数変数変換、不等式制約変換、ペナルティ項追加など）をまとめて実行する Driver API `to_qubo` が追加されました。これにより、ユーザーは複雑な変換ステップを意識することなく、簡単にQUBOを得ることができます。

`to_qubo` 関数は、内部で以下のステップを適切な順序で実行します:
1. 整数変数を含む制約や目的関数をバイナリ変数表現に変換 (Log Encodingなど)
2. 不等式制約を等式制約に変換 (デフォルト) または Penalty Method 用の形式に変換
3. 等式制約や目的関数をQUBO形式に変換
4. QUBOの解を元の問題の変数にマッピングするための `interpret` 関数を生成

なお `instance.to_qubo` として呼び出した時 `instance` は変更されることに注意してください。

```{code-cell} ipython3
# to_qubo Driver API の使用例
from ommx.v1 import Instance, DecisionVariable

# 整数変数と不等式制約を含む問題
x = [DecisionVariable.integer(i, lower=0, upper=2, name="x", subscripts=[i]) for i in range(2)]
instance = Instance.from_components(
    decision_variables=x,
    objective=sum(x),
    constraints=[(x[0] + 2*x[1] <= 3).set_id(0)],
    sense=Instance.MAXIMIZE,
)

print("元の問題:")
print(f"目的関数: {instance.objective}")
print(f"制約: {instance.get_constraints()[0]}")
print(f"変数: {[f'{v.name}{v.subscripts}' for v in instance.get_decision_variables()]}")

# QUBOに変換
qubo, offset = instance.to_qubo()

print("\nQUBO変換後:")
print(f"オフセット: {offset}")
print(f"QUBOの項数: {len(qubo)}")

# 項数が多いため一部のみ表示
print("\nQUBOの一部の項:")
items = list(qubo.items())[:5]
for (i, j), coeff in items:
    print(f"Q[{i},{j}] = {coeff}")

# 変換後の変数を確認
print("\n変換後の変数:")
print(instance.decision_variables[["kind", "name", "subscripts"]])

# 制約が削除されたことを確認
print("\n変換後の制約:")
print(f"残った制約: {instance.get_constraints()}")
print(f"削除された制約: {instance.get_removed_constraints()}")
```


## 🐛 バグ修正

## 🛠️ その他の変更・改善

## 💬 フィードバック
これらの新機能により、ommxはより広範な最適化問題をQUBO形式に変換し、様々なQUBOソルバーで解くための強力なツールとなります。ぜひ `ommx` 1.9.0 をお試しください！

フィードバックやバグ報告は、[GitHub Issues](https://github.com/Jij-Inc/ommx/issues) までお寄せください。
