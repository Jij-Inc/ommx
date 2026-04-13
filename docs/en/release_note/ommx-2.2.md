# OMMX Python SDK 2.2.x

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.2.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.2.0)

Released on 2025-11-14.

## Breaking Changes

### Relax `EvaluatedDecisionVariable` invariants ([#676](https://github.com/Jij-Inc/ommx/pull/676))

Previously, constructing an `EvaluatedDecisionVariable` enforced that the assigned value satisfied the variable's bound and kind constraints. This prevented representing infeasible solutions.

This release relaxes these invariants: bound and kind checks are removed from construction. Instead, `Solution.feasible` now checks both constraint satisfaction **and** decision variable bound/kind compliance. This allows solvers to return infeasible solutions (e.g. from time-limited runs) without raising errors during construction.
