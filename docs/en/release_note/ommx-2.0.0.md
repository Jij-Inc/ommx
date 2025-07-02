# OMMX Python SDK 2.0.0

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
