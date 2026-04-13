---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: Python 3 (ipykernel)
  language: python
  name: python3
---

```{warning}
This document was written for the OMMX Python SDK 1.8.0 release and is not compatible with Python SDK 2.0.0 or later.
```

+++

# OMMX Python SDK 1.8.0

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_1.8.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-1.8.0)

Please refer to the GitHub Release for individual changes.

⚠️ Includes breaking changes due to the addition of `SolverAdapter`.

Summary
--------
- Added a new `SolverAdapter` abstract base class to serve as a common interface for adapters to different solvers.
- `ommx-python-mip-adapter` and `ommx-pyscipopt-adapter` have been changed to use `SolverAdapter` according to the [adapter implementation guide](https://jij-inc.github.io/ommx/en/ommx_ecosystem/solver_adapter_guide.html)
  - ⚠️ This is a breaking change. Code using these adapters will need to be updated.
  - Other adapters will be updated in future versions. 

+++

# Solver Adapter 

The introduction of the `SolverAdapter` base class aims to make the API for different adapters more consistent. `ommx-python-mip-adapter` and `ommx-pyscipopt-adapter` now use the `SolverAdapter` base class.

Here is an example of the new Adapter interface to simply solve an OMMX instance.

```{code-cell} ipython3
from ommx.v1 import Instance, DecisionVariable
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

p = [10, 13, 18, 32, 7, 15]
w = [11, 15, 20, 35, 10, 33]
x = [DecisionVariable.binary(i) for i in range(6)]
instance = Instance.from_components(
    decision_variables=x,
    objective=sum(p[i] * x[i] for i in range(6)),
    constraints=[sum(w[i] * x[i] for i in range(6)) <= 47],
    sense=Instance.MAXIMIZE,
)

solution = OMMXPySCIPOptAdapter.solve(instance)
solution.objective
```

With the new update, the process looks the same as the above when using the `OMMXPythonMIPAdapter` class instead.

To replace the usage of `instance_to_model()` functions, you can instantiating an adapter and using `solver_input`. You can then apply any solver-specific parameters before optimizing manually, then calling `decode()` to obtain the OMMX solution.

```{code-cell} ipython3
adapter = OMMXPySCIPOptAdapter(instance)
model = adapter.solver_input # in OMMXPySCIPOptAdapter's case, this is a `pyscipopt.Model` object
# modify model parameters here
model.optimize() 
solution = adapter.decode(model)
solution.objective
```
