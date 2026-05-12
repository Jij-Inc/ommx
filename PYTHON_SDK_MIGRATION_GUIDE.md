# Python SDK Migration Guide

This document is a guide for migrating the OMMX Python SDK across major versions.

- [v2 to v3](#python-sdk-v2-to-v3-migration-guide) - Complete PyO3 migration for mathematical types
- [v1 to v2](#python-sdk-v1-to-v2-migration-guide) - Protocol Buffer to PyO3 migration

---

# Python SDK v2 to v3 Migration Guide

Baseline for this guide: **v2.5.1** (tag `python-2.5.1`). Upgrades from earlier 2.x releases should consult this guide plus [v1 → v2](#python-sdk-v1-to-v2-migration-guide).

## Overview

v3 completes the PyO3 migration that started in v2: every class in `ommx.v1` is now a direct Rust type re-exported from `ommx._ommx_rust`, not a Python wrapper around a protobuf message. As a side effect, a number of v2-era shims (`.raw`, `.from_raw()`, `.from_protobuf()`, `.to_protobuf()`, counter helpers, `Parameters`, …) were removed, and several APIs adopted cleaner signatures.

Themes you will encounter:

1. Every trace of the protobuf layer is gone — imports from `ommx.v1.*_pb2` must switch to `ommx.v1`, and bridge methods like `.raw`/`from_protobuf`/`to_protobuf` are removed.
2. `Constraint` no longer has an `id` — constraint IDs live only as the keys of the `dict[int, Constraint]` you pass to `Instance.from_components`. All `.id` getters, `set_id()` / `id=` kwargs, and global ID-counter helpers are gone.
3. Container types flipped: every constraint-valued argument and getter on `Instance` / `ParametricInstance` / `Solution` is now `dict[int, T]`, not `list[T]`. `decision_variables` stays a `list`.
4. A handful of renames and small signature changes (`write_mps` → `save_mps`, `Parameters(entries=...)` → plain `dict`, …).
5. Every `*_df` accessor is a method now, with `kind=` / `include=` / `removed=` parameters consolidating the per-kind / active-vs-removed family. Long-format sidecar DataFrames (`constraint_metadata_df`, `constraint_provenance_df`, `variable_parameters_df`, …) are new.
6. `instance.constraints[id]` and `instance.decision_variables` return write-through `AttachedX` handles instead of snapshot wrappers; metadata mutations through them propagate back to the host.

## 1. Import changes

### 1.1 Protobuf submodules are gone (`3.0.0a1`, [#776](https://github.com/Jij-Inc/ommx/pull/776))

Every `ommx.v1.*_pb2` module and `ommx.v1.annotation` is removed. Import classes from `ommx.v1` directly.

**Before (v2.5.1)**:
```python
from ommx.v1.constraint_pb2 import Constraint, Equality
from ommx.v1.function_pb2 import Function
from ommx.v1.linear_pb2 import Linear
from ommx.v1.solution_pb2 import State
```

**After (v3)**:
```python
from ommx.v1 import Constraint, Equality, Function, Linear, State
```

The `.from_protobuf()` / `.to_protobuf()` bridge methods on `Constraint`, `RemovedConstraint`, `DecisionVariable`, etc. are removed along with the protobuf objects they produced. Use `from_bytes` / `to_bytes` for serialisation instead.

### 1.2 Constraint-hint helper types removed (`3.0.0a1`, [#776](https://github.com/Jij-Inc/ommx/pull/776); `3.0.0a2`, [#790](https://github.com/Jij-Inc/ommx/pull/790), [#798](https://github.com/Jij-Inc/ommx/pull/798))

`ConstraintHints`, `OneHot`, `Sos1`, and the `Parameters` wrapper are no longer exported from `ommx.v1`. They are superseded by the first-class constraint types (`OneHotConstraint`, `Sos1Constraint`, `IndicatorConstraint`) and plain `dict[int, float]` for parameter substitution.

**Before (v2.5.1)**:
```python
from ommx.v1 import OneHot, Sos1, ConstraintHints, Parameters
```

**After (v3)**:
```python
from ommx.v1 import OneHotConstraint, Sos1Constraint, IndicatorConstraint
# Parameters is gone — pass a plain dict[int, float] to ParametricInstance.with_parameters
```

## 2. Removal of `.raw` and `from_raw` / `from_protobuf` / `to_protobuf` (`3.0.0a1`, [#770](https://github.com/Jij-Inc/ommx/pull/770), [#771](https://github.com/Jij-Inc/ommx/pull/771), [#774](https://github.com/Jij-Inc/ommx/pull/774), [#775](https://github.com/Jij-Inc/ommx/pull/775))

v2 deprecated `.raw`, v3 removes it. All migrated classes are direct Rust types; there is no separate underlying object.

**Affected classes**: `Linear`, `Quadratic`, `Polynomial`, `Function`, `NamedFunction`, `DecisionVariable`, `Parameter`, `Instance`, `ParametricInstance`, `Solution`, `SampleSet`, `Constraint`, `RemovedConstraint`, `Bound`, `DecisionVariableAnalysis`.

**Before (v2.5.1)**:
```python
linear.raw.linear_terms
instance.raw.sense
solution.raw.optimality = Optimality.Optimal
constraint.raw.id
Linear.from_raw(rust_linear)
Constraint.from_protobuf(pb_constraint)
dv.to_protobuf()
```

**After (v3)**:
```python
linear.linear_terms
instance.sense
solution.optimality = Optimality.Optimal
# constraint.id is gone — see §3
Linear(...)                     # just call the constructor
instance.to_bytes()             # (de)serialise whole Instance / Solution / SampleSet
```

The dataclass-style constructors (`Instance(raw=..., annotations=...)`) are also gone — `Instance`, `Solution`, `SampleSet` are no longer Python `@dataclass`es. Construct through `Instance.from_components(...)` etc. and set `instance.annotations = {...}` or `instance.add_user_annotation(...)` afterwards.

`Constraint`, `EvaluatedConstraint`, `SampledConstraint`, and `RemovedConstraint` no longer have `to_bytes` / `from_bytes` — a single constraint cannot meaningfully carry its ID on its own now that IDs live in the enclosing dict, so the per-constraint serialisation API was removed. Serialise the containing `Instance` / `Solution` / `SampleSet` instead. (3.0.0a3 extends the same removal to the rest of the non-top-level types — `Function` / `Linear` / `Quadratic` / `Polynomial`, `Parameter`, `NamedFunction` family, `DecisionVariable` family — see §12.)

## 3. Constraint IDs moved out of the `Constraint` object

### 3.1 No more `id` / `set_id()` / `id=` kwarg (`3.0.0a2`, [#806](https://github.com/Jij-Inc/ommx/pull/806))

`Constraint` (and `IndicatorConstraint`, `OneHotConstraint`, `Sos1Constraint`, `RemovedConstraint`, `EvaluatedConstraint`, `SampledConstraint`) no longer carry an ID. The constraint object is **detached** — it gets an ID only when it is placed in the `dict[int, Constraint]` you pass to `Instance.from_components` (see §4).

**Before (v2.5.1)**:
```python
c = Constraint(
    function=x + y,
    equality=Constraint.EQUAL_TO_ZERO,
    id=5,
    name="cap",
)
c.id                 # 5
c.set_id(6)

oh = OneHotConstraint(id=10, variables=[0, 1, 2])
s1 = Sos1Constraint(id=11, variables=[0, 1])
```

**After (v3)**:
```python
c  = Constraint(function=x + y, equality=Constraint.EQUAL_TO_ZERO, name="cap")
oh = OneHotConstraint(variables=[0, 1, 2])
s1 = Sos1Constraint(variables=[0, 1])

# IDs are assigned by the enclosing Instance:
instance = Instance.from_components(
    sense=Instance.MINIMIZE,
    objective=...,
    decision_variables=[...],
    constraints={5: c},
    one_hot_constraints={10: oh},
    sos1_constraints={11: s1},
)
```

### 3.2 Comparison operators return a detached `Constraint` (`3.0.0a2`, [#806](https://github.com/Jij-Inc/ommx/pull/806))

`==`, `<=`, `>=` on `DecisionVariable` / `Parameter` / `Linear` / `Quadratic` / `Polynomial` / `Function` / `NamedFunction` still return a `Constraint`, but with no ID. Assign the ID through the `constraints=` dict.

**Before (v2.5.1)**:
```python
c = (x + y <= 5).set_id(0)
Instance.from_components(..., constraints=[c], ...)
```

**After (v3)**:
```python
c = x + y <= 5
Instance.from_components(..., constraints={0: c}, ...)
```

### 3.3 Global ID-counter helpers removed (`3.0.0a2`, [#806](https://github.com/Jij-Inc/ommx/pull/806))

These module-level names are gone from `ommx._ommx_rust`:

- `CONSTRAINT_ID_COUNTER`
- `next_constraint_id()`
- `set_constraint_id_counter(...)`
- `update_constraint_id_counter(...)`
- `get_constraint_id_counter()`

Constraint IDs no longer exist outside the `BTreeMap` keys inside an `Instance`. If you need a fresh ID for a new constraint, call `instance.next_constraint_id()`.

## 4. Container-type changes (`list` → `dict[int, T]`)

### 4.1 `Instance.from_components(constraints=...)` expects a `dict[int, Constraint]` (`3.0.0a2`, [#806](https://github.com/Jij-Inc/ommx/pull/806))

All constraint-valued arguments are keyed by ID. `decision_variables` stays a `Sequence[DecisionVariable]`.

**Before (v2.5.1)**:
```python
Instance.from_components(
    sense=Instance.MINIMIZE,
    objective=obj,
    decision_variables=[x0, x1],
    constraints=[c0, c1],                 # list; IDs came from Constraint.id
    constraint_hints=ConstraintHints(...) # separate hints object
)
```

**After (v3)**:
```python
Instance.from_components(
    sense=Instance.MINIMIZE,
    objective=obj,
    decision_variables=[x0, x1],
    constraints={0: c0, 1: c1},           # dict keyed by constraint ID
    indicator_constraints={10: ic},       # all structural-constraint args are dicts
    one_hot_constraints={20: oh},
    sos1_constraints={30: sc},
)
```

All arguments are keyword-only. `ParametricInstance.from_components` takes the same `constraints: Mapping[int, Constraint]` shape.

### 4.2 Constraint accessors on `Instance` / `ParametricInstance` / `Solution` return dicts (`3.0.0a2`, [#806](https://github.com/Jij-Inc/ommx/pull/806))

**Before (v2.5.1)**:
```python
for c in instance.constraints:              # list[Constraint]
    print(c.id, c.function)

for rc in instance.removed_constraints:     # list[RemovedConstraint]
    ...

for ec in solution.constraints:             # list[EvaluatedConstraint]
    print(ec.id, ec.evaluated_value)

hints = instance.constraint_hints           # one_hot_constraints / sos1_constraints inside
for oh in hints.one_hot_constraints:
    ...
```

**After (v3)**:
```python
for cid, c in instance.constraints.items():              # dict[int, AttachedConstraint]
    print(cid, c.function)

for cid, rc in instance.removed_constraints.items():     # dict[int, RemovedConstraint]
    ...

for cid, ec in solution.constraints.items():             # dict[int, EvaluatedConstraint]
    print(cid, ec.evaluated_value)

# First-class constraint dicts replace constraint_hints:
for hid, oh in instance.one_hot_constraints.items(): ...
for hid, sc in instance.sos1_constraints.items():    ...
for hid, ic in instance.indicator_constraints.items(): ...
```

The dict shape itself landed in 3.0.0a2 with snapshot `Constraint` values. In 3.0.0a3 the constraint dicts on `Instance` / `ParametricInstance` switched to write-through `AttachedX` handles — see §11 for the read / write semantics. `Solution.constraints` keeps a snapshot value type (`EvaluatedConstraint`) since it has no edit lifecycle. `Instance.removed_constraints` still surfaces `RemovedConstraint` snapshots; relax/restore go through `Instance.relax_constraint` / `Instance.restore_constraint` rather than mutating values inside this dict.

`SampleSet.constraints` / `.decision_variables` / `.named_functions` remain `list`.

## 5. Renames and signature changes

### 5.1 `write_mps` → `save_mps` (`3.0.0a1`, [#775](https://github.com/Jij-Inc/ommx/pull/775))

```python
# v2.5.1
instance.write_mps("out.mps.gz")

# v3
instance.save_mps("out.mps.gz")                 # compress=True by default
instance.save_mps("out.mps", compress=False)
```

### 5.2 `Instance.used_decision_variable_ids()` → `Instance.required_ids()` (`3.0.0a2`, [#806](https://github.com/Jij-Inc/ommx/pull/806))

```python
# v2.5.1
instance.used_decision_variable_ids()
func.used_decision_variable_ids()               # on Function as well

# v3
instance.required_ids()
func.required_ids()
```

(`used_decision_variable_ids()` is still the name on `EvaluatedConstraint`, `SampledConstraint`, `EvaluatedDecisionVariable`, `EvaluatedNamedFunction`, `SampledNamedFunction`.)

### 5.3 `Parameter.new(id=...)` → `Parameter(id, ...)` (`3.0.0a1`, [#770](https://github.com/Jij-Inc/ommx/pull/770))

The `.new` factory is removed; the `id` argument is positional.

```python
# v2.5.1
p = Parameter.new(id=3, name="w", subscripts=[0])

# v3
p = Parameter(3, name="w", subscripts=[0])
```

### 5.4 `ParametricInstance.with_parameters` takes a plain dict (`3.0.0a1`, [#774](https://github.com/Jij-Inc/ommx/pull/774))

The `Parameters(entries=...)` wrapper is gone.

```python
# v2.5.1
from ommx.v1 import Parameters
pi.with_parameters(Parameters(entries={p.id: 1.0}))

# v3
pi.with_parameters({p.id: 1.0})
```

### 5.5 `Linear(terms=..., constant=...)` always takes `dict[int, float]` (`3.0.0a1`, [#770](https://github.com/Jij-Inc/ommx/pull/770), [#776](https://github.com/Jij-Inc/ommx/pull/776))

v2.5.1 had a protobuf form (`Linear(terms=[Linear.Term(id=j, coefficient=c) for ...], constant=-b)`) via `linear_pb2`. In v3 `terms` is always `dict[int, float]` and `Linear.Term` does not exist.

```python
# v2.5.1 (protobuf path)
from ommx.v1.linear_pb2 import Linear
Linear(
    terms=[Linear.Term(id=j, coefficient=c) for j, c in enumerate(row)],
    constant=-b,
)

# v3
from ommx.v1 import Linear
Linear(terms={int(j): float(c) for j, c in enumerate(row)}, constant=float(-b))
```

## 6. Return-type changes

### 6.1 `Constraint.name` / `Constraint.description` are `Optional[str]` (`3.0.0a1`, [#770](https://github.com/Jij-Inc/ommx/pull/770), [#771](https://github.com/Jij-Inc/ommx/pull/771))

v2.5.1 declared them `str` (empty string when unset). v3 declares `Optional[str]` and returns `None`. This also applies to `RemovedConstraint`, `IndicatorConstraint`, `EvaluatedConstraint`, `SampledConstraint`, `NamedFunction`, `EvaluatedNamedFunction`, `SampledNamedFunction`.

```python
# v2.5.1
name: str = constraint.name                      # "" when unset

# v3
name: Optional[str] = constraint.name            # None when unset
if constraint.name:                              # still works for both
    print(constraint.name)
```

### 6.2 `Linear.terms` / `Quadratic.terms` / `Polynomial.terms` are methods, not properties (`3.0.0a2`, [#806](https://github.com/Jij-Inc/ommx/pull/806))

Only `Function.terms` remains a property. The three building-block types switched to methods.

```python
# v2.5.1
linear.terms                                     # property
quadratic.terms                                  # property
polynomial.terms                                 # property

# v3
linear.terms()                                   # method call
quadratic.terms()
polynomial.terms()
```

`Linear.linear_terms`, `Quadratic.linear_terms` / `quadratic_terms`, and `Polynomial.constant_term` stay properties.

### 6.3 `DecisionVariable.BINARY`/`INTEGER`/… are `int` sentinels (`3.0.0a1`, [#770](https://github.com/Jij-Inc/ommx/pull/770))

In v2.5.1 these class constants were `Kind` enum members. In v3 they are the underlying `int` values, and `DecisionVariable.kind` returns `int` (the protobuf wire value).

```python
# v2.5.1
DecisionVariable.BINARY        # Kind.Binary
if var.kind == DecisionVariable.INTEGER:  # Kind.Integer == Kind.Integer
    ...

# v3
DecisionVariable.BINARY        # 1 (int)
if var.kind == DecisionVariable.INTEGER:  # int == int
    ...
# If you want the enum, construct it: Kind(var.kind)
```

### 6.4 `SampleSet.sample_ids` changed from list-property to set-method (`3.0.0a1`, [#775](https://github.com/Jij-Inc/ommx/pull/775))

```python
# v2.5.1
ids: list[int] = sample_set.sample_ids           # @property

# v3
ids: set[int]  = sample_set.sample_ids()         # method
ids: list[int] = sample_set.sample_ids_list      # separate property when you need a list
```

### 6.5 `evaluate` / `partial_evaluate` raise `ValueError`, not `RuntimeError` (`3.0.0a1`, [#770](https://github.com/Jij-Inc/ommx/pull/770))

Every `.evaluate(state)` / `.partial_evaluate(state)` method on `Linear`, `Quadratic`, `Polynomial`, `Function`, `Constraint`, `NamedFunction`, and `Instance` now raises `ValueError` (e.g. `ValueError: Missing entry for id: 2`) when the state is missing a required decision-variable ID or the atol is invalid. In v2.5.1 the same error surfaced as `RuntimeError` via anyhow. Update `except` clauses accordingly.

```python
# v2.5.1
try:
    linear.evaluate({1: 1})
except RuntimeError as e:
    ...

# v3
try:
    linear.evaluate({1: 1})
except ValueError as e:
    ...
```

### 6.6 `ParametricInstance.parameters` returns `list[Parameter]`, use `parameters_df()` for the DataFrame (`3.0.0a1`, [#774](https://github.com/Jij-Inc/ommx/pull/774); `3.0.0a3`, [#846](https://github.com/Jij-Inc/ommx/pull/846))

The DataFrame view moved to a separate `_df` accessor, mirroring `decision_variables` / `decision_variables_df()` and `constraints` / `constraints_df()`. The bare `parameters` attribute is now an ordered `list[Parameter]`. The split itself landed in 3.0.0a1 (#774) when `ParametricInstance` became a Rust re-export; the `_df` accessor flipped from a `#[getter]` property to a method call in 3.0.0a3 (#846), at which point every `*_df` on `Instance` / `ParametricInstance` / `Solution` / `SampleSet` requires parentheses (see §9 below).

```python
# v2.5.1 (DataFrame view)
parametric_instance.parameters            # -> pandas.DataFrame

# v3
parametric_instance.parameters            # -> list[Parameter]
parametric_instance.parameters_df()       # -> pandas.DataFrame  (method, not property)
```

## 7. Removed helpers (`3.0.0a1`, [#770](https://github.com/Jij-Inc/ommx/pull/770), [#776](https://github.com/Jij-Inc/ommx/pull/776), [#782](https://github.com/Jij-Inc/ommx/pull/782); `3.0.0a2`, [#798](https://github.com/Jij-Inc/ommx/pull/798))

- `Linear.from_object(x)` — construct via `Linear.single_term(...)`, `Linear.constant(...)`, or the arithmetic operators.
- `Linear.equals_to(other)` — use `linear.almost_equal(other, atol=...)`. (Available on every expression type.)
- `instance.constraint_hints` — replaced by `instance.one_hot_constraints` / `sos1_constraints` / `indicator_constraints`.
- `Parameters` / `OneHot` / `Sos1` / `ConstraintHints` — see §1.2.
- `Artifact` low-level types (`ArtifactArchive`, `ArtifactDir`, `ArtifactArchiveBuilder`, `ArtifactDirBuilder`) — replaced by unified `Artifact` / `ArtifactBuilder`.

```python
# v2.5.1
from ommx.artifact import ArtifactArchive, ArtifactDir
archive = ArtifactArchive.from_oci_archive(path)
dir_art = ArtifactDir.from_oci_dir(path)

# v3
from ommx.artifact import Artifact
artifact = Artifact.load_archive("path/to/file.ommx")   # file or directory
artifact = Artifact.load("ghcr.io/jij-inc/ommx/...")    # remote registry
```

## 8. Snapshot `Constraint` setters return a clone, not `self` (`3.0.0a1`, [#770](https://github.com/Jij-Inc/ommx/pull/770), [#771](https://github.com/Jij-Inc/ommx/pull/771))

v2's `Constraint.add_name(...)` / `add_subscripts(...)` / `add_description(...)` mutated the Python wrapper in place and returned `self` (the same object), so chained calls on a held reference accumulated correctly. v3's setters still mutate in place but return `self.clone()` — a fresh wrapper. Single calls behave the same; **chained calls without reassignment lose every mutation past the first** because the chain operates on clones from that point on.

```python
# Single call — identical behavior in v2 and v3
constraint = x == 1
constraint.add_name("test")
print(constraint.name)                # "test" in both versions

# Chained calls without reassignment — diverges
constraint = x == 1
constraint.add_name("a").add_subscripts([0])

# v2: constraint.name == "a" AND constraint.subscripts == [0]
#     (chain mutated `constraint` itself end-to-end)
# v3: constraint.name == "a" but constraint.subscripts == []
#     (only add_name landed in `constraint`; add_subscripts mutated the clone)

# Robust pattern that works in both: assign or chain into a fresh binding
constraint = (x == 1).add_name("test").add_description("A test constraint")
```

For constraints retrieved from an instance (`instance.constraints[id]`), use the [`AttachedConstraint`](https://github.com/Jij-Inc/ommx/pull/849) write-through API in §11 — its `set_*` / `add_*` methods write back to the instance's SoA store regardless of how you call them.

## 9. DataFrame accessors are methods, with `kind=` / `include=` / `removed=` (`3.0.0a3`, [#846](https://github.com/Jij-Inc/ommx/pull/846), [#847](https://github.com/Jij-Inc/ommx/pull/847))

Every `*_df` accessor on `Instance` / `ParametricInstance` / `Solution` / `SampleSet` is a method call now, and the per-kind family on each host (`constraints_df`, `indicator_constraints_df`, `one_hot_constraints_df`, `sos1_constraints_df`, plus the parallel `removed_*_constraints_df` and `*_removed_reasons_df` families) collapsed into one `constraints_df(kind=...)` per host. Optional column families are gated by an `include=` parameter.

```python
# v2.5.1
df = instance.constraints_df             # property, regular constraints only
df = instance.indicator_constraints_df   # separate accessor per kind
df = instance.removed_constraints_df     # separate active vs. removed
df = solution.constraints_df

# v3
df = instance.constraints_df()           # method; default kind="regular"
df = instance.constraints_df(kind="indicator")
df = instance.constraints_df(kind="regular", removed=True)
                                         # active + removed merged in id order
df = solution.constraints_df()           # no removed= (no active/removed
                                         # distinction at the evaluated stage)
```

`kind` accepts `Literal["regular", "indicator", "one_hot", "sos1"]` (default `"regular"`); unknown values raise `ValueError`. Solution / SampleSet have no `removed=` parameter — at the evaluated / sampled stage every row is materialized regardless of how it was lifecycled, and reason data is gated by `"removed_reason"` in `include=` instead.

`include` accepts a `Sequence[str]` of `"metadata"` / `"parameters"` / `"removed_reason"` (singular). The default (`None`) preserves the v2 wide shape (`("metadata", "parameters")`); `include=[]` drops every optional column family.

```python
# Default — v2-equivalent shape (metadata + parameters columns)
df = instance.constraints_df()

# Core only — drop metadata and parameters
df = instance.constraints_df(include=[])

# Active + removed in one DataFrame; reason columns auto-added
df = instance.constraints_df(removed=True)
# columns include: equality, function_type, used_ids,
#                  name, subscripts, description, parameters.{key},
#                  removed_reason, removed_reason.{key}

# decision_variables_df takes include= but no kind= or removed=
df = instance.decision_variables_df()
df = instance.decision_variables_df(include=[])
```

`"removed_reason"` is a unit flag — it gates both the `removed_reason` column and the `removed_reason.{key}` parameter columns together. The `removed_reason` column is **schema-stable**: when the flag is on it always appears in the resulting DataFrame, NA-filled if no row carries a reason, so downstream code that branches on schema doesn't need to special-case empty data.

The wide `constraints_df()` index column was renamed from unqualified `id` to `{kind}_constraint_id` (`regular_constraint_id`, `indicator_constraint_id`, `one_hot_constraint_id`, `sos1_constraint_id`). `decision_variables_df()` keeps `id` as its index name (only one variable ID space, so disambiguation isn't load-bearing); the long-format variable sidecars in §10 do use `variable_id`. The kind-qualified constraint names make cross-ID-space joins (which would silently produce wrong-but-shaped output when `int64` indexes line up) visible in `df.head()` / `df.info()` and IDE inspection.

## 10. Long-format sidecar DataFrames (`3.0.0a3`, [#846](https://github.com/Jij-Inc/ommx/pull/846))

`Instance` / `ParametricInstance` / `Solution` / `SampleSet` gained six long-format / id-indexed sidecar DataFrame methods that read directly from the SoA metadata stores:

```python
# Constraint-side — kind= dispatches across the four constraint families
instance.constraint_metadata_df(kind="regular")
                                          # name, subscripts, description
                                          # index: regular_constraint_id
instance.constraint_parameters_df(kind="regular")
                                          # columns: regular_constraint_id, key, value
instance.constraint_provenance_df(kind="regular")
                                          # columns: regular_constraint_id, step,
                                          #          source_kind, source_id
instance.constraint_removed_reasons_df(kind="regular")
                                          # columns: regular_constraint_id, reason,
                                          #          key, value

# Variable-side — single ID space, no kind=
instance.variable_metadata_df()
instance.variable_parameters_df()
```

Use these for tidy-data joins / aggregation; reach for the wide `constraints_df()` (with `include=`) when you want one row per id with columns alongside.

`provenance` is intentionally not folded into `constraints_df()` via `include=`: chains have variable length, and a wide pivot would either explode the column space or produce an object-dtype list column. Pivot the long-format `constraint_provenance_df()` yourself if you need a wide view.

## 11. Constraint and variable accessors return `AttachedX` write-through handles (`3.0.0a3`, [#849](https://github.com/Jij-Inc/ommx/pull/849), [#850](https://github.com/Jij-Inc/ommx/pull/850), [#852](https://github.com/Jij-Inc/ommx/pull/852))

The dict / list accessors that previously returned snapshot wrapper objects now return `AttachedX` write-through handles bound to the parent host (`Instance` or `ParametricInstance`). Reads pull live from the host's SoA store; metadata setters write back through to it.

```python
# v2.5.1 — id-keyed lookup via get_constraint_by_id; snapshot wrapper,
# mutation didn't propagate to the instance
c = instance.get_constraint_by_id(5)
c.add_name("balance")                              # mutated the local snapshot
print(instance.get_constraint_by_id(5).name)       # still None — fresh snapshot

# v3 — dict accessor returns a write-through handle
c = instance.constraints[5]                        # AttachedConstraint (live)
c.set_name("balance")                              # writes through to the SoA store
print(instance.constraints[5].name)                # "balance"
```

Affected return types (the column for `3.0.0a2` reflects the post-§4.2 state with snapshot value types; this section's change is the wrap into `AttachedX`):

| Accessor | v2.5.1 | 3.0.0a2 | v3 final (3.0.0a3) |
|---|---|---|---|
| `instance.constraints` | `list[Constraint]` | `dict[int, Constraint]` | `dict[int, AttachedConstraint]` |
| `instance.indicator_constraints` | — (no indicator type) | `dict[int, IndicatorConstraint]` | `dict[int, AttachedIndicatorConstraint]` |
| `instance.one_hot_constraints` | via `constraint_hints` (legacy `OneHot`) | `dict[int, OneHotConstraint]` | `dict[int, AttachedOneHotConstraint]` |
| `instance.sos1_constraints` | via `constraint_hints` (legacy `Sos1`) | `dict[int, Sos1Constraint]` | `dict[int, AttachedSos1Constraint]` |
| `instance.decision_variables` | `list[DecisionVariable]` | `list[DecisionVariable]` | `list[AttachedDecisionVariable]` |

The list → dict shape change happened in 3.0.0a2 ([§4.2](#42-constraint-accessors-on-instance--parametricinstance--solution-return-dicts)); the 3.0.0a3 wave wraps each value in an `AttachedX` write-through handle. The same change applies on `ParametricInstance`. Solution / SampleSet evaluated / sampled wrappers stay as snapshots — those collections have no edit lifecycle.

The snapshot wrapper types (`Constraint`, `IndicatorConstraint`, `OneHotConstraint`, `Sos1Constraint`, `DecisionVariable`) are unchanged in shape and remain the modeling-input type — operator overloading (`x + y == 1`), expression building, and `Instance.from_components(constraints={...})` all keep accepting / returning them. New `add_*` entry points consume snapshots and return the matching attached handle:

```python
c = (x[0] + x[1] == 1).set_name("balance")     # Constraint snapshot

attached = instance.add_constraint(c)          # -> AttachedConstraint
attached.set_subscripts([0])                   # writes through

# Single-id lookup also returns an attached handle
print(instance.constraints[attached.constraint_id].name)   # "balance"

# attached_decision_variable(id) is the dedicated lookup for variables
av = instance.attached_decision_variable(0)
av.set_name("x_0")
```

`AttachedX` exposes `.detach()` to materialize an independent snapshot when you need one (e.g. to send through `from_components`, ship via `to_bytes`, or hand off to code that expects the modeling type). `AttachedDecisionVariable` participates in arithmetic via `ToFunction` (only its id is consumed, no host borrow is taken), so existing expression-building code keeps working without `.detach()`.

Two `AttachedX` instances pointing at the same id observe the same state — a write through one is visible through any other and through the next `*_df` call. The host stays alive as long as any `AttachedX` references it (the handle holds a refcounted `Py<Instance>` / `Py<ParametricInstance>`); drop the handles to release the host.

## 12. `to_bytes` / `from_bytes` removed from non-top-level types (`3.0.0a3`, [#845](https://github.com/Jij-Inc/ommx/pull/845))

Element-level bytes serialisation is removed from these types:

- `Function`, `Linear`, `Quadratic`, `Polynomial`
- `Parameter`
- `NamedFunction`, `EvaluatedNamedFunction`, `SampledNamedFunction`
- `DecisionVariable`, `EvaluatedDecisionVariable`, `SampledDecisionVariable`

(The `Constraint` family — `Constraint`, `EvaluatedConstraint`, `SampledConstraint`, `RemovedConstraint` — already lost `to_bytes` / `from_bytes` in 3.0.0a2 along with the `Constraint.id` field; see §2.)

These methods originally existed to ferry values across the Python ↔ Rust boundary back when the Python SDK had its own protobuf-based wrapper layer. With the v3 transition to direct PyO3 re-exports the boundary is gone, so element-level bytes round-trips no longer serve a purpose. Persist or exchange data through the **container types** instead:

- `Instance.to_bytes()` / `Instance.from_bytes(...)` (and the same on `ParametricInstance`, `Solution`, `SampleSet`)
- `State.to_bytes()` / `Samples.to_bytes()` / `Parameters.to_bytes()` for the cross-`evaluate` DTOs

```python
# Before (2.5.1 / 3.0.0a2)
blob = my_function.to_bytes()
f    = Function.from_bytes(blob)

dv_blob = decision_variable.to_bytes()
dv      = DecisionVariable.from_bytes(dv_blob)

# After (3.0.0a3) — wrap in the enclosing container and round-trip that
instance      = Instance.from_components(
    sense=Instance.MINIMIZE,
    objective=my_function,
    decision_variables=[decision_variable],
    constraints={},
)
instance_blob = instance.to_bytes()
restored      = Instance.from_bytes(instance_blob)
my_function   = restored.objective
decision_variable = restored.decision_variables[0].detach()
```

## 13. Artifact API: archive becomes an exchange format

v3 redraws the artifact API around a single canonical store — the SQLite Local Registry — and treats `.ommx` files purely as an exchange format. Every artifact goes through `LocalArtifactBuilder` and lands in the registry; the archive file is produced as an explicit export afterward. The v2 split between "archive build" and "registry build" is gone, along with the v2 in-place "read archive without touching the registry" path.

For PR references see [#872](https://github.com/Jij-Inc/ommx/pull/872).

### 13.1 `ArtifactBuilder.new_archive` / `new_archive_unnamed` removed; use `new` + `Artifact.save(path)`

`ArtifactBuilder.new_archive(path, image_name)` and `ArtifactBuilder.new_archive_unnamed(path)` are gone. The "produce a `.ommx` file" step is now a separate `Artifact.save(path)` call after `build()`.

**Before (v2 / v3-alpha pre-#872)**:
```python
from ommx.artifact import ArtifactBuilder

builder = ArtifactBuilder.new_archive("my_instance.ommx", "ghcr.io/jij-inc/ommx/demo:v1")
builder.add_instance(instance)
artifact = builder.build()    # writes the .ommx file as a side effect
```

**After (v3, ≥ #872)**:
```python
from ommx.artifact import ArtifactBuilder

builder = ArtifactBuilder.new("ghcr.io/jij-inc/ommx/demo:v1")
builder.add_instance(instance)
artifact = builder.build()                # lands in the user's SQLite Local Registry
artifact.save("my_instance.ommx")         # explicit export
```

`Artifact.save(path)` is the new method that emits a `.ommx` file. The path argument carries no naming information; the resulting archive's `org.opencontainers.image.ref.name` annotation is the artifact's registry image name. `save()` errors out with `Output file already exists: ...` if the path is occupied; delete the file first or pick a different name.

### 13.2 `ArtifactBuilder.new_archive_unnamed` → `ArtifactBuilder.new_anonymous`

`new_archive_unnamed` is replaced by `new_anonymous`, which takes no path and synthesizes an OMMX-local image name of the form `<registry-id8>.ommx.local/anonymous:<local-timestamp>-<nonce>` (e.g. `99ea32f6.ommx.local/anonymous:20260512T124922-a363c62f`). Components:

- `<registry-id8>` — first 8 hex chars of a random UUID generated once when the SQLite Local Registry is created. Identifies which registry produced the artifact.
- `<local-timestamp>` — `YYYYMMDDTHHMMSS` in the caller's local time zone (no timezone marker; OCI tag syntax forbids `+` and using a fixed UTC marker would defeat the at-a-glance readability of the date).
- `<nonce>` — 8-hex random suffix, so concurrent / scripted anonymous builds (MINTO-style workflows) never collide on the same wall-clock second.

The hostname `<registry-id8>.ommx.local` uses the `.local` mDNS link-local TLD (RFC 6762), so an accidental `ommx push` of an anonymous artifact does **not** leak to a real remote registry.

**Before (v2 / v3-alpha pre-#872)**:
```python
builder = ArtifactBuilder.new_archive_unnamed("my_instance.ommx")
builder.add_instance(instance)
artifact = builder.build()
print(artifact.image_name)        # None
```

**After (v3, ≥ #872)**:
```python
builder = ArtifactBuilder.new_anonymous()
builder.add_instance(instance)
artifact = builder.build()
artifact.save("my_instance.ommx")
print(artifact.image_name)        # "99ea32f6.ommx.local/anonymous:20260512T124922-a363c62f"
```

Two behavioural shifts:

1. `image_name` is now a (synthesized) string, never `None`. v2 anonymous archives surfaced `None`; in v3 the SQLite Local Registry needs a key for every artifact so the builder synthesizes one. Code that branched on `image_name is None` to detect "unnamed archive" needs to switch to checking the `local.ommx/anonymous:` substring or — better — call `ArtifactBuilder.new(image_name)` with an explicit name when you care about identity.
2. `new_anonymous` accumulates entries in the SQLite Local Registry. Run `ommx artifact prune-anonymous` to clean them up periodically; the manifest / blob CAS records are intentionally left in place for a future GC sweep to reclaim.

**Timezone caveat**: the timestamp portion is the **builder's local time**, not UTC. If you ship an anonymous archive to someone in another timezone, the recipient reads the same digits as their own local time — the time component loses absolute meaning across machines. If absolute time matters, use `ArtifactBuilder.new(image_name)` with an explicit name.

### 13.3 `Artifact.load_archive` removed; pick `import_archive` (write) or `inspect_archive` (read-only)

v2's `Artifact.load_archive(file)` opened a `.ommx` archive in place with no side effect on the local registry. v3 splits that contract into two named methods with explicit semantics, and the v2 name raises a migration error so an upgrade cannot silently write into the SQLite Local Registry:

- {func}`Artifact.import_archive(file) <ommx.artifact.Artifact.import_archive>` — imports the archive into the user's persistent SQLite Local Registry under the archive's `org.opencontainers.image.ref.name` annotation, returns a full {class}`Artifact` handle (you can read every layer via `get_layer` / `get_blob`). The v3 replacement of `load_archive`'s "I want to use this archive" path.
- {func}`Artifact.inspect_archive(file) <ommx.artifact.Artifact.inspect_archive>` — reads only the manifest + layer descriptors without writing into the registry. Returns a new lightweight {class}`ArchiveManifest <ommx.artifact.ArchiveManifest>` with `image_name` / `manifest_digest` / `layers` / `annotations`. Use this for "what is in this archive" without committing to an import. Blob bodies are not accessible from `ArchiveManifest`; import first if you need them.

**Before (v2)**:
```python
# In-memory open, no registry side effect; layer access works.
artifact = Artifact.load_archive("my_instance.ommx")
print(artifact.image_name)
print(artifact.instance)
```

**After (v3) — full import** (matches the v2 "I will work with this archive" use):
```python
# Imports into ~/Library/Application Support/org.ommx.ommx/
# (or $OMMX_LOCAL_REGISTRY_ROOT). Subsequent Artifact.load(image_name)
# calls resolve from SQLite without re-importing.
artifact = Artifact.import_archive("my_instance.ommx")
print(artifact.image_name)
print(artifact.instance)
```

**After (v3) — read-only inspect** (no registry write):
```python
manifest = Artifact.inspect_archive("my_instance.ommx")
print(manifest.image_name)
for layer in manifest.layers:
    print(layer.media_type)
```

Calling `Artifact.load_archive(...)` in v3 raises a `RuntimeError` whose message names both replacements and explains the semantic shift.

If the archive's `index.json` descriptor lacks an `org.opencontainers.image.ref.name` annotation — the shape v2's `ArtifactBuilder.new_archive_unnamed(path)` produced — `import_archive` does **not** refuse the import. It synthesizes a fresh anonymous name (`<registry-id8>.ommx.local/anonymous:<local-timestamp>-<nonce>`, the same shape §13.2 documents) against the destination registry's `registry_id` and registers the archive under that name. v2 archives without a ref annotation therefore continue to load on upgrade; you can sweep them later via `ommx artifact prune-anonymous`. Anonymous archives produced by `ArtifactBuilder.new_anonymous` already carry their synthesized name and re-import under that name unchanged.

`ommx inspect <archive>` (the CLI command) is the CLI equivalent of `Artifact.inspect_archive` — both read the manifest without touching the SQLite Local Registry.

### 13.4 `ommx push <archive>` removed; load first, then push by name

The CLI no longer accepts an archive file or OCI Image Layout directory as the argument to `push`. v3 pushes always source from the SQLite Local Registry. The migration is the explicit two-step pattern documented above:

**Before**:
```bash
ommx push my_instance.ommx
```

**After**:
```bash
ommx load my_instance.ommx
ommx push <image_name>
```

Running the old form prints a migration hint and exits non-zero.

### 13.5 New CLI: `ommx artifact prune-anonymous [--dry-run] [--root <path>]`

Bulk-delete every SQLite ref whose `(name, reference)` matches the anonymous-artifact synthetic shape (`<8-hex>.ommx.local/anonymous:<timestamp>-<nonce>`). Cleans entries from every registry-id prefix the SQLite registry has seen, not just the current host's. Manifest / blob CAS records survive the prune and will be reclaimed by a future GC sweep; the prune itself is intentionally cheap.

```bash
ommx artifact prune-anonymous --dry-run        # list what would be removed
ommx artifact prune-anonymous                  # delete them
```

The structural match (8-hex registry-id prefix, timestamp-shaped tag, hex nonce) prevents a human-pushed real ref like `myhost.ommx.local/anonymous:v1` from being misclassified as anonymous.

## 14. Convenience additions (not breaking)

### 14.1 `DecisionVariable.binary` / `integer` / `continuous` accept `lower` / `upper` kwargs

```python
# v3
x = DecisionVariable.integer(1, lower=0, upper=10)
y = DecisionVariable.continuous(2, lower=-1.0, upper=1.0)
z = DecisionVariable.integer(3)                  # unbounded
```

### 14.2 `DecisionVariable.equals_to(other)` for object equality

Because `==` creates a `Constraint`, v3 adds an explicit `equals_to` method (and the same for `Parameter` / `Linear` / …) for `bool` equality.

```python
x = DecisionVariable.integer(1, lower=0, upper=10)
y = DecisionVariable.integer(1, lower=0, upper=10)

c = x == y          # Constraint (not a bool)
x.equals_to(y)      # True
x.id == y.id        # True
```

### 14.3 `Parameter` supports the same operators as `DecisionVariable`

```python
from ommx.v1 import Parameter, DecisionVariable

p = Parameter(1, name="param1")
x = DecisionVariable.integer(2, lower=0, upper=10)

expr = x + p      # Linear
expr = x * p      # Quadratic
expr = 2 * p + 3  # Linear
```

## Migration checklist

- [ ] Replace every `from ommx.v1.*_pb2 import ...` with `from ommx.v1 import ...`.
- [ ] Remove all `.raw`, `from_raw(...)`, `from_protobuf(...)`, `to_protobuf(...)` usage; use `from_bytes` / `to_bytes` or direct properties.
- [ ] Replace `Constraint(id=N, ...)` / `.set_id(N)` / `(expr <= 0).set_id(N)` with `{N: (expr <= 0)}` in the `constraints=` dict.
- [ ] Remove reads of `constraint.id`; iterate with `.items()` on constraint dicts instead.
- [ ] Remove any use of `next_constraint_id()` / `set_constraint_id_counter(...)` / related counter helpers.
- [ ] Update `Instance.from_components(constraints=[...])` to pass a `dict[int, Constraint]`. Same for `indicator_constraints`, `one_hot_constraints`, `sos1_constraints`.
- [ ] Replace `constraint_hints` reads with `instance.{one_hot,sos1,indicator}_constraints` (all `dict[int, T]`).
- [ ] Rename `write_mps(...)` → `save_mps(...)`.
- [ ] Rename `instance.used_decision_variable_ids()` / `function.used_decision_variable_ids()` → `required_ids()`.
- [ ] Replace `Parameter.new(id=...)` with `Parameter(id, ...)`.
- [ ] Replace `pi.with_parameters(Parameters(entries={...}))` with `pi.with_parameters({...})`.
- [ ] Update `constraint.name` / `constraint.description` handling for `None` return (was `""`).
- [ ] Update code that used `Linear.terms` / `Quadratic.terms` / `Polynomial.terms` as a property — they are methods now.
- [ ] `SampleSet.sample_ids` is a method returning `set[int]`; use `sample_set.sample_ids_list` if you need a `list`.
- [ ] Change `except RuntimeError` around `.evaluate(...)` / `.partial_evaluate(...)` calls to `except ValueError`.
- [ ] Switch `parametric_instance.parameters` DataFrame reads to `parametric_instance.parameters_df()` (now a method; `.parameters` returns `list[Parameter]`).
- [ ] Audit chained `Constraint.add_name(...).add_subscripts(...)` calls — the chain operates on a clone after the first method, so only the first mutation lands in the original wrapper. Assign the chain to a fresh binding (`c = (...).add_name(...).add_subscripts(...)`), or use the live `AttachedConstraint` from `instance.constraints[id]` for write-through mutation.
- [ ] Replace `ArtifactArchive` / `ArtifactDir` usage with `Artifact.load_archive(...)` or `Artifact.load(...)`.
- [ ] Remove any `Linear.from_object(...)` / `Linear.equals_to(...)` calls.
- [ ] Add parentheses to every `*_df` access — `instance.constraints_df` → `instance.constraints_df()` etc. (every `*_df` accessor is a method now).
- [ ] Replace per-kind `instance.indicator_constraints_df` / `one_hot_constraints_df` / `sos1_constraints_df` and `removed_*_constraints_df` / `*_removed_reasons_df` calls with `constraints_df(kind=..., removed=...)` on the same host.
- [ ] If you depended on the unqualified `id` index column on a wide constraint `*_df`, switch to the kind-qualified `{kind}_constraint_id` name. `decision_variables_df()` keeps `id` (only one variable ID space); long-format variable sidecars use `variable_id`.
- [ ] Drop the in-place `c.add_name(...)` mutation pattern on snapshot wrappers retrieved from an instance — those calls return a new object and don't write through to the host. Use the live handle returned by `instance.constraints[id]` (an `AttachedConstraint`) and call its `set_*` / `add_*` methods, or re-add via `from_components`.
- [ ] Update return-type annotations / static analysis for `instance.constraints` etc. to expect `AttachedX` (`dict[int, AttachedConstraint]`, `list[AttachedDecisionVariable]`, …). Call `.detach()` if you need an independent snapshot.
- [ ] Replace element-level `to_bytes()` / `from_bytes()` calls on `Function` / `Linear` / `Quadratic` / `Polynomial` / `Parameter` / the `NamedFunction` family / the `DecisionVariable` family with whole-`Instance` / `Solution` / `SampleSet` round-trips (or the `State` / `Samples` / `Parameters` DTOs for evaluate plumbing). See §12.
- [ ] Replace `ArtifactBuilder.new_archive(path, image_name).build()` with `ArtifactBuilder.new(image_name).build()` + `artifact.save(path)`. See §13.1.
- [ ] Replace `ArtifactBuilder.new_archive_unnamed(path).build()` with `ArtifactBuilder.new_anonymous().build()` + `artifact.save(path)`. Audit code that branched on `artifact.image_name is None` — anonymous artifacts now have a synthesized `<...>.ommx.local/anonymous:...` name. See §13.2.
- [ ] Replace `Artifact.load_archive(file)` with `Artifact.import_archive(file)` (registry-write semantics, returns a full handle) for code that wants to use the archive's contents. Use `Artifact.inspect_archive(file)` for the side-effect-free read of the manifest / layer descriptors (returns an `ArchiveManifest`). The v3 `load_archive` raises a migration error pointing at both. See §13.3.
- [ ] Update any `ommx push <archive-file>` invocation to the two-step `ommx load <file>` + `ommx push <image_name>` flow. See §13.4.
- [ ] Add periodic `ommx artifact prune-anonymous` to clean accumulated entries if your workflow makes heavy use of `ArtifactBuilder.new_anonymous`. See §13.5.

---

# Python SDK v1 to v2 Migration Guide

This section covers the migration from Protocol Buffer-based (v1) to Rust-PyO3-based (v2).

## ⚠️ Important: Deprecation of `raw` Attributes

In v2, the `raw` attribute is deprecated across all migrated classes (`Instance`, `Solution`, `SampleSet`, etc.). Direct access to `.raw` should be avoided. Instead, use the methods directly available on the classes themselves:

**Examples:**
```python
# ❌ Deprecated: Don't access through raw
solution.raw.evaluated_constraints[0].evaluated_value
instance.raw.decision_variables
sample_set.raw.samples

# ✅ Recommended: Use direct properties and methods
solution.get_constraint_value(0)
instance.decision_variables  # Now a property returning a list
sample_set.get(sample_id)
```

These direct methods provide:
- Better performance through native Rust implementation
- Improved type safety
- Cleaner, more intuitive APIs

While `raw` attributes remain available for backward compatibility, they will be removed in future versions.

## Import Changes

**Old approach (v1)**:
```python
from ommx.v1.constraint_pb2 import Constraint, Equality
from ommx.v1.function_pb2 import Function
from ommx.v1.linear_pb2 import Linear
from ommx.v1 import Instance, DecisionVariable
```

**New approach (v2) - Recommended**:
```python
# Import everything uniformly from ommx.v1
from ommx.v1 import (
    Instance, DecisionVariable, Constraint,
    Function, Linear, Quadratic, Polynomial,
    Solution, State, SampleSet,
    # New evaluated types (v2.0.0rc3+)
    EvaluatedDecisionVariable, EvaluatedConstraint,
    SampledDecisionVariable, SampledConstraint
)
```

## DecisionVariable Factory Methods

**Still available**:
```python
# of_type method (still available)
DecisionVariable.of_type(
    DecisionVariable.BINARY, var.idx, 
    lower=var.lb, upper=var.ub, name=var.name
)
```

**Newly added methods**:
```python
# More concise type-specific factory methods
DecisionVariable.binary(var.idx, name=var.name)
DecisionVariable.integer(var.idx, lower=var.lb, upper=var.ub, name=var.name)  
DecisionVariable.continuous(var.idx, lower=var.lb, upper=var.ub, name=var.name)
```

## Function Creation

**Old approach**:
```python
# Direct Protocol Buffer creation
Function(constant=constant)
Function(linear=Linear(terms=terms, constant=constant))
```

**New approach**:
```python
# Unified constructor
Function(constant)  # Create from scalar value
Function(linear)    # Create from Linear object
Function(quadratic) # Create from Quadratic object

# Creating Linear objects
linear = Linear(terms=terms, constant=constant)
```

## Constraint Creation

**Old approach**:
```python
# Direct Protocol Buffer creation
Constraint(
    id=id,
    equality=Equality.EQUALITY_EQUAL_TO_ZERO,
    function=function,
    name=name,
)
```

**New approach**:
```python
# Direct constructor creation (using ommx.v1.Function)
constraint = Constraint(
    id=id,
    function=function,  # Use ommx.v1.Function
    equality=Constraint.EQUAL_TO_ZERO,  # Use Python SDK constants
    name=name,
)
```

## Function Inspection and Conversion

**Old approach**:
```python
# Protocol Buffer HasField check
if function.HasField("linear"):
    linear_terms = function.linear.terms
    constant = function.linear.constant
```

**New approach**:
```python
# Check polynomial degree using Function.degree() and use direct property access
degree = function.degree()
if degree == 0:
    # Constant function
    constant = function.constant_term
elif degree == 1:
    # Linear function - direct property access
    linear_terms = function.linear_terms      # dict[int, float]
    constant = function.constant_term         # float
elif degree == 2:
    # Quadratic function - direct property access
    quadratic_terms = function.quadratic_terms  # dict[tuple[int, int], float]
    linear_terms = function.linear_terms        # dict[int, float]
    constant = function.constant_term           # float

# Real adapter usage example (PySCIPOpt):
def _make_linear_expr(self, f: Function) -> pyscipopt.Expr:
    return (
        pyscipopt.quicksum(
            coeff * self.varname_map[str(id)]
            for id, coeff in f.linear_terms.items()
        )
        + f.constant_term
    )

def _make_quadratic_expr(self, f: Function) -> pyscipopt.Expr:
    # Quadratic terms
    quad_terms = pyscipopt.quicksum(
        self.varname_map[str(row)] * self.varname_map[str(col)] * coeff
        for (row, col), coeff in f.quadratic_terms.items()
    )
    # Linear terms
    linear_terms = pyscipopt.quicksum(
        coeff * self.varname_map[str(var_id)]
        for var_id, coeff in f.linear_terms.items()
    )
    return quad_terms + linear_terms + f.constant_term
```


## Migration Steps

1. **Update imports**: Remove direct Protocol Buffer imports (`*_pb2`) and change to unified imports from `ommx.v1`
2. **Change Function inspection**: Replace `.HasField()` with `.degree()` checks and direct property access
3. **Utilize new methods**: More concise type-specific factory methods (`binary()`, `integer()`, `continuous()`) are available

## Common Issues and Solutions

- **`AttributeError: 'builtins.Function' object has no attribute 'HasField'`**: Use `.degree()` check followed by direct property access (`.linear_terms`, `.constant_term`, etc.)
- **`TypeError: 'float' object is not callable`**: Access `function.constant_term` as a property, not `function.constant_term()`
- **Using `.raw` attributes**: The `raw` attribute is deprecated. Use methods directly available on the classes (e.g., `solution.get_constraint_value()`, `instance.decision_variables`) for better performance and type safety

## Important Notes

- Import everything uniformly from `ommx.v1` and avoid direct Protocol Buffer imports
- When determining constraint types, check in order of increasing degree (constant → linear → quadratic)

## Newly Available Methods

### Function Class
```python
# Information retrieval
function.degree() -> int      # Function degree
function.num_terms() -> int   # Number of terms

# Direct property access (recommended)
function.constant_term      # float - constant term
function.linear_terms       # dict[int, float] - linear term coefficients
function.quadratic_terms    # dict[tuple[int, int], float] - quadratic term coefficients

# Evaluation
function.evaluate(state: State | dict[int, float]) -> float
function.partial_evaluate(state: State | dict[int, float]) -> Function
```

### Solution Class (v2.0.0rc3+)
```python
# New list-based properties (consistent with Instance)
solution.decision_variables  # list[EvaluatedDecisionVariable] - sorted by ID
solution.constraints        # list[EvaluatedConstraint] - sorted by ID

# New individual access methods
solution.get_decision_variable_by_id(variable_id: int) -> EvaluatedDecisionVariable
solution.get_constraint_by_id(constraint_id: int) -> EvaluatedConstraint
```

### SampleSet Class (v2.0.0rc3+)
```python
# New list-based properties (consistent with Instance)
sample_set.decision_variables # list[SampledDecisionVariable] - sorted by ID
sample_set.constraints       # list[SampledConstraint] - sorted by ID

# New individual access methods
sample_set.get_sample_by_id(sample_id: int) -> Solution  # Alias for get()
sample_set.get_decision_variable_by_id(variable_id: int) -> SampledDecisionVariable
sample_set.get_constraint_by_id(constraint_id: int) -> SampledConstraint
```

## Recommended Implementation Patterns

```python
# Unified imports (v2.0.0rc3+)
from ommx.v1 import (
    Instance, DecisionVariable, Constraint,
    Function, Linear, Solution, State, SampleSet,
    # New evaluated types for consistent API access
    EvaluatedDecisionVariable, EvaluatedConstraint,
    SampledDecisionVariable, SampledConstraint
)

# DecisionVariable creation (new factory methods)
var1 = DecisionVariable.binary(0, name="x1")
var2 = DecisionVariable.integer(1, lower=0, upper=10, name="x2")

# Function inspection (direct property access)
if objective.degree() == 1:
    terms = objective.linear_terms      # dict[int, float]
    constant = objective.constant_term  # float
elif objective.degree() == 2:
    linear_terms = objective.linear_terms        # dict[int, float]
    quadratic_terms = objective.quadratic_terms  # dict[tuple[int, int], float]
    constant = objective.constant_term           # float

# Consistent API patterns across all classes (v2.0.0rc3+)
# All three classes now follow the same pattern:

# Instance
for var in instance.decision_variables:  # list[DecisionVariable]
    process_variable(var.id, var)
var = instance.get_decision_variable_by_id(var_id)  # DecisionVariable

# Solution  
for var in solution.decision_variables:  # list[EvaluatedDecisionVariable]
    process_evaluated_variable(var.id, var.value)
var = solution.get_decision_variable_by_id(var_id)  # EvaluatedDecisionVariable

# SampleSet
for var in sample_set.decision_variables:  # list[SampledDecisionVariable] 
    process_sampled_variable(var.id, var.samples)
var = sample_set.get_decision_variable_by_id(var_id)  # SampledDecisionVariable
```

## State Constructor Changes (PyO3 Migration)

**Enhancement**: `State(entries=...)` constructor enhanced to accept both `dict[int, float]` and `Iterable[tuple[int, float]]`.

**Before (Protobuf)**:
```python
# These worked with protobuf State
state = State(entries=zip(variables, values))  # ✅ Worked
state = State(entries=[(1, 0.5), (2, 1.0)])   # ✅ Worked
```

**After (PyO3) - Enhanced Constructor**:
```python
# All these patterns now work with enhanced PyO3 State constructor
state = State(entries=zip(variables, values))        # ✅ Works with iterables
state = State(entries=[(1, 0.5), (2, 1.0)])         # ✅ Works with iterables  
state = State(entries=dict(zip(variables, values)))  # ✅ Works with dict
state = State(entries={1: 0.5, 2: 1.0})             # ✅ Works with dict
```

**Adapter Code Example**:
```python
# In adapter code (e.g., ommx-openjij-adapter)
def decode_to_samples(response: oj.Response) -> Samples:
    # Both patterns now work with enhanced PyO3 State:
    state = State(entries=zip(response.variables, sample))           # ✅ Works directly
    # OR
    state = State(entries=dict(zip(response.variables, sample)))     # ✅ Also works
```

**Migration Status**:
- ✅ **Completed**: `ommx.v1.State` migrated to PyO3 `_ommx_rust.State`
- ✅ **Completed**: Enhanced State constructor to accept both dict and iterables
- ✅ **Completed**: Adapter compatibility fixes for State constructor changes
  - ✅ OpenJij adapter: Compatible with both `zip()` and `dict(zip())` patterns
  - ✅ PyScipOpt adapter: Enhanced `to_state()` function for protobuf/PyO3 compatibility
  - ✅ Enhanced `ToState` type alias to include legacy protobuf State
- ✅ **Completed**: `ommx.v1.Solution` migrated to PyO3 `_ommx_rust.Solution`
- ✅ **Completed**: `ommx.v1.SampleSet` migrated to PyO3 `_ommx_rust.SampleSet`

## Solution API Changes

**Enhancement**: `Solution` now provides direct methods for accessing constraints and dual variables.

### Accessing Constraint Values

**Before (Protobuf)**:
```python
# Direct access to evaluated_constraints list
solution.raw.evaluated_constraints[0].evaluated_value  # ❌ No longer available
```

**After (PyO3)**:
```python
# Use new getter methods
solution.get_constraint_value(0)  # ✅ Get constraint value by ID
```

### Managing Dual Variables

**Before (Protobuf)**:
```python
# Direct modification of constraint objects
for constraint in solution.raw.evaluated_constraints:
    constraint.dual_variable = dual_variables[constraint.id]  # ❌ No longer available
```

**After (PyO3)**:
```python
# Use new setter/getter methods
solution.set_dual_variable(constraint_id, dual_value)  # ✅ Set dual variable by ID
solution.get_dual_variable(constraint_id)             # ✅ Get dual variable by ID
```

### Accessing Constraint IDs

**Before (Protobuf)**:
```python
# Iterate through evaluated_constraints
for constraint in solution.raw.evaluated_constraints:
    id = constraint.id  # ❌ No longer available
```

**After (PyO3)**:
```python
# Use constraint_ids property
for constraint_id in solution.constraint_ids:  # ✅ Returns set of constraint IDs
    value = solution.get_constraint_value(constraint_id)
```

### Adapter Implementation Examples

**HiGHS Adapter**:
```python
# Old approach
for constraint in solution.raw.evaluated_constraints:
    if constraint.id < row_dual_len:
        constraint.dual_variable = row_dual[constraint.id]

# New approach
for constraint_id in solution.constraint_ids:
    if constraint_id < row_dual_len:
        solution.set_dual_variable(constraint_id, row_dual[constraint_id])
```

**Python-MIP Adapter**:
```python
# Old approach
for constraint in solution.raw.evaluated_constraints:
    id = constraint.id
    if id in dual_variables:
        constraint.dual_variable = dual_variables[id]

# New approach
for constraint_id, dual_value in dual_variables.items():
    solution.set_dual_variable(constraint_id, dual_value)
```

### Complete Solution API Reference

```python
# Properties
solution.objective           # float - objective value
solution.constraint_ids      # set[int] - all constraint IDs
solution.decision_variable_ids  # set[int] - all decision variable IDs
solution.feasible           # bool - feasibility status
solution.feasible_relaxed   # bool - relaxed feasibility status

# New list-based properties (v2.0.0rc3+)
solution.decision_variables  # list[EvaluatedDecisionVariable] - sorted by ID
solution.constraints        # list[EvaluatedConstraint] - sorted by ID

# Methods
solution.get_constraint_value(constraint_id: int) -> float
solution.get_dual_variable(constraint_id: int) -> Optional[float]
solution.set_dual_variable(constraint_id: int, value: Optional[float]) -> None
solution.extract_decision_variables(name: str) -> dict[tuple[int, ...], float]
solution.extract_constraints(name: str) -> dict[tuple[int, ...], float]

# New individual access methods (v2.0.0rc3+)
solution.get_decision_variable_by_id(variable_id: int) -> EvaluatedDecisionVariable
solution.get_constraint_by_id(constraint_id: int) -> EvaluatedConstraint

# State access (backward compatible)
solution.state              # State object with variable values
solution.state.entries      # dict[int, float] - variable ID to value mapping
```

### Solution API Consistency Improvements (v2.0.0rc3+)

**Enhancement**: Solution API now follows the same patterns as Instance for consistency.

**New Properties**:
```python
# List-based access (consistent with Instance)
solution.decision_variables  # list[EvaluatedDecisionVariable] - sorted by ID
solution.constraints        # list[EvaluatedConstraint] - sorted by ID

# Individual access by ID (consistent with Instance)
solution.get_decision_variable_by_id(variable_id: int) -> EvaluatedDecisionVariable
solution.get_constraint_by_id(constraint_id: int) -> EvaluatedConstraint
```

**Migration Pattern**:
```python
# Before: Direct access to constraint values
solution.get_constraint_value(constraint_id)
solution.get_dual_variable(constraint_id)

# After: Access through constraint objects (alternative pattern)
constraint = solution.get_constraint_by_id(constraint_id)
value = constraint.evaluated_value
dual_var = constraint.dual_variable

# Both patterns are supported for backward compatibility
```

## SampleSet API Reference

The `SampleSet` class now provides direct methods for accessing samples and extracting data:

```python
# Properties
sample_set.sample_ids         # set[int] - all sample IDs
sample_set.feasible_ids       # set[int] - feasible sample IDs
sample_set.best_feasible_id   # Optional[int] - best feasible sample ID
sample_set.best_feasible      # Optional[Solution] - best feasible solution

# New list-based properties (v2.0.0rc3+)
sample_set.decision_variables # list[SampledDecisionVariable] - sorted by ID
sample_set.constraints       # list[SampledConstraint] - sorted by ID

# Methods
sample_set.get(sample_id: int) -> Solution
sample_set.extract_decision_variables(name: str, sample_id: int) -> dict[tuple[int, ...], float]
sample_set.extract_constraints(name: str, sample_id: int) -> dict[tuple[int, ...], float]

# New individual access methods (v2.0.0rc3+)
sample_set.get_sample_by_id(sample_id: int) -> Solution  # Alias for get()
sample_set.get_decision_variable_by_id(variable_id: int) -> SampledDecisionVariable
sample_set.get_constraint_by_id(constraint_id: int) -> SampledConstraint
```

### SampleSet API Consistency Improvements (v2.0.0rc3+)

**Enhancement**: SampleSet API now follows the same patterns as Instance and Solution for consistency.

**New Properties**:
```python
# List-based access (consistent with Instance and Solution)
sample_set.decision_variables # list[SampledDecisionVariable] - sorted by ID
sample_set.constraints       # list[SampledConstraint] - sorted by ID

# Individual access by ID (consistent with Instance and Solution)
sample_set.get_sample_by_id(sample_id: int) -> Solution  # Alias for existing get()
sample_set.get_decision_variable_by_id(variable_id: int) -> SampledDecisionVariable
sample_set.get_constraint_by_id(constraint_id: int) -> SampledConstraint
```

**API Consistency Achievement**:
All three core classes now follow the same pattern:
- **Instance**: `decision_variables` → `list[DecisionVariable]`, `get_decision_variable_by_id()` → `DecisionVariable`
- **Solution**: `decision_variables` → `list[EvaluatedDecisionVariable]`, `get_decision_variable_by_id()` → `EvaluatedDecisionVariable`  
- **SampleSet**: `decision_variables` → `list[SampledDecisionVariable]`, `get_decision_variable_by_id()` → `SampledDecisionVariable`

Same patterns apply to constraints and other access methods.

## Adapter API Changes (v2.0-rc.4)

**Breaking Change**: Instance API methods changed from methods to properties and return types changed from dictionaries to lists.

### Instance API Changes

**Before**:
```python
# Methods returning dictionaries
for var_id, var in instance.decision_variables().items():
    process_variable(var_id, var)

for constraint_id, constraint in instance.constraints().items():
    process_constraint(constraint_id, constraint)

# Raw access for iteration
for var_id, var in instance.raw.decision_variables.items():
    process_variable(var_id, var)
```

**After**:
```python
# Properties returning lists (ID-sorted)
for var in instance.decision_variables:
    process_variable(var.id, var)

for constraint in instance.constraints:
    process_constraint(constraint.id, constraint)

# No raw access needed - use properties directly
for var in instance.decision_variables:
    process_variable(var.id, var)
```

### Instance Sense Access

**Before**:
```python
if instance.raw.sense == Instance.MAXIMIZE:
    # Handle maximize
elif instance.raw.sense == Instance.MINIMIZE:
    # Handle minimize
```

**After**:
```python
if instance.sense == Instance.MAXIMIZE:
    # Handle maximize
elif instance.sense == Instance.MINIMIZE:
    # Handle minimize
```

### Adapter Implementation Updates

**Python-MIP Adapter**:
```python
# Before
def _set_decision_variables(self):
    for var_id, var in self.instance.raw.decision_variables.items():
        # Process variable

# After
def _set_decision_variables(self):
    for var in self.instance.decision_variables:
        # Process variable using var.id and var properties
```

**State Creation Pattern**:
```python
# Before
return State(entries={
    var_id: data.var_by_name(str(var_id)).x
    for var_id, var in self.instance.raw.decision_variables.items()
})

# After
return State(entries={
    var.id: data.var_by_name(str(var.id)).x
    for var in self.instance.decision_variables
})
```

### New Instance Methods

**Individual Access**:
```python
# Get specific items by ID (with KeyError on missing ID)
var = instance.get_decision_variable_by_id(variable_id)  # Individual variable access
constraint = instance.get_constraint_by_id(constraint_id)  # Individual constraint access
removed_constraint = instance.get_removed_constraint(constraint_id)  # Individual removed constraint access
```

### Required Adapter Changes

1. **Replace `.raw` access**: Use direct properties instead of `.raw.decision_variables.items()`
2. **Update iteration patterns**: Change from `dict.items()` to direct list iteration
3. **Access individual IDs**: Use `.id` property on each object instead of dict keys
4. **Update sense access**: Use `instance.sense` instead of `instance.raw.sense`
5. **Use new individual access methods**: Use `get_decision_variable_by_id()` and `get_constraint_by_id()` for individual item access

### Migration Checklist for Adapters

- [ ] Replace `instance.raw.decision_variables.items()` → `instance.decision_variables`
- [ ] Replace `instance.raw.constraints.items()` → `instance.constraints`
- [ ] Replace `instance.raw.sense` → `instance.sense`
- [ ] Update variable access from `(var_id, var)` → `var` (use `var.id`)
- [ ] Update constraint access from `(constraint_id, constraint)` → `constraint` (use `constraint.id`)
- [ ] Update test assertions from `len(instance.decision_variables())` → `len(instance.decision_variables)`
- [ ] Use `instance.get_decision_variable_by_id(id)` for individual variable access
- [ ] Use `instance.get_constraint_by_id(id)` for individual constraint access