# OMMX Python SDK 2.5.x

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.5.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.5.0)
[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.5.1-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.5.1)
[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.5.2-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.5.2)

See the GitHub Releases above for full details. The following summarizes the main changes.

## New Features

### `NamedFunction` (2.5.0, [#748](https://github.com/Jij-Inc/ommx/pull/748))

A new `NamedFunction` message and corresponding Python class have been introduced for tracking auxiliary functions (costs, penalties, KPIs, etc.) alongside optimization problems. Related types `EvaluatedNamedFunction` and `SampledNamedFunction` are also added.

Named functions can be attached to `Instance`, and are automatically evaluated when calling `Instance.evaluate`, with results stored in `Solution`. They integrate with the pandas `DataFrame` export via `Solution.named_functions_df`.

This feature is useful for:
- Tracking multiple objective components (e.g. cost vs. penalty breakdowns)
- Recording domain-specific metrics alongside solutions
- Comparing auxiliary quantities across different solver runs

### Format version field for forward compatibility (2.5.2, [#835](https://github.com/Jij-Inc/ommx/pull/835))

A `format_version` field has been added to the four top-level OMMX exchange messages: `Instance`, `Solution`, `SampleSet`, and `ParametricInstance`. This prevents silent misinterpretation of future data with semantic-breaking format changes — for example, in Python SDK v3 `OneHot` and `SOS1` are promoted to special constraints and will no longer be represented as v2 `ConstraintHints`.

This is the v2 maintenance release that must ship before v3, so that users upgrading to v3-produced data get a clear error rather than a silently-wrong parse.

Policy summary:

- `ommx.v1` backward compatibility is unchanged — data produced by older SDKs is always readable by newer SDKs.
- Non-semantic-breaking proto additions continue to rely on protobuf's standard forward compatibility (unknown fields are ignored).
- Semantic-breaking format changes bump `format_version`. This *may* only happen on an SDK major version update.
  In that case, data produced by the newer SDK cannot be read by older SDKs, which will raise an error prompting an SDK upgrade.

## Bug Fixes

### `extract_decision_variables` ignores parameters (2.5.0, [#745](https://github.com/Jij-Inc/ommx/pull/745))

`extract_decision_variables` now ignores parameters and uses only subscripts for variable identification. Previously, variables with the same subscripts but different parameters would cause extraction failures. This is a fix for practical use cases where parameters vary across problem instances but subscripts remain stable.

### Dependent variable evaluation order (2.5.1, [#753](https://github.com/Jij-Inc/ommx/pull/753))

Dependent variables were evaluated in ID order, which fails when a lower-ID variable depends on a higher-ID one. Fixed by evaluating in topological order.
