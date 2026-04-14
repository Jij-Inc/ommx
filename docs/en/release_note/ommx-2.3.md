# OMMX Python SDK 2.3.x

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.3.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.3.0)

Released on 2025-11-18.

## New Features

### Pyodide (WebAssembly) support ([#679](https://github.com/Jij-Inc/ommx/pull/679))

OMMX can now run in the browser via [Pyodide](https://pyodide.org/). Network-dependent features (OCI artifact push/pull) are gated behind a `remote-artifact` feature flag, allowing the core SDK to compile to `wasm32-unknown-emscripten`.

### Constraint violation calculation ([#680](https://github.com/Jij-Inc/ommx/pull/680))

New methods for quantifying constraint violations in solutions:

- `EvaluatedConstraint.violation` — returns the violation amount for a single constraint (0 if feasible).
- `Solution.total_violation_l1()` — sum of all constraint violations (L1 norm).
- `Solution.total_violation_l2()` — root sum of squares of all constraint violations (L2 norm).

These are useful for analyzing near-feasible solutions and implementing penalty-based methods.

## New Features (2.3.1–2.3.6)

### `NoSolutionObtained` exception (2.3.1, [#688](https://github.com/Jij-Inc/ommx/pull/688))

A new `ommx.adapter.NoSolutionObtained` exception distinguishes the case where a solver times out without finding any feasible solution from `InfeasibleDetected` or `UnboundedDetected`. The PySCIPOpt and Python-MIP adapters have been updated to raise the appropriate exception type.

### Logical Memory Profiler (2.3.1, [#683](https://github.com/Jij-Inc/ommx/pull/683))

A logical memory profiling system that outputs flamegraph-compatible folded-stack format, covering 13+ OMMX types. Accessible from Python via `instance.logical_memory_profile()`. Useful for understanding the memory footprint of large-scale instances.

### `log_encode` scoped to `used_decision_variables` (2.3.3, [#696](https://github.com/Jij-Inc/ommx/pull/696))

`log_encode` now creates variables only for decision variables actually referenced in the objective or constraints. This avoids duplicate variable creation on repeated calls and reduces overhead for instances with many unused variables.

### Other new features

- (2.3.2) Quantum adapters documentation ([#690](https://github.com/Jij-Inc/ommx/pull/690))
- (2.3.4) OMMXOpenJijAdapter supports Python 3.13 ([#704](https://github.com/Jij-Inc/ommx/pull/704))

## Bug Fixes (2.3.1–2.3.6)

### `Function().terms` with non-zero constants (2.3.5, [#714](https://github.com/Jij-Inc/ommx/pull/714))

`Function.terms` was returning a raw `float` instead of a proper dict entry when the function had a non-zero constant term.

## Others (2.3.1–2.3.6)

- (2.3.2) Update PySCIPOpt dependency ([#691](https://github.com/Jij-Inc/ommx/pull/691))
- (2.3.6) Migrate Pyodide build from maturin to cibuildwheel ([#708](https://github.com/Jij-Inc/ommx/pull/708))
- (2.3.6) Use Python MIP 1.17 ([#724](https://github.com/Jij-Inc/ommx/pull/724))
- (2.3.6) Add weekly Python dependency update workflow ([#728](https://github.com/Jij-Inc/ommx/pull/728))
