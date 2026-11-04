# Install and Run the OMMX CLI

The `ommx` command lets you inspect and exchange Artifacts and obtain bundled
agent plugins and skills. The Python SDK includes the CLI starting with 2.9.4.
From your uv project, install a v2 package and run:

```bash
uv add 'ommx>=2.9.4,<3'
uv run ommx --help
```

```{note}
The CLI and bundled agent commands are under development for Python SDK 2.9.4.
Until that release is published, run `uv sync` from the root of a `dev-2.x`
source checkout instead of the `uv add` command above.
```

## Install the standalone executable

The standalone Rust executable works without a Python environment. To use the
agent commands before the next Rust SDK 2.x release, build from `dev-2.x`:

```bash
cargo install --git https://github.com/Jij-Inc/ommx --branch dev-2.x --locked ommx
ommx --help
```

After a Rust SDK 2.x release includes these commands, you can install that
version from crates.io with `cargo install ommx --version '<version>' --locked`.
Select a 2.x version explicitly to keep using the v2 CLI.

## Check the version and available commands

```bash
uv run ommx --version
uv run ommx --help
uv run ommx inspect --help
```

For the standalone executable, use `ommx` in place of `uv run ommx`.
OMMX manages Rust and Python SDK versions independently; `ommx --version`
reports the Rust CLI version, including when invoked from Python.
The v2 Artifact commands include `load`, `save`, `list`, `inspect`, `push`,
and `pull`; consult each command's `--help` for arguments.

To obtain and install the bundled agent plugin or individual skill, continue to
[Install OMMX Agent Plugins and Skills](agent_skills.md).
