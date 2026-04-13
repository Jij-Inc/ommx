# OMMX Python SDK 2.5.x

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.5.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.5.0)

Released on 2026-03-19.

## New Features

### `NamedFunction` ([#748](https://github.com/Jij-Inc/ommx/pull/748))

A new `NamedFunction` message and corresponding Python class have been introduced for tracking auxiliary functions (costs, penalties, KPIs, etc.) alongside optimization problems. Related types `EvaluatedNamedFunction` and `SampledNamedFunction` are also added.

Named functions can be attached to `Instance`, and are automatically evaluated when calling `Instance.evaluate`, with results stored in `Solution`. They integrate with the pandas `DataFrame` export via `Solution.named_functions_df`.

This feature is useful for:
- Tracking multiple objective components (e.g. cost vs. penalty breakdowns)
- Recording domain-specific metrics alongside solutions
- Comparing auxiliary quantities across different solver runs

### Bug fix: `extract_decision_variables` ignores parameters ([#745](https://github.com/Jij-Inc/ommx/pull/745))

`extract_decision_variables` now ignores parameters and uses only subscripts for variable identification. Previously, variables with the same subscripts but different parameters would cause extraction failures. This is a fix for practical use cases where parameters vary across problem instances but subscripts remain stable.

## Patch Releases

### 2.5.1

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.5.1-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.5.1)

- Fix: Topological sort for dependent variable evaluation ([#753](https://github.com/Jij-Inc/ommx/pull/753))
