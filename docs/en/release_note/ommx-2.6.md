# OMMX Python SDK 2.6.x

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.6.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.6.0)

See the GitHub Releases above for full details. The following summarizes the main changes.

## New Features

### `Instance.substitute` (2.6.0, [#892](https://github.com/Jij-Inc/ommx/pull/892))

`Instance.substitute` is now exposed in the Python SDK.

This method rewrites an `Instance` by substituting decision variables with expressions. The substituted variables are recorded as dependent variables so their values can be reconstructed when evaluating a solution.

See the [Instance user guide](../user_guide/instance.ipynb) for details and modeling caveats.

### `ParametricInstance.substitute` (2.6.1, [#898](https://github.com/Jij-Inc/ommx/pull/898))

`ParametricInstance.substitute` is now exposed in the Python SDK.

This method substitutes decision variables while keeping parameter references symbolic. Assignment targets must be decision variables; attempting to substitute a parameter ID raises an error.

See the [ParametricInstance user guide](../user_guide/parametric_instance.ipynb) for the parameter-specific behavior.

## Bug Fixes

### Substitution validation and parameter materialization (2.6.1, [#898](https://github.com/Jij-Inc/ommx/pull/898))

`Instance.substitute` now rejects substitution expressions whose right-hand side references an undefined decision variable ID. `ParametricInstance.substitute` accepts right-hand side references only when they are registered decision variables or parameters.

`ParametricInstance.with_parameters` now also evaluates parameter references in `decision_variable_dependency`, so dependent-variable definitions are fully materialized when converting to an `Instance`.
