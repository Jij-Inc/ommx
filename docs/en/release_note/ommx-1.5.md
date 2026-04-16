---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: .venv
  language: python
  name: python3
---

```{warning}
This document was written for the OMMX Python SDK 1.5.0 release and is not compatible with Python SDK 2.0.0 or later.
```

+++

# OMMX Python SDK 1.5.0

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_1.5.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-1.5.0)

This notebook describes the new features. Please refer the GitHub release note for the detailed information.

+++

## Evaluation and Partial Evaluation

From the first release of OMMX, `ommx.v1.Instance` supports `evaluate` method to produce `Solution` message

```{code-cell} ipython3
from ommx.v1 import Instance, DecisionVariable

# Create an instance of the OMMX API
x = DecisionVariable.binary(1)
y = DecisionVariable.binary(2)

instance = Instance.from_components(
    decision_variables=[x, y],
    objective=x + y,
    constraints=[x + y <= 1],
    sense=Instance.MINIMIZE
)
solution = instance.evaluate({1: 1, 2: 0})
```

```{code-cell} ipython3
solution.decision_variables
```

From Python SDK 1.5.0, `Function` and its base classes, `Linear`, `Quadratic`, and `Polynomial` also support `evaluate` method:

```{code-cell} ipython3
f = 2*x + 3*y
value, used_ids = f.evaluate({1: 1, 2: 0})
print(f"{value=}, {used_ids=}")
```

This returns evaluated value of the function and used decision variable IDs. If some decision variables are lacking, the `evaluate` method raises an exception:

```{code-cell} ipython3
try:
    f.evaluate({3: 1})
except RuntimeError as e:
    print(e)
```

In addition, there is `partial_evaluate` method

```{code-cell} ipython3
f2, used_ids = f.partial_evaluate({1: 1})
print(f"{f2=}, {used_ids=}")
```

This creates a new function by substituting `x = 1`. `partial_evaluate` is also added to `ommx.v1.Instance` class:

```{code-cell} ipython3
new_instance = instance.partial_evaluate({1: 1})
new_instance.objective
```

This method will be useful for creating a problem with fixing specific decision variables.
