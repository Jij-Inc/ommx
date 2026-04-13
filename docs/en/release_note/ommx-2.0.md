# OMMX Python SDK 2.0.x

This is the first major version release in about a year since the [OMMX Python SDK 1.0.0](https://github.com/Jij-Inc/ommx/releases/tag/python-1.0.0) release on 2024/7/10. This version includes significant performance improvements, API enhancements with breaking changes, and the addition of new features.

```{note}
In OMMX, the SDK version and the data format version are independent. The new SDK can read all existing data.
```

## Performance Improvements

In the initial design of OMMX, the main purpose was to provide a standardized data format, so the model generation API in the SDK was primarily for testing and debugging, and performance was not a major concern. However, as features like QUBO conversion at the OMMX level became available, performance bottlenecks became more frequent.

This version significantly improves the performance of the OMMX API. Since many parts have been improved at the computational complexity order level, a significant performance improvement can be expected, especially for large-scale problems. In particular, improvements have been made in the following areas:

- The implementation of the API, which was auto-generated from Protocol Buffers schema definitions for Python, has been replaced with an implementation based on the Rust SDK. This reduces the overhead of unnecessary serialization and deserialization, speeding up API calls.
- In the Rust SDK as well, the parts that were auto-generated from the schema definition have been re-implemented more naturally in Rust. By using more appropriate data structures, a significant performance improvement has been achieved. In addition, consistency checks, such as the inability to register a polynomial containing variables not registered as decision variables as an objective function, which could not be described in Protocol Buffers, can now be guaranteed at the Rust type level, enabling more efficient and strict checks.
- We have set up an online profiling and continuous benchmarking environment for the Rust and Python SDKs with [CodSpeed](https://codspeed.io/Jij-Inc/ommx). Although we have made significant improvements in this release, there are still many areas that are far from optimal, and we will continue to make improvements in the future.

## API Updates

As mentioned above, in addition to replacing the API that was auto-generated from the Protocol Buffers definition, we have improved the API to be more natural and easier for AI assistants like [GitHub Copilot] and [Claude Code] to generate, in line with their widespread adoption. This time, we are making API improvements that include breaking changes for the major version upgrade.

We have prepared a migration guide specifically for use with [Claude Code] in the [Python SDK v1 to v2 Migration Guide](https://github.com/Jij-Inc/ommx/blob/main/PYTHON_SDK_MIGRATION_GUIDE.md). You can migrate more smoothly by loading this into [Claude Code] before performing the migration. Using type checking with `pyright` or `mypy` will make the migration even smoother.

[GitHub Copilot]: https://github.com/features/copilot
[Claude Code]: https://www.anthropic.com/claude-code
[`ommx.v1.Instance`]: https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance
[`ommx.v1.ParametricInstance`]: https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.ParametricInstance
[`ommx.v1.Solution`]: https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Solution
[`ommx.v1.SampleSet`]: https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.SampleSet
[`DataFrame`]: https://pandas.pydata.org/pandas-docs/stable/reference/frame.html

### Deprecation of the `raw` API

Before 2.0.0, fields like `ommx.v1.Instance.raw` were fields with classes auto-generated from Protocol Buffers, but as mentioned above, this has been replaced with an implementation based on the Rust SDK. We will not maintain compatibility at this layer, and instead, you can now achieve the necessary processing by directly using the [`ommx.v1.Instance`] API. We will phase out the `raw` API in the future.

### Renaming of Function APIs that Return DataFrame

Previously, properties like `Instance.decision_variables` and `Instance.constraints` returned a [`DataFrame`], but these have been renamed to [`Instance.decision_variables_df`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.decision_variables_df) and [`Instance.constraints_df`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.constraints_df) to clarify that they return a [`DataFrame`].

Instead, properties like [`Instance.decision_variables`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.decision_variables) and [`Instance.constraints`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.constraints) now return `list[ommx.v1.DecisionVariable]` and `list[ommx.v1.Constraint]`, respectively. These are sorted by the ID of the decision variable and constraint. These are more natural to handle than returning a [`DataFrame`] when used in regular Python code. To get a decision variable or constraint from its ID, use [`Instance.get_decision_variable_by_id`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.get_decision_variable_by_id) and [`Instance.get_constraint_by_id`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.get_constraint_by_id).

These changes are also applied to classes such as [`ommx.v1.ParametricInstance`], [`ommx.v1.Solution`], and [`ommx.v1.SampleSet`].

## New Features

The main purpose of this release was to finalize the internal structure changes and breaking API changes, but some new features have also been added.

### HUBO (high-order unconstrained binary optimization) support in OpenJij adapter

OpenJij can directly and quickly handle higher-order polynomials of degree 3 or more as objective functions without performing operations such as degree reduction. This can now be handled directly via the OMMX Adapter. In addition, a [`to_hubo`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.to_hubo) method has been added to [`ommx.v1.Instance`], similar to [`to_qubo`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.to_qubo), which automatically performs binary encoding of integer variables and converts inequality constraints to equality constraints.

```{warning}
Originally, `Instance` had a method called `as_pubo_format`, but in 2.0.0 it was renamed to [`as_hubo_format`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.as_hubo_format) and the return value was also changed. PUBO (polynomial unconstrained binary optimization) and HUBO (high-order unconstrained binary optimization) are often used with almost the same meaning to indicate that they can handle higher-order terms of degree 3 or more compared to QUBO (Quadratic Unconstrained Binary Optimization), but the OMMX project has decided to use the name HUBO from now on.
```

### ARM CPU support for Linux

Binary packages (wheels) for Linux aarch64 are now provided. This makes it easier to use OMMX in the following environments:

- Use on Linux VMs such as Docker on macOS
- IaaS using high-performance ARM CPUs such as AWS Graviton and Ampere, and corresponding PaaS
- GitHub Actions `ubuntu-24.04-arm` environment

## New Features (2.0.1–2.0.12)

### Rust-idiomatic `ParametricInstance` (2.0.3, [#566](https://github.com/Jij-Inc/ommx/pull/566))

The Python bindings for `ParametricInstance` were migrated from the Protocol Buffers auto-generated `ommx::v1::ParametricInstance` to a new Rust-native `ommx::ParametricInstance` with stricter validation via the `Parse` trait. Previously valid-but-semantically-invalid instances (e.g., referencing undefined variables) are now rejected at parse time with clear error messages.

### `Instance.used_decision_variables` and `penalty_method` (2.0.3, [#572](https://github.com/Jij-Inc/ommx/pull/572), [#553](https://github.com/Jij-Inc/ommx/pull/553))

`Instance.used_decision_variables` exposes the set of decision variables actually referenced in the objective or constraints. `Instance.insert_constraint` and `Instance.penalty_method` allow adding constraints after construction and converting constrained problems to unconstrained penalty formulations.

### Quadratic objective and constraints in MPS format (2.0.5, [#597](https://github.com/Jij-Inc/ommx/pull/597))

The MPS parser and writer now handle `QUADOBJ` and `QCMATRIX` sections (as used by Gurobi and similar solvers), enabling full roundtrip of quadratic programs through the MPS format.

### Partial evaluate for `ConstraintHints` (2.0.6, [#609](https://github.com/Jij-Inc/ommx/pull/609))

When calling `Instance.partial_evaluate`, `OneHot` and `SOS1` constraint hints now automatically propagate fixed values to dependent variables, iterated to a fixed point. This prevents adapter errors from dangling variable references after partial evaluation.

### Configurable default absolute tolerance (2.0.6, [#610](https://github.com/Jij-Inc/ommx/pull/610))

A global absolute tolerance for feasibility checks can now be configured at runtime via `ommx.set_default_atol()` / `ommx.get_default_atol()`, or via the `OMMX_DEFAULT_ATOL` environment variable. The default remains `1e-6`.

### QPLIB as OMMX Artifact (2.0.9, [#640](https://github.com/Jij-Inc/ommx/pull/640))

453 QPLIB benchmark instances are now packaged as OMMX Artifacts, mirroring the existing MIPLIB2017 support. Access them via `ommx.dataset.qplib(tag)` and `ommx.dataset.qplib_instance_annotations()` in Python.

### `Instance.stats()` (2.0.11, [#652](https://github.com/Jij-Inc/ommx/pull/652))

A new `Instance.stats()` method returns a hierarchical summary of the instance: counts of decision variables by kind (binary/integer/continuous/semi-continuous/semi-integer) and by usage (objective, constraints, fixed, dependent, irrelevant), plus active vs. removed constraint counts.

### Artifact registry functions in Python API (2.0.7–2.0.12, [#622](https://github.com/Jij-Inc/ommx/pull/622), [#623](https://github.com/Jij-Inc/ommx/pull/623), [#625](https://github.com/Jij-Inc/ommx/pull/625), [#662](https://github.com/Jij-Inc/ommx/pull/662))

Local registry management functions (`get_local_registry_root`, `set_local_registry_root`, `get_image_dir`, `get_images`) were incrementally exposed to Python, with aliases added under `ommx.artifact.*` for convenience. All return `pathlib.Path` objects.

### Other new features

- (2.0.1) `substituted_value` property and binary power reduction ([#537](https://github.com/Jij-Inc/ommx/pull/537), [#540](https://github.com/Jij-Inc/ommx/pull/540))
- (2.0.1) Compare `Bound` by value ([#541](https://github.com/Jij-Inc/ommx/pull/541))
- (2.0.2) Direct `from_bytes`/`to_bytes` in Rust SDK ([#549](https://github.com/Jij-Inc/ommx/pull/549))
- (2.0.2) `PartialOrd<u32>` for `Degree` ([#550](https://github.com/Jij-Inc/ommx/pull/550))
- (2.0.3) QPLIB parser updates ([#575](https://github.com/Jij-Inc/ommx/pull/575))
- (2.0.6) Split `constraint_hints` submodule ([#608](https://github.com/Jij-Inc/ommx/pull/608))
- (2.0.11) ID allocation methods ([#650](https://github.com/Jij-Inc/ommx/pull/650))

## Bug Fixes (2.0.1–2.0.12)

### OpenJij inverted sign in objective for maximization (2.0.5, [#600](https://github.com/Jij-Inc/ommx/pull/600))

When solving maximization problems, the OpenJij adapter negated the objective to convert to a minimization problem but did not flip it back before recording the solution, causing objective values to have inverted signs.

### `ConstraintHints` not restored after relaxing constraints (2.0.2, [#551](https://github.com/Jij-Inc/ommx/pull/551))

Relaxing and then un-relaxing constraints could leave `ConstraintHints` in an inconsistent state.

### Other bug fixes

- (2.0.1) `Instance.decision_variables_df` missing `substituted_value` column ([#542](https://github.com/Jij-Inc/ommx/pull/542))
- (2.0.3) MPS I/O for `Instance`, handle `f64::INFINITY` in `Bound` ([#562](https://github.com/Jij-Inc/ommx/pull/562), [#577](https://github.com/Jij-Inc/ommx/pull/577))
- (2.0.4) Normalize `-0.0` to `0.0` in `Bound` ([#581](https://github.com/Jij-Inc/ommx/pull/581))
- (2.0.9) Fix `to_qubo`/`to_hubo` docstrings ([#631](https://github.com/Jij-Inc/ommx/pull/631))
- (2.0.10) QPLIB annotation key format inconsistency ([#648](https://github.com/Jij-Inc/ommx/pull/648))
