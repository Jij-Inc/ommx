(python-sdk-v1-to-v2-migration-guide)=
# Python SDK v1 から v2 へのマイグレーションガイド

v2 は、Protocol Buffers に近い API から Rust + PyO3 実装の API へ移行したバージョンです。v3 ほど大きな破壊的変更ではありませんが、`.raw` への依存を減らし、Python から自然に扱える property / method へ寄せる必要があります。

## `.raw` への直接アクセスを避ける

v2 では `.raw` attribute は deprecated です。可能な限り class 自身の property / method を使ってください。

```python
# v1
solution.raw.evaluated_constraints[0].evaluated_value
instance.raw.decision_variables
sample_set.raw.samples

# v2
solution.get_constraint_value(0)
instance.decision_variables
sample_set.get(sample_id)
```

## import の統一

protobuf module から個別に import していた class は、`ommx.v1` からまとめて import できます。

```python
# v1
from ommx.v1.constraint_pb2 import Constraint, Equality
from ommx.v1.function_pb2 import Function
from ommx.v1.linear_pb2 import Linear
from ommx.v1 import Instance, DecisionVariable

# v2
from ommx.v1 import (
    Instance,
    DecisionVariable,
    Constraint,
    Function,
    Linear,
    Quadratic,
    Polynomial,
    Solution,
    State,
    SampleSet,
)
```

## `DecisionVariable` factory

`DecisionVariable.of_type(...)` は引き続き使えますが、type-specific factory を使うと簡潔です。

```python
DecisionVariable.binary(var_id, name="x")
DecisionVariable.integer(var_id, lower=0, upper=10, name="y")
DecisionVariable.continuous(var_id, lower=0.0, upper=1.0, name="z")
```

## `Function` / expression の作成

直接 protobuf object を組み立てるより、PyO3 API と演算子 overload を使う形へ寄せます。

```python
x = DecisionVariable.binary(0, name="x")
y = DecisionVariable.binary(1, name="y")

objective = x + 2 * y + 1
constraint = x + y <= 1
```

## `Instance` / `Solution` / `SampleSet` accessor

v2 では `Instance`、`Solution`、`SampleSet` が同じような access pattern を持ちます。ID から個別に取りたい場合は専用 method を使ってください。

```python
for var in instance.decision_variables:
    print(var.id, var.name)

for constraint in instance.constraints:
    print(constraint.id, constraint.name)

var = instance.get_decision_variable_by_id(variable_id)
constraint = instance.get_constraint_by_id(constraint_id)
```

## adapter 実装の移行

adapter では `.raw` の内部表現に依存せず、公開 API を使って backend model へ変換します。

```python
# v1
for var_id, var in self.instance.raw.decision_variables.items():
    process_variable(var_id, var)

# v2
for var in self.instance.decision_variables:
    process_variable(var.id, var)
```

`sense` も `.raw.sense` ではなく `instance.sense` を使います。

```python
if instance.sense == Instance.MAXIMIZE:
    ...
elif instance.sense == Instance.MINIMIZE:
    ...
```

## v1 から v2 へのチェックリスト

- [ ] `ommx.v1.*_pb2` からの direct import を `ommx.v1` import に寄せる。
- [ ] `.raw` への直接アクセスを公開 property / method に置き換える。
- [ ] `instance.raw.decision_variables.items()` を `instance.decision_variables` の iteration に置き換える。
- [ ] `instance.raw.constraints.items()` を `instance.constraints` の iteration に置き換える。
- [ ] `instance.raw.sense` を `instance.sense` に置き換える。
- [ ] ID から要素を取る箇所では `get_decision_variable_by_id()` / `get_constraint_by_id()` を使う。
- [ ] `DecisionVariable.binary` / `integer` / `continuous` など、より直接的な factory を使う。
- [ ] `pyright` や `mypy` を併用して、返り値型の変化を確認する。
