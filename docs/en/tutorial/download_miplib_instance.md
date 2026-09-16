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

# Downloading a MIPLIB Instance

The OMMX repository provides mixed-integer programming benchmark instances from MIPLIB 2017 in OMMX Artifact format.

```{note}
`dataset.miplib2017` uses the published
`ghcr.io/jij-inc/ommx/v2.7/miplib2017:{instance-name}` distribution
([package](https://github.com/Jij-Inc/ommx/pkgs/container/ommx%2Fv2.7%2Fmiplib2017)).
These Artifacts were generated with the corrected MPS integer bounds in OMMX
2.7.0 and can also be read by the v3 SDK. The distribution version identifies
the adopted dataset, independently of the installed SDK version.

Distribution format or mathematical-model changes require an SDK minor or
major release and a new `/v{major}.{minor}/` namespace. Patch releases keep
their adopted distribution; published path/tag references are never overwritten.
The previous unversioned repository remains available for reproducibility,
but the loader does not fall back to it, including when old Artifacts are cached.

The [v2.7 publication record](https://github.com/Jij-Inc/ommx/blob/1ec99bbe3c36696e5fce3cac2cd87616e457af49/rust/dataset/distributions/v2.7/README.md)
lists the source archive, published instances, unsupported inputs, and model digests.

Please see [this page](https://docs.github.com/ja/packages/working-with-a-github-packages-registry/working-with-the-container-registry) for information on GitHub Container Registry.
```

You can easily download these instances with the OMMX SDK, then directly use them as inputs to OMMX Adapters.
For example, to solve the neos-1122047 instance from MIPLIB 2017 ([reference](https://miplib.zib.de/instance_details_neos-1122047.html)) with PySCIPOpt, you can:

1. Download the neos-1122047 instance with `dataset.miplib2017` from the OMMX Python SDK.
2. Solve with PySCIPOpt via the OMMX PySCIPOpt Adapter.

Here is a sample Python code:

```{code-cell} ipython3
# OMMX Python SDK
from ommx import dataset
# OMMX PySCIPOpt Adapter
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

# Step 1: Download the neos-1122047 instance from MIPLIB 2017
instance = dataset.miplib2017("neos-1122047")

# Step 2: Solve with PySCIPOpt via the OMMX PySCIPOpt Adapter
solution = OMMXPySCIPOptAdapter.solve(instance)
```

This functionality makes it easy to run benchmark tests on multiple OMMX-compatible solvers using the same MIPLIB instances.

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

MIPLIB-specific annotations are prefixed with `org.ommx.miplib.*`.

For example, the optimal objective of the neos-1122047 instance is `161`, which you can check with the key `org.ommx.miplib.objective`:

```{code-cell} ipython3
# Note that the values of annotations are all strings (str)!
instance.annotations["org.ommx.miplib.objective"]
```

Thus, we can verify that the optimization result from the OMMX PySCIPOpt Adapter matches the expected optimal value.

```{code-cell} ipython3
import numpy as np

best = float(instance.annotations["org.ommx.miplib.objective"])
assert np.isclose(solution.objective, best)
```
