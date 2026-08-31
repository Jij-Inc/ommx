---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: ommx
  language: python
  name: python3
---

# Download Benchmark Instances

The OMMX Python SDK's `dataset` submodule provides benchmark problems from [MIPLIB 2017](https://miplib.zib.de/) and [QPLIB](https://qplib.zib.de/) as {class}`~ommx.Instance` values. In this tutorial, we download one problem from each dataset and solve it with the PySCIPOpt Adapter.

## Installing the Required Library

```
pip install ommx-pyscipopt-adapter
```

## MIPLIB 2017

MIPLIB 2017 is a benchmark set for mixed-integer linear programming. The OMMX-format Artifacts are published on [GitHub Container Registry](https://github.com/Jij-Inc/ommx/pkgs/container/ommx%2Fmiplib2017).

Pass a problem name to `dataset.miplib2017()` to download it. The [`neos-1122047`](https://miplib.zib.de/instance_details_neos-1122047.html) problem used here can be passed directly to the PySCIPOpt Adapter, so it needs no Preparation.

```{code-cell} ipython3
from ommx import dataset
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

miplib_instance = dataset.miplib2017("neos-1122047")
miplib_solution = OMMXPySCIPOptAdapter.solve(miplib_instance)
```

The MIPLIB-specific annotation `org.ommx.miplib.objective` stores the known optimal value as a string. We can compare it with the objective value returned by the Adapter.

```{code-cell} ipython3
import math

miplib_best = float(miplib_instance.annotations["org.ommx.miplib.objective"])
assert math.isclose(miplib_solution.objective, miplib_best)
miplib_solution.objective
```

## QPLIB

QPLIB is a benchmark set for quadratic programming. The OMMX-format Artifacts are published on [GitHub Container Registry](https://github.com/Jij-Inc/ommx/pkgs/container/ommx%2Fqplib).

Pass the QPLIB problem number to `dataset.qplib()`. [`QPLIB_3514`](https://qplib.zib.de/QPLIB_3514.html) can also be passed directly to the PySCIPOpt Adapter.

```{code-cell} ipython3
qplib_instance = dataset.qplib("3514")
qplib_solution = OMMXPySCIPOptAdapter.solve(qplib_instance)
qplib_solution.objective
```

## Inspect the Annotations

The downloaded Instance's `annotations` include common metadata such as the title, authors, license, and dataset name, together with dataset-specific metadata.

```{code-cell} ipython3
import pandas as pd

pd.DataFrame.from_dict(
    miplib_instance.annotations,
    orient="index",
    columns=["value"],
).sort_index()
```

QPLIB-specific annotations use the `org.ommx.qplib.*` prefix. For example, we can inspect the problem and objective types and the objective curvature recorded in the original QPLIB data.

```{code-cell} ipython3
qplib_annotations = {
    "problem_type": qplib_instance.annotations["org.ommx.qplib.probtype"],
    "objective_type": qplib_instance.annotations["org.ommx.qplib.objtype"],
    "objective_curvature": qplib_instance.annotations["org.ommx.qplib.objcurvature"],
    "source_variables": qplib_instance.annotations["org.ommx.qplib.nvars"],
    "source_constraints": qplib_instance.annotations["org.ommx.qplib.ncons"],
}
pd.Series(qplib_annotations, name="value")
```

`org.ommx.qplib.ncons` is the number of constraints recorded in the original QPLIB data.
