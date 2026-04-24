# OMMX Python SDK 2.4.x

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.4.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.4.0)

See the GitHub Release above for full details. The following summarizes the main changes.

## Breaking Changes

### Allow `removed_constraint` to contain fixed/dependent variables ([#738](https://github.com/Jij-Inc/ommx/pull/738))

Previously, `removed_constraint` was implicitly assumed not to reference fixed or dependent variable IDs. This release lifts that restriction — `removed_constraint` may now contain such variables. Accordingly, `partial_evaluate` now skips `removed_constraint`, preventing performance degradation from unused constraints. These constraints are partially evaluated when restored via `restore_constraint`.

## Bug Fixes

### Clear constraint hints in penalty methods ([#739](https://github.com/Jij-Inc/ommx/pull/739))

`Instance.penalty_method` and `Instance.uniform_penalty_method` now correctly clear constraint hints when moving constraints to `removed_constraints`. Previously, stale hints could reference constraints that no longer existed as active constraints.

### Reduce constraint hint log level ([#740](https://github.com/Jij-Inc/ommx/pull/740))

The log message emitted when constraint hints are discarded has been changed from `warn` to `debug` to reduce noise in normal usage.
