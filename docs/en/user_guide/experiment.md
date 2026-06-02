# Experiment Management

{mod}`ommx.experiment` records optimization work as one OMMX Artifact. Use it when you need to compare multiple solver runs, keep model inputs and solver outputs together, or share the complete history with another environment.

For a runnable walkthrough, see [Record and Share Experiments](../tutorial/experiment_management.md). This page describes the API model and lifecycle.

## Data Model

An Experiment has two storage spaces.

| Space | API | Use for |
|---|---|---|
| Experiment space | {class}`~ommx.experiment.Experiment` logging methods | Shared context such as the source model, dataset metadata, or analysis notes |
| Run space | {class}`~ommx.experiment.Run` logging methods | One trial's parameters, solver input/output, traces, and run-specific files |

Use {meth}`~ommx.experiment.Run.log_parameter` for scalar values you want to compare across runs. These values appear in {meth}`~ommx.experiment.Experiment.run_parameters_df`.

Use attachments for payloads. JSON, {class}`~ommx.v1.Instance`, {class}`~ommx.v1.ParametricInstance`, {class}`~ommx.v1.Solution`, and {class}`~ommx.v1.SampleSet` have typed helpers. Unknown media types are stored and loaded as bytes so external packages can own their own codecs.

Use {meth}`~ommx.experiment.Run.log_solve` when a solver call should be recorded as one {class}`~ommx.experiment.Solve`. It stores the input Instance, output Solution, adapter class name, and JSON-serializable adapter options.

## Lifecycle

A new {class}`~ommx.experiment.Experiment` is an uncommitted session. Logging methods store payload bytes in the Local Registry immediately, but the Experiment is not shareable until it is committed. A successful {meth}`~ommx.experiment.Experiment.commit` writes the Experiment config and manifest, publishes the requested image reference, and turns the object into a read-only view.

Using `with Experiment(...)` calls `commit()` on normal exit:

```python
from ommx.experiment import Experiment

with Experiment("ghcr.io/example/team/experiment:baseline") as experiment:
    experiment.log_json("dataset", {"name": "demo"})
    with experiment.run() as run:
        run.log_parameter("capacity", 47)
```

Runs also have lifecycle. Use `with experiment.run()` so a run is closed even when user code raises. Closed runs have status `"finished"`, `"failed"`, or `"interrupted"`. `KeyboardInterrupt` is recorded as `"interrupted"`.

## Checkpoints

OMMX writes local checkpoints for partial Experiment state.

- Closing a Run publishes a best-effort draft checkpoint.
- Exiting an Experiment with an exception publishes a failed or interrupted checkpoint instead of advancing the successful Experiment image reference.
- A successful commit removes the local checkpoint when one exists.

Restore from a checkpoint with the original Experiment image name:

```python
experiment = Experiment.restore_from_checkpoint(
    "ghcr.io/example/team/experiment:baseline",
)
```

Checkpoint names and checkpoint Artifact handles are intentionally not exposed as normal public handles. Keep the original Experiment image name if you want to resume.

Payloads written by an open Run are stored in the Local Registry, but they become recoverable through a checkpoint only after the Run is closed. If a process is killed in the middle of an open Run, recovery starts from the latest closed Run checkpoint.

## Sharing

Committed Experiments can be shared like other OMMX Artifacts.

```python
experiment.rename("ghcr.io/example/team/experiment:baseline")
experiment.push()
experiment.save("experiment.ommx")
```

Use {meth}`~ommx.experiment.Experiment.load` for a named Experiment. Use {meth}`~ommx.experiment.Experiment.import_archive` for a received `.ommx` archive.

## Forking

A committed Experiment is immutable. To add more Runs, use {meth}`~ommx.experiment.Experiment.fork`.

```python
loaded = Experiment.load("ghcr.io/example/team/experiment:baseline")

with loaded.fork("ghcr.io/example/team/experiment:capacity-64") as child:
    with child.run() as run:
        run.log_parameter("capacity", 64)
```

The child inherits the parent's attachments, Runs, Solves, and run parameters. When committed, the child manifest records the parent manifest as OCI `subject`. Payload blobs are content-addressed and reused, so unchanged Instance, Solution, and attachment bytes are not duplicated.

## Local Registry Cleanup

The Local Registry stores blobs by digest and refs in SQLite. This makes repeated logging efficient, but it can leave orphan blobs when a process writes payloads that never become reachable from a committed Experiment or checkpoint.

Use the cleanup commands in dry-run mode first:

```bash
ommx prune-anonymous
ommx gc
```

Both commands report by default and only mutate the registry with `--delete`.

```bash
ommx prune-anonymous --delete
ommx gc --delete
```

`ommx gc` treats SQLite refs, including Experiment checkpoints, as roots. Unreachable blobs newer than the grace period are deferred to avoid deleting data from active writes. Pass `--show-digests` only when you need low-level diagnostic output.
