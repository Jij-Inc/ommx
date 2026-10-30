# Install the Python SDK and adapters

**Until the 3.0.0 stable release, both the Book and API Reference describe `main`.** Install the SDK and adapters from the same checkout. Start with the SDK and add an adapter when you need a solver.

## 1. Get the main source

Install [Git](https://git-scm.com/), [uv](https://docs.astral.sh/uv/getting-started/installation/), the [stable Rust toolchain](https://www.rust-lang.org/tools/install), and your operating system's C/C++ build tools. The SDK's Rust extension is built from source. These instructions use Python 3.12, which uv can install if needed.

```sh
git clone --branch main https://github.com/Jij-Inc/ommx.git
cd ommx
```

Run the following commands from this repository's root.

(install-python-sdk)=
## 2. Install the Python SDK

```sh
uv sync --locked --package ommx --no-default-groups --python 3.12
uv run --no-sync python -c "from ommx import Instance; print(Instance.minimize())"
```

uv creates `.venv` and installs the checked-out SDK with the dependencies recorded in `uv.lock`. `--package ommx` selects only the SDK; add adapters and development tools when needed. To run an example saved as a script, use `uv run --no-sync python example.py`.

Transforming a problem with {meth}`~ommx.Instance.prepare`, checking {class}`~ommx.InstanceClass` membership, and evaluating a candidate as a {class}`~ommx.Solution` require only the SDK. Downloading public problems from [MIPLIB](../tutorial/download_miplib_instance.md) or [QPLIB](../tutorial/download_qplib_instance.md) requires network access on the first retrieval.

### Use a Notebook

Add a kernel to the same environment:

```sh
uv pip install ipykernel
```

In a Jupyter-capable editor, select this checkout's `.venv` as the kernel. To register it with an existing Jupyter environment, run the following and select “OMMX Book” in the Notebook:

```sh
uv run --no-sync python -m ipykernel install --user --name ommx-book --display-name "OMMX Book"
```

(install-adapters)=
## 3. Add an adapter when you need a solver

These commands install adapters from the same checkout. `--inexact` keeps other adapters and Notebook packages you have already added.

### PySCIPOpt Adapter

```sh
uv sync --locked --package ommx-pyscipopt-adapter --no-default-groups --inexact
```

Start with [Solve with an OMMX Adapter](../tutorial/solve_with_ommx_adapter.md). The {class}`~ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter` Reference contains complete examples of native special constraints and explicit preparation followed by solving.

(install-highs-adapter)=
### HiGHS Adapter

```sh
uv sync --locked --package ommx-highs-adapter --no-default-groups --inexact
```

The {class}`~ommx_highs_adapter.OMMXHighsAdapter` Reference describes its inputs and execution. To [compare adapters](../tutorial/switching_adapters.md), install both HiGHS and PySCIPOpt, then add matplotlib for plotting:

```sh
uv pip install matplotlib
```

(install-openjij-adapter)=
### OpenJij Adapter

```sh
uv sync --locked --package ommx-openjij-adapter --no-default-groups --inexact
```

This also installs OpenJij itself. Other adapters and solver license setup are unnecessary. See the {mod}`~ommx_openjij_adapter` Reference for sampling APIs.

The optional [TSP Notebook](../tutorial/tsp_sampling_with_openjij_adapter) uses matplotlib to draw routes:

```sh
uv pip install matplotlib
```

## Update from main

```sh
git pull --ff-only
uv sync --locked --package ommx --no-default-groups --inexact
```

Repeat the installation commands for the adapters you use so that they match the SDK checkout. Restart active Notebook kernels after updating the source.

(install-cli)=
## Optional: manage Artifacts with the CLI

The SDK includes the [OMMX CLI](../user_guide/cli.md):

```sh
uv run --no-sync ommx --version
uv run --no-sync ommx --help
```

To install only the CLI without the Python SDK, build it from the same checkout with Rust:

```sh
cargo install --locked --path rust/ommx --features cli
ommx --version
ommx --help
```

The CLI reports the Rust SDK version. Rust and Python SDK versions are managed independently.
