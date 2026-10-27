# Install the Python SDK and Adapters

The examples linked here use a **preview release of OMMX Python SDK v3**. Install the SDK first, then add only the Adapter needed for your chosen solver.

An unversioned installation selects stable v2, whose API differs from these tutorials. The instructions below use the published `3.0.0b7` packages.

See [Versions and execution environments](compatibility.md) for task-specific dependencies, shared version boundaries, and Rust/CLI entry points. The minimal Artifact and Experiment examples also need only the SDK installed in steps 1–2. Add an Adapter only when using its backend.

## 1. Create a virtual environment

Use Python 3.12 in a working directory. The documented examples were checked on macOS (Apple Silicon) with Python 3.12.12.

```{note}
`3.0.0b7` has no Intel Mac wheel. See the [PyPI file list](https://pypi.org/project/ommx/3.0.0b7/#files) for distribution targets.
```

```sh
python -m venv .venv
```

On macOS or Linux, activate it with:

```sh
source .venv/bin/activate
```

On Windows PowerShell, use:

```powershell
.\.venv\Scripts\Activate.ps1
```

Use this environment for subsequent installation and example execution.

For notebooks, add a kernel to the same environment:

```sh
python -m pip install ipykernel
```

Select this environment's Python as the kernel in your Jupyter-capable editor. To register it with an existing Jupyter installation, run the following in the activated environment, then select “OMMX Book” in the notebook:

```sh
python -m ipykernel install --user --name ommx-book --display-name "OMMX Book"
```

(install-python-sdk)=
## 2. Install the Python SDK

```sh
python -m pip install "ommx==3.0.0b7"
```

This is enough to [transform an Instance and inspect the result](../how_to/transform_instance_without_solver.md). The [InstanceClass membership example](../user_guide/instance_class.md) also uses only the SDK.

It is also enough to [evaluate an existing problem and candidate](../tutorial/evaluate_existing_candidate). [Obtaining existing problems](../how_to/load_existing_instance.md) and evaluating candidates do not use an Adapter. The first retrieval of public data requires the network access described on those pages.

(install-adapters)=
## 3. Add only the Adapter you need

### PySCIPOpt Adapter

```sh
python -m pip install "ommx-pyscipopt-adapter==3.0.0b7"
```

Use this for the first two tutorials and the [custom Policy how-to](../how_to/customize_preparation.md):

1. [Solve a 0-1 knapsack problem with PySCIPOpt](../tutorial/solve_with_ommx_adapter): solve without transformation and read results.
2. [Solve special constraints natively with PySCIPOpt](../tutorial/solve_special_constraints_with_pyscipopt_adapter.md): use Indicator and SOS1.

(highs-adapter)=
### HiGHS Adapter

```sh
python -m pip install "ommx-highs-adapter==3.0.0b7"
```

Use this for [Prepare an Instance for an Adapter](../tutorial/prepare_instance_for_adapter.md). That page rebuilds its model and does not need the previous tutorials' runtime state or the PySCIPOpt Adapter.

To [compare several Adapters](../how_to/compare_adapters.md), install both Adapters and matplotlib for plotting:

```sh
python -m pip install matplotlib
```

(install-openjij-adapter)=
### OpenJij Adapter

Add the matching Adapter to the SDK `3.0.0b7` environment from steps 1–2:

```sh
python -m pip install "ommx-openjij-adapter==3.0.0b7"
```

This supports [Sample a small problem with the OpenJij Adapter](../how_to/sample_with_openjij_adapter). OpenJij is installed as a dependency; no separate solver installation or license configuration is needed. Other Adapters, such as PySCIPOpt or HiGHS, are not used.

For the optional [TSP application notebook](../tutorial/tsp_sampling_with_openjij_adapter), add matplotlib for plots:

```sh
python -m pip install matplotlib
```

```{note}
This Book targets v3 development; the generated API Reference includes features absent from public b7. See [Versions and execution environments](compatibility.md) and [Reference](../reference/index.md) for task-specific requirements and version-pinned sources. Additional external-tool and registry-authentication requirements are stated in the how-tos that use them.
```
