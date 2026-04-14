# OMMX Python SDK 2.4.x

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.4.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.4.0)

Released on 2026-03-11.

## Breaking Changes

### Allow removed constraints to contain fixed/dependent variables ([#738](https://github.com/Jij-Inc/ommx/pull/738))

Previously, removed constraints were implicitly assumed not to reference fixed or dependent variable IDs. This release lifts that restriction — removed constraints may now contain such variables. Constraint hints are updated to reference only active constraints.

## Bug Fixes

### Clear constraint hints in penalty methods ([#739](https://github.com/Jij-Inc/ommx/pull/739))

`Instance.penalty_method` and `Instance.uniform_penalty_method` now correctly clear constraint hints when moving constraints to `removed_constraints`. Previously, stale hints could reference constraints that no longer existed as active constraints.

### Reduce constraint hint log level ([#740](https://github.com/Jij-Inc/ommx/pull/740))

The log message emitted when constraint hints are discarded has been changed from `warn` to `debug` to reduce noise in normal usage.

