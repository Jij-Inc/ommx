(python-sdk-v2-to-v3-migration-guide)=
# Python SDK v2 から v3 へのマイグレーションガイド

この節の基準バージョンは **v2.5.1**（tag `python-2.5.1`）です。それ以前の 2.x から移行する場合は、[v1 から v2 へのマイグレーションガイド](python_sdk_v1_to_v2.md) も確認してください。

## 概要

v3 では、v2 で始まった PyO3 移行が完了しました。`ommx.v1` の各クラスは、protobuf message を包む Python wrapper ではなく、`ommx._ommx_rust` から re-export される Rust 実装の型になっています。

移行で特に注意する点は次の通りです。

1. `ommx.v1.*_pb2` は削除されました。ユーザーコードは `ommx.v1` から直接 import します。
2. `Constraint` は `id` を持たなくなりました。制約IDは `Instance.from_components(..., constraints={id: constraint})` に渡す `dict` の key が所有します。
3. `Instance` / `ParametricInstance` / `Solution` の制約コレクションは `list[T]` ではなく `dict[int, T]` です。`decision_variables` は `list` のままです。
4. `.raw`、`.from_raw()`、`.from_protobuf()`、`.to_protobuf()` など、protobuf 層を露出する bridge API は削除されました。
5. `*_df` accessor は property ではなく method です。`instance.constraints_df()` のように呼び出してください。
6. `instance.constraints[id]` や `instance.decision_variables` は、snapshot ではなく書き込みが host に反映される `AttachedX` handle を返します。

## 1. import の変更

### 1.1 protobuf submodule は削除

`ommx.v1.*_pb2` module と `ommx.v1.annotation` は削除されました。クラスは `ommx.v1` から直接 import します。

```python
# v2.5.1
from ommx.v1.constraint_pb2 import Constraint, Equality
from ommx.v1.function_pb2 import Function
from ommx.v1.linear_pb2 import Linear
from ommx.v1.solution_pb2 import State

# v3
from ommx.v1 import Constraint, Equality, Function, Linear, State
```

`.from_protobuf()` / `.to_protobuf()` は、protobuf object と一緒に削除されています。`Instance` / `Solution` / `SampleSet` など全体をシリアライズする場合は `from_bytes` / `to_bytes` を使います。

### 1.2 constraint hint helper は first-class constraint type へ

`ConstraintHints`、`OneHot`、`Sos1`、`Parameters` wrapper は `ommx.v1` から export されなくなりました。

```python
# v2.5.1
from ommx.v1 import OneHot, Sos1, ConstraintHints, Parameters

# v3
from ommx.v1 import OneHotConstraint, Sos1Constraint, IndicatorConstraint

# Parameters wrapper の代わりに plain dict を渡します
parametric_instance.with_parameters({parameter_id: 1.0})
```

## 2. `.raw` と bridge method の削除

v3 の各型は直接 Rust 実装を持つため、別の underlying object はありません。`.raw`、`from_raw`、`from_protobuf`、`to_protobuf` を使っていた箇所は、公開 property / method に置き換えます。

```python
# v2.5.1
linear.raw.linear_terms
instance.raw.sense
solution.raw.optimality = Optimality.Optimal
Constraint.from_protobuf(pb_constraint)
dv.to_protobuf()

# v3
linear.linear_terms
instance.sense
solution.optimality = Optimality.Optimal
instance.to_bytes()
```

`Constraint` や `Function` などの要素単体は、ID や host context を持てないため、原則として単体で bytes round-trip しません。`Instance` / `Solution` / `SampleSet` のような所有者単位でシリアライズしてください。

## 3. 制約IDは `Constraint` ではなく host 側が所有

### 3.1 `id` / `set_id()` / `id=` は削除

`Constraint`、`IndicatorConstraint`、`OneHotConstraint`、`Sos1Constraint`、`RemovedConstraint`、`EvaluatedConstraint`、`SampledConstraint` は、オブジェクト自身に ID を持ちません。ID は `Instance.from_components` に渡す辞書の key で決まります。

```python
# v2.5.1
c = Constraint(
    function=x + y,
    equality=Constraint.EQUAL_TO_ZERO,
    id=5,
    name="cap",
)
c.set_id(6)

# v3
c = Constraint(function=x + y, equality=Constraint.EQUAL_TO_ZERO, name="cap")

instance = Instance.from_components(
    sense=Instance.MINIMIZE,
    objective=objective,
    decision_variables=decision_variables,
    constraints={5: c},
)
```

### 3.2 比較演算子は detached な `Constraint` を返す

`==`、`<=`、`>=` は引き続き `Constraint` を作りますが、その時点では ID を持ちません。

```python
# v2.5.1
c = (x + y <= 5).set_id(0)
Instance.from_components(..., constraints=[c], ...)

# v3
c = x + y <= 5
Instance.from_components(..., constraints={0: c}, ...)
```

### 3.3 グローバルIDカウンタは削除

`next_constraint_id()`、`set_constraint_id_counter(...)`、`get_constraint_id_counter()` などの module-level helper は削除されました。新しい制約IDが必要な場合は、所有者である `Instance` の `instance.next_constraint_id()` を使います。

## 4. container type の変更

### 4.1 `Instance.from_components(constraints=...)` は `dict[int, Constraint]`

制約系の引数はすべて ID を key にした `dict` です。`decision_variables` は `Sequence[DecisionVariable]` のままです。

```python
# v2.5.1
Instance.from_components(
    sense=Instance.MINIMIZE,
    objective=obj,
    decision_variables=[x0, x1],
    constraints=[c0, c1],
    constraint_hints=ConstraintHints(...),
)

# v3
Instance.from_components(
    sense=Instance.MINIMIZE,
    objective=obj,
    decision_variables=[x0, x1],
    constraints={0: c0, 1: c1},
    indicator_constraints={10: ic},
    one_hot_constraints={20: oh},
    sos1_constraints={30: sc},
)
```

`ParametricInstance.from_components` も同じ shape を取ります。

(42-constraint-accessors-on-instance--parametricinstance--solution-return-dicts)=
### 4.2 `Instance` / `ParametricInstance` / `Solution` の制約 accessor は dict を返す

```python
# v2.5.1
for c in instance.constraints:
    print(c.id, c.function)

# v3
for cid, c in instance.constraints.items():
    print(cid, c.function)
```

`Instance` / `ParametricInstance` の制約 dict は、v3 final では `AttachedX` handle を返します。`Solution.constraints` は評価結果の snapshot なので `EvaluatedConstraint` のままです。`SampleSet.constraints` / `.decision_variables` / `.named_functions` は `list` のままです。

## 5. rename と signature 変更

主な rename / signature 変更は次の通りです。

| v2.5.1 | v3 |
|---|---|
| `instance.write_mps(path)` | `instance.save_mps(path)` |
| `instance.used_decision_variable_ids()` | `instance.required_ids()` |
| `func.used_decision_variable_ids()` | `func.required_ids()` |
| `Parameter.new(id=..., ...)` | `Parameter(id, ...)` |
| `Parameters(entries={...})` | plain `dict[int, float]` |
| `Linear(terms=[Linear.Term(...)])` | `Linear(terms={id: coeff})` |

```python
# v2.5.1
instance.write_mps("out.mps.gz")
p = Parameter.new(id=3, name="w")
pi.with_parameters(Parameters(entries={p.id: 1.0}))

# v3
instance.save_mps("out.mps.gz")
p = Parameter(3, name="w")
pi.with_parameters({p.id: 1.0})
```

## 6. return type の変更

`Constraint.name` / `Constraint.description` などは、未設定時に空文字列ではなく `None` を返します。

```python
name = constraint.name
if name is not None:
    print(name)
```

`Linear.terms` / `Quadratic.terms` / `Polynomial.terms` は property ではなく method です。

```python
linear.terms()
quadratic.terms()
polynomial.terms()
```

`SampleSet.sample_ids` は list property ではなく set を返す method になりました。list が必要な場合は `sample_ids_list` を使います。

```python
ids: set[int] = sample_set.sample_ids()
ids_list: list[int] = sample_set.sample_ids_list
```

`evaluate` / `partial_evaluate` は missing state などの入力エラーで `RuntimeError` ではなく `ValueError` を投げます。

## 7. 削除された helper

次の helper は削除または置き換えられました。

- `Linear.from_object(x)` - `Linear.single_term(...)`、`Linear.constant(...)`、または arithmetic operator を使います。
- `Linear.equals_to(other)` - `linear.almost_equal(other, atol=...)` を使います。
- `instance.constraint_hints` - `one_hot_constraints` / `sos1_constraints` / `indicator_constraints` に分かれました。
- `ArtifactArchive` / `ArtifactDir` 系 - `Artifact` / `ArtifactDraft` に統合されました。

## 8. DataFrame accessor

`*_df` は property ではなく method です。

```python
# v2.5.1
df = instance.constraints_df

# v3
df = instance.constraints_df()
```

kind 別や removed / active 別の DataFrame accessor は、`constraints_df(kind=..., removed=...)` に統合されています。

```python
instance.constraints_df(kind="normal")
instance.constraints_df(kind="one_hot")
instance.constraints_df(kind="sos1", removed=True)
solution.constraints_df(kind="indicator")
```

## 9. metadata と annotation

`Instance` / `Solution` / `SampleSet` は Python dataclass ではありません。metadata は dedicated property や method で扱います。

```python
instance.title = "portfolio"
instance.add_user_annotation("owner", "analytics")
instance.replace_annotations({"team": "optimization"})
```

`annotations` property は read-only projection です。直接 `obj.annotations[...] = ...` のようには変更できません。

## 10. `AttachedX` handle と snapshot

`instance.constraints[id]` や `instance.decision_variables` は、host に書き戻す `AttachedX` handle を返します。

```python
c = instance.constraints[5]
c.set_name("balance")
assert instance.constraints[5].name == "balance"
```

detached snapshot が必要な場合は `detach()` を使います。

```python
snapshot = instance.constraints[5].detach()
```

## 11. named function ID

Named function 系も table-owned ID model に移行しています。`NamedFunction`、`EvaluatedNamedFunction`、`SampledNamedFunction` の row object 自身ではなく、host 側の table key が ID の source of truth です。

Python API ではユーザーが移行しやすいように `.id` を参照できる箇所がありますが、実装上の所有者は host table です。新しいコードでは、collection を走査するときに key と value を分けて扱うことを優先してください。

## 12. 要素単体の bytes round-trip

`Function` / `Linear` / `Quadratic` / `Polynomial` / `Parameter` / `NamedFunction` family / `DecisionVariable` family の要素単体 `to_bytes()` / `from_bytes()` は削除されています。

要素を永続化したい場合は、所有者である `Instance` / `Solution` / `SampleSet` へ入れてから全体を round-trip してください。

```python
instance_blob = instance.to_bytes()
restored = Instance.from_bytes(instance_blob)
```

(13-artifact-api-archive-becomes-an-exchange-format)=
## 13. Artifact API: archive becomes an exchange format

v3 では Artifact API が SQLite Local Registry を中心に整理され、`.ommx` file は registry から明示的に export / import する exchange format になりました。

### 13.1 `ArtifactBuilder.new_archive` / `new_archive_unnamed` は削除

`.ommx` file を作る処理は、`ArtifactDraft` を commit した後の `Artifact.save(path)` に分離されました。

```python
# v2
builder = ArtifactBuilder.new_archive("my_instance.ommx", "ghcr.io/jij-inc/ommx/demo:v1")
builder.add_instance(instance)
artifact = builder.build()

# v3
draft = ArtifactDraft.new("ghcr.io/jij-inc/ommx/demo:v1")
draft.add_instance(instance)
artifact = draft.commit()
artifact.save("my_instance.ommx")
```

### 13.2 anonymous archive は `new_anonymous`

`new_archive_unnamed(path)` は `ArtifactDraft.new_anonymous()` に置き換わりました。v3 の anonymous Artifact も Local Registry 内では image name を持つため、`artifact.image_name is None` を前提にしたコードは見直してください。

```python
draft = ArtifactDraft.new_anonymous()
draft.add_instance(instance)
artifact = draft.commit()
artifact.save("my_instance.ommx")
```

anonymous Artifact を多用する workflow では、定期的に `ommx artifact prune-anonymous` を実行してください。

### 13.3 `Artifact.load_archive` は `import_archive` / `inspect_archive` に分割

v2 の `Artifact.load_archive(file)` は、v3 では目的別に分かれました。

- `Artifact.import_archive(file)` - archive を Local Registry に import し、全 layer を読める `Artifact` handle を返します。
- `Artifact.inspect_archive(file)` - registry に書き込まず、manifest / layer descriptor だけを読む read-only path です。

```python
# archive を使う
artifact = Artifact.import_archive("my_instance.ommx")

# 中身を確認するだけ
manifest = Artifact.inspect_archive("my_instance.ommx")
```

### 13.4 CLI flow

archive file を直接 push する flow は廃止されました。いったん load / import して Local Registry に入れてから、image name で push します。

```bash
ommx load my_instance.ommx
ommx push ghcr.io/jij-inc/ommx/demo:v1
```

### 13.5 Artifact migration checklist

- [ ] `ArtifactBuilder.new_archive(path, image_name).build()` を `ArtifactDraft.new(image_name).commit()` + `artifact.save(path)` に置き換える。
- [ ] `ArtifactBuilder.new_archive_unnamed(path).build()` を `ArtifactDraft.new_anonymous().commit()` + `artifact.save(path)` に置き換える。
- [ ] `Artifact.load_archive(file)` を、用途に応じて `Artifact.import_archive(file)` または `Artifact.inspect_archive(file)` に置き換える。
- [ ] `ommx push <archive-file>` を `ommx load <file>` + `ommx push <image_name>` に置き換える。
- [ ] anonymous Artifact を大量に作る場合は `ommx artifact prune-anonymous` を運用に入れる。

## v2 から v3 へのチェックリスト

- [ ] `ommx.v1.*_pb2` import を `ommx.v1` からの direct import に置き換える。
- [ ] `.raw` / `from_raw` / `from_protobuf` / `to_protobuf` を削除する。
- [ ] `Constraint.id` / `set_id()` / `id=` を削除し、host dict の key で ID を渡す。
- [ ] `constraints=[...]` を `constraints={id: constraint}` に置き換える。
- [ ] `constraint_hints` を `one_hot_constraints` / `sos1_constraints` / `indicator_constraints` に置き換える。
- [ ] `*_df` accessor に `()` を付ける。
- [ ] `RuntimeError` を捕捉していた `evaluate` / `partial_evaluate` 周辺を `ValueError` に変える。
- [ ] element-level `to_bytes()` / `from_bytes()` を、所有者全体の round-trip に置き換える。
- [ ] Artifact archive API を `ArtifactDraft` / `Artifact.save` / `Artifact.import_archive` / `Artifact.inspect_archive` に移行する。
