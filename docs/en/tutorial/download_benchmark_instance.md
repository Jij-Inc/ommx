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

MIPLIB 2017 is a benchmark set for mixed-integer linear programming. `dataset.miplib2017()` loads the [published v2.7 distribution](https://github.com/Jij-Inc/ommx/pkgs/container/ommx%2Fv2.7%2Fmiplib2017), which corrects integer-variable bounds. The distribution version is independent of the installed SDK version and can be read by the v3 SDK.

Pass a problem name to `dataset.miplib2017()` to download it. The [`neos-1122047`](https://miplib.zib.de/instance_details_neos-1122047.html) problem used here can be passed directly to the PySCIPOpt Adapter without model conversion.

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

QPLIB is a benchmark set for quadratic programming. `dataset.qplib()` loads the [published v2.8 distribution](https://github.com/Jij-Inc/ommx/pkgs/container/ommx%2Fv2.8%2Fqplib), regenerated with corrected quadratic coefficients. The loader selects this corrected distribution even if an old unversioned Artifact is cached.

Pass the QPLIB problem number to `dataset.qplib()`. [`QPLIB_3514`](https://qplib.zib.de/QPLIB_3514.html) can also be passed directly to the PySCIPOpt Adapter.

```{code-cell} ipython3
qplib_instance = dataset.qplib("3514")
qplib_solution = OMMXPySCIPOptAdapter.solve(qplib_instance)
qplib_solution.objective
```

## Evaluate a Published QPLIB Solution

Download matching `.qplib` and `.sol` files from the [QPLIB website](https://qplib.zib.de/QPLIB_0018.html) to evaluate a published solution without running a solver.

```python
from ommx import Instance, State

instance = Instance.load_qplib("QPLIB_0018.qplib")
state = State.load_qplib_solution(
    "QPLIB_0018.sol", num_variables=len(instance.decision_variables)
)
solution = instance.evaluate(state, atol=1e-8)
print(solution.objective, solution.feasible)
```

Passing the original problem's variable count fills omitted variable values with zero. `objvar` is not treated as a decision variable. See {meth}`~ommx.State.load_qplib_solution` for the file format and error contract.

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

## Distributions and Reproducibility

The loaders use `ghcr.io/jij-inc/ommx/v2.7/miplib2017:{instance-name}` and `ghcr.io/jij-inc/ommx/v2.8/qplib:{numeric-tag}`. Changes to the distribution format or mathematical models use an SDK minor or major release and a new `/v{major}.{minor}/` namespace. Patch releases retain the adopted distribution, and published paths and tags are not overwritten. The loaders do not fall back to old unversioned distributions.

See the [MIPLIB v2.7 distribution record](https://github.com/Jij-Inc/ommx/blob/1ec99bbe3c36696e5fce3cac2cd87616e457af49/rust/dataset/distributions/v2.7/README.md) and [QPLIB v2.8 distribution record](https://github.com/Jij-Inc/ommx/blob/b4cffe9f1ce5323c15de67eca55b05d9f87285a0/rust/dataset/distributions/v2.8/README.md) for source data, corrections, and published digests. Reload a separately saved old Instance from the corrected distribution to apply the model fixes.
