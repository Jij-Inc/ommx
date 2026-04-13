# OMMX Python SDK 2.3.0

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
