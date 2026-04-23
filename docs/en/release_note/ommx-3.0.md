# OMMX Python SDK 3.0.x

```{note}
Python SDK 3.0.0 contains breaking API changes. A migration guide is available in the [Python SDK v2 to v3 Migration Guide](https://github.com/Jij-Inc/ommx/blob/main/PYTHON_SDK_MIGRATION_GUIDE.md).
```

## Unreleased

### 🆕 OpenTelemetry-based tracing and profiling ([#816](https://github.com/Jij-Inc/ommx/pull/816), [#823](https://github.com/Jij-Inc/ommx/pull/823), [#826](https://github.com/Jij-Inc/ommx/pull/826), [#828](https://github.com/Jij-Inc/ommx/pull/828), [#829](https://github.com/Jij-Inc/ommx/pull/829))

The legacy `log` + `pyo3-log` → Python `logging` bridge is replaced by a `tracing` + `pyo3-tracing-opentelemetry` pipeline, so the Rust core's spans can now be consumed through the Python OTel SDK.

Two entry points ship under `ommx.tracing`:

- **`%%ommx_trace`** — a Jupyter cell magic that renders a per-cell span tree and a Chrome Trace JSON download link
- **`capture_trace` / `@traced`** — a context manager and decorator for the same workflow from regular Python scripts, tests, and CI

See [Tracing and Profiling](../user_guide/tracing.md) for the full walkthrough, configuring your own `TracerProvider`, and troubleshooting.

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

In addition to regular constraints, the following three special constraint types are now first-class citizens — they can be passed to `Instance.from_components` via `indicator_constraints=` / `one_hot_constraints=` / `sos1_constraints=`, and corresponding `*_constraints_df` DataFrames are available on {class}`~ommx.v1.Solution` / {class}`~ommx.v1.SampleSet`.

- {class}`~ommx.v1.IndicatorConstraint` — conditional constraint on a binary variable (new)
- {class}`~ommx.v1.OneHotConstraint` — replaces the previous `ConstraintHints.OneHot` metadata
- {class}`~ommx.v1.Sos1Constraint` — replaces the previous `ConstraintHints.Sos1` metadata

For concrete usage, evaluation-result access, and the Indicator relax / restore workflow, see [Special Constraints](../user_guide/special_constraints.md).

Accordingly, the legacy `ConstraintHints` / `OneHot` / `Sos1` classes, the `Instance.constraint_hints` property, and the PySCIPOpt Adapter's `use_sos1` flag are removed.

### ⚠ `removed_reason` column split into a separate table ([#796](https://github.com/Jij-Inc/ommx/pull/796))

In v2.5.1 {attr}`Solution.constraints_df <ommx.v1.Solution.constraints_df>` carried a `removed_reason` column. In v3.0.0a2 that column is split out into a separate {attr}`Solution.removed_reasons_df <ommx.v1.Solution.removed_reasons_df>` table, which you can join on if you need the previous shape. The same change applies to {class}`~ommx.v1.SampleSet`.

```python
# Before (2.5.1)
df = solution.constraints_df  # contains a 'removed_reason' column

# After (3.0.0a2)
df = solution.constraints_df.join(solution.removed_reasons_df)
```

Corresponding `*_removed_reasons_df` accessors are also provided for Indicator, OneHot, and SOS1.

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
