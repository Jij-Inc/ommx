# Install and Run the OMMX CLI

The `ommx` command lets you inspect and exchange Artifacts, manage the Local
Registry, and obtain bundled agent plugins and skills. You can run it as a
standalone executable or from the OMMX Python package.

If your uv project already has a Python SDK version that includes the CLI, run
this from the project root:

```bash
uv run ommx --help
```

## Install the standalone executable with cargo-binstall

Install [cargo-binstall](https://github.com/cargo-bins/cargo-binstall#installation),
then select a v3 version from the
[Rust SDK releases](https://github.com/Jij-Inc/ommx/releases?q=rust-3.0).
For example, the published `3.0.0-beta.6` release provides CLI archives:

```bash
cargo binstall ommx@3.0.0-beta.6
ommx --help
```

OMMX's release metadata lets cargo-binstall select the archive for your platform.
The standalone executable can be used without a Python environment.

To select the latest stable release, use `cargo binstall ommx`. During v3
prerelease development, specify a v3 version explicitly; an unversioned install
selects the stable v2 series. Choose a release whose notes include the commands
you need. The agent plugin and skill commands described in this documentation
are newer than `3.0.0-beta.6`; see the
[Rust SDK release notes](https://github.com/Jij-Inc/ommx/blob/main/rust/ommx/doc/release_note/3.0.md)
and the [Python SDK release notes](../release_note/ommx-3.0.md) for availability.

## Use the CLI included in the Python package

The Python `ommx` package includes the `ommx` console command starting with
`3.0.0b8`. Add that package to your uv project, then run the command in the
project's environment:

```bash
uv add 'ommx>=3.0.0b8'
uv run ommx --help
```

The CLI uses the OMMX version selected by the project's dependencies. If the
project already depends on a compatible `ommx`, no separate CLI installation is
needed. The command syntax is the same for the same CLI version. Available
commands depend on the installed version.

```{note}
The Python console command is currently listed under
[Unreleased](../release_note/ommx-3.0.md). The installation example above requires
a published version containing this feature. Python SDK `3.0.0b7` and earlier
do not provide this entry point.
```

## Check the version and available commands

```bash
uv run ommx --version
uv run ommx --help
uv run ommx inspect --help
```

For the standalone executable, use `ommx` in place of `uv run ommx`.
OMMX manages Rust and Python SDK versions independently; `ommx --version`
reports the Rust CLI version, including when invoked from Python.

To obtain and install the bundled agent plugin or individual skill, continue to
[Install OMMX Agent Plugins and Skills](agent_skills.md). For ref deletion,
restoration, and garbage collection, see
{ref}`Local Registry cleanup <experiment-cleanup-workflow>`.
