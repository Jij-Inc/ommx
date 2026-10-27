# Inspect saved Artifacts and manage the Local Registry

The `ommx` command inspects and exchanges Artifacts, manages the Local Registry, and provides bundled agent plugins and skills. Install a standalone executable or use the CLI included in a suitable Python package.

The public Python SDK `3.0.0b7` does not include the CLI. Keep the b7 environment unchanged and install the standalone CLI below first. Instructions for a development Python package that includes the CLI appear separately later on this page.

## Obtain the CLI

These operations are optional. Solving, evaluating candidates, transforming problems, and comparing Adapters do not require the CLI. Install the Rust CLI separately without changing the Python examples' SDK `3.0.0b7` environment.

### Install a standalone executable with cargo-binstall

Install [cargo-binstall](https://github.com/cargo-bins/cargo-binstall#installation), then select a v3 [Rust SDK release](https://github.com/Jij-Inc/ommx/releases?q=rust-3.0). The published `3.0.0-beta.7` release includes CLI binaries:

```bash
cargo binstall ommx@3.0.0-beta.7
ommx --help
```

cargo-binstall chooses the appropriate archive using release metadata. The standalone executable works without a Python environment.

Use `cargo binstall ommx` to select the latest stable release. During the v3 prerelease period, specify v3 explicitly; omitting the version selects stable v2. Choose a release containing the commands you need. See [Agent plugins and skills](../user_guide/agent_skills.md) for auxiliary-tool availability. This page concerns Artifact management only.

### CLI included with the development Python package

The Python package includes the `ommx` command starting with `3.0.0b8`. In a uv project:

```bash
uv add 'ommx==3.0.0b8'
uv run ommx --help
```

`3.0.0b8` is not the public b7 baseline used for this Book's examples. If it is unpublished, use the standalone CLI above. Use a separate environment for development b8; do not upgrade the existing b7 environment just for this procedure.

The project's resolved OMMX version is used. No separate CLI installation is needed when a suitable package is already a dependency. Commands have the same syntax for the same CLI version, but availability varies between versions.

## Check versions and commands

```bash
uv run ommx --version
uv run ommx --help
uv run ommx inspect --help
```

For a standalone executable, replace `uv run ommx` with `ommx`. Rust and Python SDK versions are managed independently: `ommx --version` reports the Rust CLI version even when launched through Python.

## Inspect a file or saved name

Read the manifest, the inventory of packaged data, in a received `.ommx` file:

```sh
ommx inspect shared.ommx
ommx list
```

This does not evaluate an Instance or Solution. See the [Artifact tutorial](../tutorial/share_in_ommx_artifact.md) to retrieve contents. `list` displays names in the current Local Registry. Given a saved name, inspect its target and logical size; replace these names with your own:

```sh
ommx inspect example.com/team/result:trial-1
ommx size example.com/team/result:trial-1
```

Several names can refer to the same data. Summing `size` values does not measure actual disk usage. See [Names and digests](../user_guide/artifact.md).

## Search from Python

The following works in public SDK b7. An empty registry gives empty results.

```python
from ommx.artifact import list_artifacts

for record in list_artifacts("example.com/team"):
    print(record.image_name, record.manifest_digest, record.annotations)
```

Use {py:func}`~ommx.experiment.list_experiments` to find only Experiments. Record experiment-wide search criteria at creation, for example with `experiment.set_annotation("com.example.study", "trial")`. Do not use reserved `org.ommx.*` annotations. Record conditions that vary by Run with `run.log_parameter()`.

```python
from ommx.experiment import Experiment, list_experiments

for record in list_experiments("example.com/team"):
    if record.annotations.get("com.example.study") == "trial":
        loaded = Experiment.load(str(record.image_name))
        print(loaded.run_parameters_df())
```

A name can later refer to different contents. Retain `manifest_digest` to identify exactly what you inspected. See [Continue, checkpoint, and fork an Experiment](continue_experiment.md) to resume work.

(experiment-cleanup-workflow)=
## Inspect targets before deletion

These commands report candidates without deleting by default:

```sh
ommx prune-anonymous
ommx gc
```

`prune-anonymous` targets automatically generated name references; `gc` targets data blobs unreachable from any reference. Add `--experiments` to include automatically named Experiments. Inspect counts, targets, and age first:

```sh
ommx prune-anonymous --experiments --older-than 7d
```

The following actually deletes data. Run only after confirming the targets:

```sh
ommx rm example.com/team/result:obsolete
ommx prune-anonymous --delete --experiments --older-than 7d
ommx gc --delete
```

`rm` and `prune-anonymous --delete` remove name references and display a restoration command. While blobs remain, use the displayed `ommx restore-ref '<name>' '<digest>'`. After `gc --delete` collects the blobs, that registry alone cannot restore them. Save important data to an `.ommx` file or another backup first.

GC's default grace period is 24 hours. Do not use `--grace-period 0s` on shared or active registries: the grace period protects in-progress writes. When cleaning another registry, verify the target and use `--root <path>` on commands such as `gc` or `rm`.

For structured Python reports, see {py:func}`~ommx.artifact.gc`, {py:func}`~ommx.artifact.prune_anonymous`, {py:func}`~ommx.artifact.remove_image`, and {py:func}`~ommx.artifact.restore_image`. Exact retention and restoration conditions belong to the API Reference.
