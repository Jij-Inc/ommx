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

## Patch Releases

### 2.3.1

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.3.1-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.3.1)

- Refine timelimit exceptions in adapters ([#688](https://github.com/Jij-Inc/ommx/pull/688))
- Logical Memory Profiler ([#683](https://github.com/Jij-Inc/ommx/pull/683))

### 2.3.2

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.3.2-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.3.2)

- Update PySCIPOpt dependency ([#691](https://github.com/Jij-Inc/ommx/pull/691))
- Add quantum adapters ([#690](https://github.com/Jij-Inc/ommx/pull/690))

### 2.3.3

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.3.3-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.3.3)

- Use only `used_decision_variables` in `log_encode` ([#696](https://github.com/Jij-Inc/ommx/pull/696))

### 2.3.4

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.3.4-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.3.4)

- Update timelimit tests ([#695](https://github.com/Jij-Inc/ommx/pull/695))
- OMMXOpenJijAdapter supports Python 3.13 ([#704](https://github.com/Jij-Inc/ommx/pull/704))

### 2.3.5

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.3.5-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.3.5)

- Fix: `Function().terms` method in the case of non-zero constants ([#714](https://github.com/Jij-Inc/ommx/pull/714))

### 2.3.6

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.3.6-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.3.6)

- Migrate Pyodide build from maturin to cibuildwheel ([#708](https://github.com/Jij-Inc/ommx/pull/708))
- Use Python MIP 1.17 ([#724](https://github.com/Jij-Inc/ommx/pull/724))
- Add weekly Python dependency update workflow ([#728](https://github.com/Jij-Inc/ommx/pull/728))
