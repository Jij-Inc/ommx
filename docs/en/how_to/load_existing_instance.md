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

# Load an existing problem as an Instance

Obtain an `Instance` from a public benchmark or a local file. **Obtaining a problem and solving it are separate operations.** Use it to evaluate supplied candidates, transform it, or solve it.

Install Python SDK `3.0.0b7` using steps 1–2 of the [installation guide](../getting_started/install.md). The following acquisition and loading operations do not use an Adapter.

## Obtain a public benchmark

The first retrieval requires network access to the public registry `ghcr.io`.

### QPLIB

QPLIB is a library of quadratic programming instances. Request problem `0018` as a string, including the leading zeros:

```{code-cell} ipython3
from ommx import dataset

qplib_instance = dataset.qplib("0018")
print(qplib_instance.title)
print(f"決定変数の数: {len(qplib_instance.decision_variables)}")
```

You now have an Instance. If candidate values are available, inspect their objective and constraint satisfaction with [Evaluate an existing problem and candidate](../tutorial/evaluate_existing_candidate).

### MIPLIB 2017

Retrieve a mixed-integer programming instance from MIPLIB 2017 by name:

```{code-cell} ipython3
from ommx import dataset

miplib_instance = dataset.miplib2017("neos-1122047")
print(miplib_instance.title)
print(f"決定変数の数: {len(miplib_instance.decision_variables)}")
```

Data is stored in the Local Registry and reused when the same reference is already available. You do not need to download the whole MIPLIB or QPLIB archive first.

See the official [QPLIB](https://qplib.zib.de/) and [MIPLIB 2017](https://miplib.zib.de/) sites for problem identifiers and descriptions. Not every original instance is included in the OMMX distribution. Distribution records below describe included inputs and limitations.

## Load a local problem file

Use a loader for the existing file's format. Replace `problem.mps` or `QPLIB_0018.qplib` below with a real path.

For MPS:

```python
from ommx import Instance

instance = Instance.load_mps("problem.mps")
```

For QPLIB:

```python
from ommx import Instance

instance = Instance.load_qplib("QPLIB_0018.qplib")
```

These operations load an OMMX problem representation; they do not search for candidates. See {meth}`~ommx.Instance.load_mps` and {meth}`~ommx.Instance.load_qplib` for supported inputs and errors. A problem file alone does not provide candidate variable values.

## Receive a problem from a modeler or Artifact

If a modeler produces an OMMX Instance, use it for subsequent evaluation, transformation, or Adapter calls. Check the modeler's own documentation, such as the [JijModeling documentation (Japanese)](https://jij-inc-jijmodeling-tutorials-ja.readthedocs-hosted.com/ja/latest/introduction.html), for export instructions and supported SDK versions. The linked documentation may target different APIs. If it requires a different SDK version, do not mix those instructions into this `3.0.0b7` environment unchanged.

An OMMX Artifact packages problem, candidate, and result data for exchange. To receive an `.ommx` file containing an Instance:

```python
from ommx.artifact import Artifact

artifact = Artifact.import_archive("problem.ommx")
instance = artifact.instance
```

`import_archive()` persistently imports the file contents into the Local Registry. `instance` returns its first problem. See {meth}`~ommx.artifact.Artifact.get_instance` to select among several problems and {meth}`~ommx.artifact.Artifact.import_archive` for import details. Creating and sharing Artifacts is a separate task, not a prerequisite to loading a problem here.

## Check distributions and versions

The public benchmark loaders use these distributions:

| Loader | Distribution | Corrections and records |
|---|---|---|
| `dataset.miplib2017(name)` | `ghcr.io/jij-inc/ommx/v2.7/miplib2017:{name}` | Corrected integer-variable bounds in MPS. [v2.7 distribution record](https://github.com/Jij-Inc/ommx/blob/1ec99bbe3c36696e5fce3cac2cd87616e457af49/rust/dataset/distributions/v2.7/README.md) |
| `dataset.qplib(tag)` | `ghcr.io/jij-inc/ommx/v2.8/qplib:{tag}` | Corrected quadratic coefficients. [v2.8 distribution record](https://github.com/Jij-Inc/ommx/blob/b4cffe9f1ce5323c15de67eca55b05d9f87285a0/rust/dataset/distributions/v2.8/README.md) |

`v2.7` and `v2.8` identify **dataset distributions**, not the installed SDK, which remains `3.0.0b7`. The loaders do not fall back to older unversioned distributions. Record the distribution reference as well as the problem name. Distribution records include original sources, included instances, and digests identifying model contents.

These versions are independent:

- **SDK version:** the Python API, here `ommx==3.0.0b7`.
- **Message wire schema:** serialized problems and results. The `v1` in `application/org.ommx.v1.instance` is not the SDK major version. See [Message definitions for b7](https://github.com/Jij-Inc/ommx/tree/python-3.0.0b7/proto).
- **Artifact format:** the package containing Messages and other data, defined independently of Python by the [Artifact specification](https://github.com/Jij-Inc/ommx/blob/python-3.0.0b7/rust/ommx/doc/artifact_design.md).

## Next steps

- Check a candidate: [Evaluate an existing problem and candidate](../tutorial/evaluate_existing_candidate).
- Transform its representation: [Transform an Instance and inspect the result](./transform_instance_without_solver.md).
- Execute it: [Choose an OMMX Adapter](../user_guide/supported_ommx_adapters.md).

Inspect authors, license, and provenance through `instance.authors`, `instance.license`, and `instance.annotations`. A published known objective value is problem metadata, not a feasibility or optimality guarantee for your candidate.
