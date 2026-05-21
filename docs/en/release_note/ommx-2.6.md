# OMMX Python SDK 2.6.x

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.6.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.6.0)

See the GitHub Releases above for full details. The following summarizes the main changes.

## New Features

### `Instance.substitute` (2.6.0, [#892](https://github.com/Jij-Inc/ommx/pull/892))

`Instance.substitute` is now exposed in the Python SDK.

This method rewrites an `Instance` by substituting decision variables with expressions. The substituted variables are recorded as dependent variables so their values can be reconstructed when evaluating a solution.
