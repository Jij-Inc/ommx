# Install and Run the OMMX CLI

The `ommx` command lets you inspect and exchange Artifacts, manage the Local
Registry, and obtain bundled agent plugins and skills. You can run it as a
standalone executable or from the OMMX Python package.

Until the 3.0.0 stable release, this Book and the API Reference describe `main`.
Use the [installation guide](../getting_started/install.md)
to choose the latest prerelease or a Python SDK environment built from `main`.

## Use the CLI included in the Python package

The Python SDK includes the `ommx` console command. From your working project or checkout root, run
it in the environment prepared by the installation guide:

```bash
uv run --no-sync ommx --help
```

The command uses the same installed SDK as your Python code. No separate CLI
installation is needed.

## Install the standalone executable from main

To use the CLI without a Python environment, install the Rust toolchain and
get the `main` source as described in the installation guide. Run this from the repository root:

```bash
cargo install --locked --path rust/ommx --features cli
ommx --help
```

The standalone executable provides the same commands as the Python package's
CLI built from that revision.

## Check the version and available commands

```bash
uv run --no-sync ommx --version
uv run --no-sync ommx --help
uv run --no-sync ommx inspect --help
```

For the standalone executable, use `ommx` in place of `uv run --no-sync ommx`.
OMMX manages Rust and Python SDK versions independently; `ommx --version`
reports the Rust CLI version, including when invoked from Python.

To obtain and install the bundled agent plugin or individual skill, continue to
[Install OMMX Agent Plugins and Skills](agent_skills.md). For ref deletion,
restoration, and garbage collection, see
{ref}`Local Registry cleanup <experiment-cleanup-workflow>`.
