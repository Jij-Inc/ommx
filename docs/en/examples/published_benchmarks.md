---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
kernelspec:
  display_name: Python 3 (ipykernel)
  language: python
  name: python3
---

# Solve a published benchmark and compare its result

The {mod}`~ommx.dataset` loaders return ordinary {class}`~ommx.Instance` objects.
Install the SDK and PySCIPOpt adapter to run this example. The first retrieval
requires access to the public registry. The same solver can then be used with
either distribution.

```{code-cell} ipython3
from math import isclose
from ommx import dataset
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

mip = dataset.miplib2017("neos-1122047")
result = OMMXPySCIPOptAdapter.solve(mip)
published = float(mip.annotations["org.ommx.miplib.objective"])
assert result.feasible
assert isclose(result.objective, published, abs_tol=1e-6)
```

The MIPLIB annotation is a string describing the benchmark. Convert it to a
number before comparing it with the evaluated solver result. QPLIB annotations
instead use the `org.ommx.qplib.*` prefix and include the problem type and
objective curvature.

```{code-cell} ipython3
quadratic = dataset.qplib("3514")
print(quadratic.annotations["org.ommx.qplib.probtype"])
result = OMMXPySCIPOptAdapter.solve(quadratic)
assert result.feasible
```

If you already have candidate values, the {func}`QPLIB solution example
<ommx.qplib.load_solution>` shows how to evaluate them without searching.
