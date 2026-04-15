# OMMX Python SDK 2.3.x

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.3.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.3.0)
[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.3.1-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.3.1)
[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.3.2-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.3.2)
[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.3.3-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.3.3)
[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.3.4-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.3.4)
[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.3.5-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.3.5)
[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.3.6-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.3.6)

See the GitHub Releases above for full details. The following summarizes the main changes.

## New Features

### Pyodide (WebAssembly) support (2.3.0, [#679](https://github.com/Jij-Inc/ommx/pull/679))

OMMX can now run in the browser via [Pyodide](https://pyodide.org/). However, network-dependent features (OCI artifact push/pull) are not available. Starting from [2.3.6](https://github.com/Jij-Inc/ommx/releases/tag/python-2.3.6), Pyodide builds are provided in GitHub Releases.

### Constraint violation calculation (2.3.0, [#680](https://github.com/Jij-Inc/ommx/pull/680))

New methods for quantifying constraint violations in solutions:

- `EvaluatedConstraint.violation` — returns the violation amount for a single constraint (0 if feasible).
- `Solution.total_violation_l1()` — sum of all constraint violations (L1 norm).
- `Solution.total_violation_l2()` — root sum of squares of all constraint violations (L2 norm).

Useful for analyzing infeasible solutions and implementing penalty-based methods.

### `NoSolutionObtained` exception (2.3.1, [#688](https://github.com/Jij-Inc/ommx/pull/688))

A new `ommx.adapter.NoSolutionObtained` exception distinguishes the case where a solver times out without finding any feasible solution from `InfeasibleDetected` or `UnboundedDetected`. The PySCIPOpt and Python-MIP adapters have been updated to raise the appropriate exception type.

### Logical Memory Profiler (2.3.1, [#683](https://github.com/Jij-Inc/ommx/pull/683))

A logical memory profiling system that outputs flamegraph-compatible folded-stack format. Accessible from Python via `instance.logical_memory_profile()`. Useful for understanding the memory footprint of large-scale instances.

### `log_encode` scoped to `used_decision_variables` (2.3.3, [#696](https://github.com/Jij-Inc/ommx/pull/696))

`log_encode` now creates variables only for decision variables actually referenced in the objective or constraints. This avoids duplicate variable creation on repeated calls and reduces overhead for instances with many unused variables.

## Bug Fixes

### `Function().terms` with non-zero constants (2.3.5, [#714](https://github.com/Jij-Inc/ommx/pull/714))

`Function.terms` was returning a raw `float` instead of a proper dict entry when the function had a non-zero constant term.
