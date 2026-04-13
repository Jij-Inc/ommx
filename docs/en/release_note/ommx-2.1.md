# OMMX Python SDK 2.1.x

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.1.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.1.0)

Released on 2025-11-13.

## Breaking Changes

### Drop Python 3.9, add Python 3.10–3.14 support ([#669](https://github.com/Jij-Inc/ommx/pull/669))

Python 3.9 has reached end-of-life. This release drops support for Python 3.9 and upgrades the PyO3 ABI3 baseline from `py39` to `py310`. Wheels are now built for Python 3.10 (ABI3), 3.13t, and 3.14t (free-threaded).

## New Features

### Optional `atol` parameter for evaluate methods ([#666](https://github.com/Jij-Inc/ommx/pull/666))

All evaluate methods (`Instance.evaluate`, `Function.evaluate`, `Constraint.evaluate`, etc.) now accept an optional keyword-only `atol` parameter to specify a custom absolute tolerance for feasibility checks. The default remains `1e-6`.

### `decision_variable_names` and `extract_all_decision_variables` ([#667](https://github.com/Jij-Inc/ommx/pull/667))

- `decision_variable_names` property is added to `Instance`, `Solution`, and `SampleSet`, returning the set of all decision variable names.
- `extract_all_decision_variables()` method returns a dictionary mapping variable names to their subscript-value mappings, complementing the existing `extract_decision_variables(name)` method.

### `DecisionVariableAnalysis` Display and serialization ([#668](https://github.com/Jij-Inc/ommx/pull/668))

`DecisionVariableAnalysis`, which provides kind/usage-based partitioning of decision variables (e.g. identifying dependent variables created through `substitute_acyclic`), now supports `to_dict()` and `__repr__()` in Python, and `Display` trait and `Serialize`/`Deserialize` in Rust.
