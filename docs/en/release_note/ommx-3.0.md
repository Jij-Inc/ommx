# OMMX Python SDK 3.0.x

```{note}
Python SDK 3.0.0 contains breaking API changes. A migration guide is available in the [Python SDK v2 to v3 Migration Guide](https://github.com/Jij-Inc/ommx/blob/main/PYTHON_SDK_MIGRATION_GUIDE.md).
```

## Unreleased

Changes merged after the most recent release will be appended here as they land, and promoted to a new version section when the next release is cut.

### ⚠ Artifact API: archive becomes an exchange format ([#872](https://github.com/Jij-Inc/ommx/pull/872))

v3 redraws the artifact API around the SQLite Local Registry as the single canonical store; `.ommx` archives become a pure exchange format. Breaking changes on {class}`~ommx.artifact.ArtifactBuilder` and {class}`~ommx.artifact.Artifact` need migration:

- {func}`ArtifactBuilder.new_archive <ommx.artifact.ArtifactBuilder.new_archive>` → {func}`ArtifactBuilder.new <ommx.artifact.ArtifactBuilder.new>` + {func}`Artifact.save <ommx.artifact.Artifact.save>` (new method).
- {func}`ArtifactBuilder.new_archive_unnamed <ommx.artifact.ArtifactBuilder.new_archive_unnamed>` → {func}`ArtifactBuilder.new_anonymous <ommx.artifact.ArtifactBuilder.new_anonymous>` + `Artifact.save(path)`. Anonymous artifacts now carry a synthesized `<registry-id8>.ommx.local/anonymous:<timestamp>-<nonce>` image name instead of `None`.
- {func}`Artifact.load_archive <ommx.artifact.Artifact.load_archive>` raises a migration error pointing at the two replacement methods: {func}`Artifact.import_archive <ommx.artifact.Artifact.import_archive>` (imports the archive into the user's persistent SQLite Local Registry — the v3 successor with registry-write semantics) and {func}`Artifact.inspect_archive <ommx.artifact.Artifact.inspect_archive>` (side-effect-free read of the manifest + layer descriptors, returns a new {class}`ArchiveManifest <ommx.artifact.ArchiveManifest>` view). v2's `load_archive` opened archives in place with no registry side effect, so the rename makes the semantic shift explicit instead of silently writing into the registry on upgrade. `import_archive` accepts v2 archives produced by `ArtifactBuilder.new_archive_unnamed` (no `org.opencontainers.image.ref.name` annotation) by synthesizing an anonymous name on the fly; `inspect_archive` reads such archives back with `ArchiveManifest.image_name = None` (no registry context for synthesis).
- CLI `ommx push <archive>` and `ommx push <oci-dir>` removed — load into the registry first, then push by image name.
- New CLI `ommx artifact prune-anonymous [--dry-run]` bulk-cleans accumulated anonymous-build entries.
- `ommx.get_image_dir(...)` and the CLI `ommx image-dir <name>` subcommand are removed. The return value was a v2 disk-cache path (`<root>/<image_name>/<tag>/`) that no longer corresponds to any v3 storage location — the SQLite Local Registry stores blobs content-addressed and refs in SQLite — so pointing users at it was actively misleading. Existing v2 caches still migrate via `ommx artifact import`.

See the [Python SDK v2→v3 Migration Guide §13](https://github.com/Jij-Inc/ommx/blob/main/PYTHON_SDK_MIGRATION_GUIDE.md#13-artifact-api-archive-becomes-an-exchange-format) for the full before/after code and migration checklist.

## 3.0.0 Alpha 3

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_3.0.0a3-orange?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-3.0.0a3)

See the GitHub Release above for full details. The following summarizes the main changes. This is a pre-release version. APIs may change before the final release.

### ⚠ `*_df` accessors are methods + `include=` filter + sidecar DataFrames ([#846](https://github.com/Jij-Inc/ommx/pull/846))

Every `*_df` accessor on `Instance` / `ParametricInstance` / `Solution` / `SampleSet` is now a regular method instead of a `#[getter]` property. Existing call sites need parentheses:

```python
# Before
df = solution.constraints_df

# After
df = solution.constraints_df()
```

The wide `*_df` methods take an `include` argument that gates the metadata / parameters column families. The default `include=("metadata", "parameters")` preserves the v2-equivalent wide shape:

```python
solution.decision_variables_df()                       # core + metadata + parameters
solution.decision_variables_df(include=[])             # core only
solution.decision_variables_df(include=["metadata"])   # core + metadata
solution.decision_variables_df(include=["parameters"]) # core + parameters
```

Six new long-format / id-indexed sidecar accessors read directly from the SoA metadata stores. `kind=` selects the constraint family (`"regular"` / `"indicator"` / `"one_hot"` / `"sos1"`, default `"regular"`):

- `constraint_metadata_df(kind=...)` — id-indexed (`name` / `subscripts` / `description`)
- `constraint_parameters_df(kind=...)` — long format (`{kind}_constraint_id` / `key` / `value`)
- `constraint_provenance_df(kind=...)` — long format (`{kind}_constraint_id` / `step` / `source_kind` / `source_id`)
- `constraint_removed_reasons_df(kind=...)` — long format (`{kind}_constraint_id` / `reason` / `key` / `value`)
- `variable_metadata_df()` — id-indexed
- `variable_parameters_df()` — long format

Sidecar index names are kind-qualified (`regular_constraint_id` / `indicator_constraint_id` / `one_hot_constraint_id` / `sos1_constraint_id` / `variable_id`) so accidental cross-id-space `df.join()` mistakes surface in `df.head()` and friends. Long-format `*_parameters_df` / `*_removed_reasons_df` rows are sorted by `(id, key)`, and empty long-format DataFrames keep their column schema instead of returning a column-less frame.

### ⚠ `removed_reason` column gated by `include=` ([#796](https://github.com/Jij-Inc/ommx/pull/796), [#847](https://github.com/Jij-Inc/ommx/pull/847))

In v2.5.1 {meth}`Solution.constraints_df <ommx.v1.Solution.constraints_df>` carried a `removed_reason` column unconditionally. The initial `include=` gate of that column landed in 3.0.0a2 (#796), and 3.0.0a3 finalizes it into the `kind=` / `include=` / `removed=` dispatch shape documented above (#847): the column is opted in by `"removed_reason"` in `include=` (a unit flag that controls both the reason name and `removed_reason.{key}` parameter columns). Rows whose constraint was not removed before evaluation get NA in those columns.

```python
# Before (2.5.1)
df = solution.constraints_df  # contains a 'removed_reason' column

# After (3.0.0a3 — `*_df` are now methods)
df = solution.constraints_df()  # no removed_reason column
df = solution.constraints_df(include=("metadata", "parameters", "removed_reason"))
# ↳ adds removed_reason / removed_reason.{key} (NA for active rows)
```

The same `kind=` / `include=` shape applies on {class}`~ommx.v1.SampleSet`. On {class}`~ommx.v1.Instance` and {class}`~ommx.v1.ParametricInstance`, `removed=True` returns active + removed rows in one DataFrame and auto-sets `"removed_reason"` so removed rows are distinguishable.

### ⚠ `to_bytes` / `from_bytes` removed from non-top-level types ([#845](https://github.com/Jij-Inc/ommx/pull/845))

Bytes serialization is removed from the following component-level types:

- {class}`~ommx.v1.Function`, {class}`~ommx.v1.Linear`, {class}`~ommx.v1.Quadratic`, {class}`~ommx.v1.Polynomial`
- {class}`~ommx.v1.Parameter`
- {class}`~ommx.v1.NamedFunction`, {class}`~ommx.v1.EvaluatedNamedFunction`, {class}`~ommx.v1.SampledNamedFunction`
- {class}`~ommx.v1.DecisionVariable`, {class}`~ommx.v1.EvaluatedDecisionVariable`, {class}`~ommx.v1.SampledDecisionVariable`

These methods originally existed to ferry values across the Python ↔ Rust boundary back when the Python SDK had its own protobuf-based wrapper layer and had to serialize on every hop. With the v3 transition to direct PyO3 re-exports the boundary disappears, so element-level bytes round-trips no longer serve a purpose, and keeping them aligned with the upcoming metadata-storage redesign would only add maintenance cost. `to_bytes` / `from_bytes` remain available on the container types ({class}`~ommx.v1.Instance`, {class}`~ommx.v1.ParametricInstance`, {class}`~ommx.v1.Solution`, {class}`~ommx.v1.SampleSet`) and on the cross-evaluate DTOs ({class}`~ommx.v1.State`, {class}`~ommx.v1.Samples`, {class}`~ommx.v1.Parameters`) — use those when you need to persist or exchange data on disk or over the wire.

### 🆕 Write-through metadata wrappers: `AttachedConstraint` / `AttachedDecisionVariable` ([#849](https://github.com/Jij-Inc/ommx/pull/849), [#850](https://github.com/Jij-Inc/ommx/pull/850), [#852](https://github.com/Jij-Inc/ommx/pull/852))

`Instance.add_constraint` / `instance.constraints[id]` and the matching accessors on `ParametricInstance` now return write-through handles bound to the parent host instead of snapshot copies. Reads pull live data from the host and metadata setters write straight to its SoA metadata store, so two handles pointing at the same id observe the same state.

```python
c = instance.add_constraint(x + y == 0)         # AttachedConstraint
c.set_name("budget")                             # writes through to instance
assert instance.constraints[c.constraint_id].name == "budget"
```

Five write-through types ship: {class}`~ommx.v1.AttachedConstraint`, {class}`~ommx.v1.AttachedIndicatorConstraint`, {class}`~ommx.v1.AttachedOneHotConstraint`, {class}`~ommx.v1.AttachedSos1Constraint`, and {class}`~ommx.v1.AttachedDecisionVariable`. {class}`~ommx.v1.Constraint` and {class}`~ommx.v1.DecisionVariable` are unchanged in shape — they remain the snapshot wrappers used for modeling input (operator overloading, `Instance.from_components`). Each `AttachedX` exposes `.detach()` to obtain an equivalent snapshot when you need to break the back-reference to the host.

As part of the same change, `instance.decision_variables` now returns `list[AttachedDecisionVariable]` (previously `list[DecisionVariable]` snapshots), aligning with `instance.constraints` and the special-constraint accessors.

### 🆕 OpenTelemetry-based tracing and profiling ([#816](https://github.com/Jij-Inc/ommx/pull/816), [#823](https://github.com/Jij-Inc/ommx/pull/823), [#826](https://github.com/Jij-Inc/ommx/pull/826), [#828](https://github.com/Jij-Inc/ommx/pull/828), [#829](https://github.com/Jij-Inc/ommx/pull/829))

The legacy `log` + `pyo3-log` → Python `logging` bridge is replaced by a `tracing` + `pyo3-tracing-opentelemetry` pipeline, so the Rust core's spans can now be consumed through the Python OTel SDK.

Two entry points ship under `ommx.tracing`:

- **`%%ommx_trace`** — a Jupyter cell magic that renders a per-cell span tree and a Chrome Trace JSON download link
- **`capture_trace` / `@traced`** — a context manager and decorator for the same workflow from regular Python scripts, tests, and CI

See [Tracing and Profiling](../user_guide/tracing.md) for the full walkthrough, configuring your own `TracerProvider`, and troubleshooting.

### 🆕 Tracing spans in solver/sampler adapters ([#833](https://github.com/Jij-Inc/ommx/pull/833))

Every OMMX adapter now emits three OpenTelemetry spans per solve/sample call, so the OTel tracing pipeline above can attribute wall-clock time to the three phases an adapter actually spends time in:

- **`convert`** — OMMX `Instance` → solver-native problem translation
- **`solve`** / **`sample`** — the call into the underlying solver / sampler itself
- **`decode`** — decoding the solver's response back to `Solution` / `SampleSet` (Rust-side `evaluate` spans nest underneath)

Each adapter uses its own tracer name, so runs from different solvers are easy to distinguish in the tree view:

| Adapter | Tracer | Spans |
|---|---|---|
| `ommx-pyscipopt-adapter` | `ommx.adapter.pyscipopt` | `convert` / `solve` / `decode` |
| `ommx-highs-adapter` | `ommx.adapter.highs` | `convert` / `solve` / `decode` |
| `ommx-python-mip-adapter` | `ommx.adapter.python_mip` | `convert` / `solve` / `decode` |
| `ommx-openjij-adapter` | `ommx.adapter.openjij` | `convert` / `sample` / `decode` |

```python
from ommx.tracing import capture_trace
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

with capture_trace() as trace:
    solution = OMMXPySCIPOptAdapter.solve(instance)

print(trace.text_tree())  # shows convert / solve / decode with durations
```

Spans are emitted through the standard OpenTelemetry API, so they are a no-op when no `TracerProvider` is installed — there is no runtime cost for users who do not opt in.

### 🆕 `Function.evaluate_bound` is now available from Python ([#831](https://github.com/Jij-Inc/ommx/pull/831))

{meth}`Function.evaluate_bound <ommx.v1.Function.evaluate_bound>` is now exposed on {class}`~ommx.v1.Function`. Given per-variable bounds, it returns a {class}`~ommx.v1.Bound` that contains the range of the function value — useful when deriving feasibility bounds or doing simple presolve on the Python side.

```python
from ommx.v1 import Function, Linear, Bound

f = Function(Linear(terms={1: 2}, constant=3))  # 2*x1 + 3
b = f.evaluate_bound({1: Bound(0.0, 2.0)})
# b.lower == 3.0, b.upper == 7.0
```

The bound is computed monomial-wise and summed, so it is a sound over-approximation of the true range but is **not guaranteed to be tight** when multiple terms share variables (the classic dependency problem in interval arithmetic). Variable IDs missing from `bounds` are treated as unbounded.

## 3.0.0 Alpha 2

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_3.0.0a2-orange?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-3.0.0a2)

See the GitHub Release above for full details. The following summarizes the main changes. This is a pre-release version. APIs may change before the final release.

### ⚠ Removal of the `Constraint.id` field ([#806](https://github.com/Jij-Inc/ommx/pull/806))

The `id` field (along with the `.id` getter, `set_id()`, and `id=` constructor argument) is removed from {class}`~ommx.v1.Constraint` and its variants ({class}`~ommx.v1.IndicatorConstraint` / {class}`~ommx.v1.OneHotConstraint` / {class}`~ommx.v1.Sos1Constraint` / {class}`~ommx.v1.EvaluatedConstraint` / {class}`~ommx.v1.SampledConstraint` / {class}`~ommx.v1.RemovedConstraint`). A constraint's ID now exists only as the key of the `dict[int, Constraint]` passed to {meth}`Instance.from_components <ommx.v1.Instance.from_components>`.

```python
# Before (2.5.1)
c = Constraint(function=x + y, equality=Constraint.EQUAL_TO_ZERO, id=5)
Instance.from_components(..., constraints=[c], ...)

# After (3.0.0a2)
c = Constraint(function=x + y, equality=Constraint.EQUAL_TO_ZERO)
Instance.from_components(..., constraints={5: c}, ...)
```

Global ID counters (`next_constraint_id` and friends) and per-constraint `to_bytes` / `from_bytes` are also removed. For full details and migration steps, see the [Python SDK v2 to v3 Migration Guide](https://github.com/Jij-Inc/ommx/blob/main/PYTHON_SDK_MIGRATION_GUIDE.md).

### 🆕 First-class special constraint types ([#789](https://github.com/Jij-Inc/ommx/pull/789), [#790](https://github.com/Jij-Inc/ommx/pull/790), [#795](https://github.com/Jij-Inc/ommx/pull/795), [#796](https://github.com/Jij-Inc/ommx/pull/796), [#798](https://github.com/Jij-Inc/ommx/pull/798))

In addition to regular constraints, the following three special constraint types are now first-class citizens — they can be passed to `Instance.from_components` via `indicator_constraints=` / `one_hot_constraints=` / `sos1_constraints=`, and read back through {meth}`~ommx.v1.Solution.constraints_df` / {meth}`~ommx.v1.SampleSet.constraints_df` with `kind=` selecting the family.

- {class}`~ommx.v1.IndicatorConstraint` — conditional constraint on a binary variable (new)
- {class}`~ommx.v1.OneHotConstraint` — replaces the previous `ConstraintHints.OneHot` metadata
- {class}`~ommx.v1.Sos1Constraint` — replaces the previous `ConstraintHints.Sos1` metadata

For concrete usage, evaluation-result access, and the Indicator relax / restore workflow, see [Special Constraints](../user_guide/special_constraints.md).

Accordingly, the legacy `ConstraintHints` / `OneHot` / `Sos1` classes, the `Instance.constraint_hints` property, and the PySCIPOpt Adapter's `use_sos1` flag are removed.

### 🆕 Adapter Capability Model ([#790](https://github.com/Jij-Inc/ommx/pull/790), [#805](https://github.com/Jij-Inc/ommx/pull/805), [#810](https://github.com/Jij-Inc/ommx/pull/810), [#811](https://github.com/Jij-Inc/ommx/pull/811), [#814](https://github.com/Jij-Inc/ommx/pull/814))

Alongside the special constraint types, adapters now declare their own supported capabilities via an `ADDITIONAL_CAPABILITIES` class attribute. When `super().__init__(instance)` is called, any undeclared special constraint is automatically converted to regular constraints (Big-M for Indicator / SOS1, linear equality for OneHot) before the instance reaches the solver.

**Existing OMMX Adapters must be updated for Python SDK 3.0.0 to call `super().__init__(instance)`.** Currently the PySCIPOpt Adapter declares support for Indicator and SOS1.

For details and the manual conversion APIs, see [Adapter Capability Model and Conversions](../user_guide/capability_model.md).

### 🔄 numpy scalar support ([#794](https://github.com/Jij-Inc/ommx/pull/794))

The {class}`~ommx.v1.Function` constructor now accepts `numpy.integer` and `numpy.floating` values. In v2.5.1, `Function(numpy.int64(3))` raised `TypeError`.

## 3.0.0 Alpha 1

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_3.0.0a1-orange?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-3.0.0a1)

See the GitHub Release above for full details. The following summarizes the main changes. This is a pre-release version. APIs may change before the final release.

### Complete Rust re-export of `ommx.v1` and `ommx.artifact` types ([#770](https://github.com/Jij-Inc/ommx/pull/770), [#771](https://github.com/Jij-Inc/ommx/pull/771), [#774](https://github.com/Jij-Inc/ommx/pull/774), [#775](https://github.com/Jij-Inc/ommx/pull/775), [#782](https://github.com/Jij-Inc/ommx/pull/782))

Python SDK 3.0.0 is fully based on Rust/PyO3.
In 2.0.0, the core implementation was rewritten in Rust while Python wrapper classes remained for compatibility. In 3.0.0, those Python wrappers are removed entirely — all types in `ommx.v1` and `ommx.artifact` are now direct re-exports from Rust, and the `protobuf` Python runtime dependency is eliminated. The `.raw` attribute that previously provided access to the underlying PyO3 implementation has also been removed.

### Migration to Sphinx and ReadTheDocs hosting ([#780](https://github.com/Jij-Inc/ommx/pull/780), [#785](https://github.com/Jij-Inc/ommx/pull/785))

In v2, the Sphinx-based API Reference and Jupyter Book-based documentation were each hosted on [GitHub Pages](https://jij-inc.github.io/ommx/en/introduction.html). In v3, documentation has been fully migrated to Sphinx and is now hosted on [ReadTheDocs](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/). GitHub Pages will continue to host the documentation as of v2.5.1, but all future updates will be on ReadTheDocs only.
