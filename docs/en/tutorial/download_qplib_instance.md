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

# Downloading a QPLIB Instance

The OMMX repository provides quadratic programming benchmark instances from QPLIB in OMMX Artifact format.

```{note}
More details: The QPLIB instances in OMMX Artifact format are hosted in the GitHub Container Registry for the OMMX repository ([link](https://github.com/Jij-Inc/ommx/pkgs/container/ommx%2Fqplib)).

QPLIB is a library of quadratic programming instances. For more information about QPLIB, see the [QPLIB website](http://qplib.zib.de/).

Please see [this page](https://docs.github.com/ja/packages/working-with-a-github-packages-registry/working-with-the-container-registry) for information on GitHub Container Registry.
```

You can easily download these instances with the OMMX SDK, then directly use them as inputs to OMMX Adapters.
For example, to solve the QPLIB_3514 instance ([reference](http://qplib.zib.de/QPLIB_3514.html)) with PySCIPOpt, you can:

1. Download the 3514 instance with `dataset.qplib` from the OMMX Python SDK.
2. Solve with PySCIPOpt via the OMMX PySCIPOpt Adapter.

Here is a sample Python code:

```{code-cell} ipython3
# OMMX Python SDK
from ommx import dataset
# OMMX PySCIPOpt Adapter
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

# Step 1: Download the 3514 instance from QPLIB
instance = dataset.qplib("3514")

# Step 2: Solve with PySCIPOpt via the OMMX PySCIPOpt Adapter
solution = OMMXPySCIPOptAdapter.solve(instance)
```

This makes it easy to benchmark quadratic programming solvers using the same QPLIB instances.

+++

## Note about Annotations with the Instance

The downloaded instance includes various annotations accessible via the `annotations` property:

```{code-cell} ipython3
import pandas as pd
# Display annotations in tabular form using pandas
pd.DataFrame.from_dict(instance.annotations, orient="index", columns=["Value"]).sort_index()
```

These instances have both dataset-level annotations and dataset-specific annotations.

There are seven dataset-wide annotations with dedicated properties:

| Annotation                                    | Property          | Description                                               |
|----------------------------------------------|-------------------|-----------------------------------------------------------|
| `org.ommx.v1.instance.authors`               | `authors`         | The authors of the instance                              |
| `org.ommx.v1.instance.constraints`           | `num_constraints` | The number of constraint conditions in the instance      |
| `org.ommx.v1.instance.created`               | `created`         | The date of the instance was saved as an OMMX Artifact   |
| `org.ommx.v1.instance.dataset`               | `dataset`         | The name of the dataset to which this instance belongs   |
| `org.ommx.v1.instance.license`               | `license`         | The license of this dataset                              |
| `org.ommx.v1.instance.title`                 | `title`           | The name of the instance                                 |
| `org.ommx.v1.instance.variables`             | `num_variables`   | The total number of decision variables in the instance   |

## QPLIB Annotations

QPLIB instances include comprehensive annotations that describe the mathematical properties of quadratic programming problems. These annotations are based on the official QPLIB specification and are prefixed with `org.ommx.qplib.*`.

For detailed information about all available QPLIB annotations and their meanings, please refer to the [official QPLIB documentation](https://qplib.zib.de/doc.html).

For example, you can check the problem type and objective curvature of the QPLIB instance:

```{code-cell} ipython3
# QPLIB-specific annotations
print(f"Problem type: {instance.annotations['org.ommx.qplib.probtype']}")
print(f"Objective type: {instance.annotations['org.ommx.qplib.objtype']}")
print(f"Objective curvature: {instance.annotations['org.ommx.qplib.objcurvature']}")
print(f"Number of variables: {instance.annotations['org.ommx.qplib.nvars']}")
print(f"Number of constraints: {instance.annotations['org.ommx.qplib.ncons']}")
```
