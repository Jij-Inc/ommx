# Install the Python SDK and adapters

**Until the 3.0.0 stable release, both the Book and API Reference describe `main`.** You can install either the latest published prerelease or the source on `main`.

| Installation | Choose it when |
|---|---|
| Latest prerelease | You want to start with published packages. A compatible wheel avoids building the SDK locally. |
| `main` | You need changes that have not been released yet, or want to work from the same source as the current documentation. |

A prerelease can lag behind the documentation. If a documented API is not yet available in your installed release, use `main`. Start with the SDK and add an adapter when you need a solver.

Install [uv](https://docs.astral.sh/uv/getting-started/installation/). These instructions use Python 3.12, which uv can install if needed.

(install-python-sdk)=
## 1. Install the Python SDK

### Latest prerelease

Create a working project outside the OMMX repository:

```sh
uv init --bare --python 3.12 ommx-work
cd ommx-work
uv add --python 3.12 --prerelease=allow ommx
```

This selects the latest compatible published version, including prereleases, and records the dependencies in `pyproject.toml` and `uv.lock`. Run subsequent commands for this route from `ommx-work`.

### main

Also install [Git](https://git-scm.com/), the [stable Rust toolchain](https://www.rust-lang.org/tools/install), and your operating system's C/C++ build tools. This route builds the SDK's Rust extension from source.

```sh
git clone --branch main https://github.com/Jij-Inc/ommx.git
cd ommx
uv sync --locked --package ommx --no-default-groups --python 3.12
```

Run subsequent commands for this route from the checkout root. uv installs the checked-out SDK with the dependencies recorded in the repository's `uv.lock`. Add adapters from this same checkout.

### Check the environment

Both routes create a `.venv`. From the directory for your chosen route, run:

```sh
uv run --no-sync python -c "from importlib.metadata import version; from ommx import Instance; print(version('ommx')); print(Instance.minimize())"
```

To run an example saved as a script, use `uv run --no-sync python example.py`. Transforming a problem with {meth}`~ommx.Instance.prepare`, checking {class}`~ommx.InstanceClass` membership, and evaluating a candidate as a {class}`~ommx.Solution` require only the SDK. Downloading public problems from [MIPLIB](../tutorial/download_miplib_instance.md) or [QPLIB](../tutorial/download_qplib_instance.md) requires network access on the first retrieval.

(install-adapters)=
## 2. Add an adapter when you need a solver

Use the commands for the same installation route as your SDK. For prereleases, `uv add` resolves the SDK and adapters together in your working project. For `main`, `--inexact` keeps other adapters and Notebook packages you have already added.

### PySCIPOpt Adapter

Latest prerelease:

```sh
uv add --prerelease=allow ommx-pyscipopt-adapter
```

main:

```sh
uv sync --locked --package ommx-pyscipopt-adapter --no-default-groups --inexact
```

Start with [Solve with an OMMX Adapter](../tutorial/solve_with_ommx_adapter.md). The {class}`~ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter` Reference contains complete examples of native special constraints and explicit preparation followed by solving.

(install-highs-adapter)=
### HiGHS Adapter

Latest prerelease:

```sh
uv add --prerelease=allow ommx-highs-adapter
```

main:

```sh
uv sync --locked --package ommx-highs-adapter --no-default-groups --inexact
```

The {class}`~ommx_highs_adapter.OMMXHighsAdapter` Reference describes its inputs and execution. To [compare adapters](../tutorial/switching_adapters.md), install both HiGHS and PySCIPOpt, then add matplotlib as described below.

(install-openjij-adapter)=
### OpenJij Adapter

Latest prerelease:

```sh
uv add --prerelease=allow ommx-openjij-adapter
```

main:

```sh
uv sync --locked --package ommx-openjij-adapter --no-default-groups --inexact
```

This also installs OpenJij itself. Other adapters and solver license setup are unnecessary. See the {mod}`~ommx_openjij_adapter` Reference for sampling APIs. The optional [TSP Notebook](../tutorial/tsp_sampling_with_openjij_adapter) uses matplotlib to draw routes.

## 3. Add Notebook and plotting tools when needed

For the prerelease project, record additional tools as dependencies:

```sh
uv add ipykernel
```

For the `main` checkout, add them to its environment:

```sh
uv pip install ipykernel
```

In a Jupyter-capable editor, select your chosen directory's `.venv` as the kernel. To register it with an existing Jupyter environment, run the following and select “OMMX Book” in the Notebook:

```sh
uv run --no-sync python -m ipykernel install --user --name ommx-book --display-name "OMMX Book"
```

For plotting, use `uv add matplotlib` in the prerelease project or `uv pip install matplotlib` in the `main` checkout.

## Update the environment

For the latest prerelease, update the SDK and all adapters recorded in the project together:

```sh
uv sync --upgrade --prerelease=allow
```

For `main`:

```sh
git pull --ff-only
uv sync --locked --package ommx --no-default-groups --inexact
```

Repeat the `main` installation commands for the adapters you use so that they match the SDK checkout. Restart active Notebook kernels after updating either environment.

(install-cli)=
## Optional: manage Artifacts with the CLI

The SDK includes the [OMMX CLI](../user_guide/cli.md). From either environment, run:

```sh
uv run --no-sync ommx --version
uv run --no-sync ommx --help
```

To install only the CLI without the Python SDK, get the `main` source as above and build it with Rust from the checkout root:

```sh
cargo install --locked --path rust/ommx --features cli
ommx --version
ommx --help
```

The CLI reports the Rust SDK version. Rust and Python SDK versions are managed independently.
