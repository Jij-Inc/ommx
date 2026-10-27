# Continue or restore an Experiment

Add Runs to an editable Experiment, restore interrupted editing from a checkpoint, or fork a committed record to continue without changing it. None of these operations resumes a solver's internal search state.

Use `ommx==3.0.0b7` from steps 1–2 of [Install the SDK](../getting_started/install.md). These small examples need no Adapter or external data. The names are dedicated to the examples; choose different names for your own data.

## Restore a checkpoint containing closed Runs

First, close one Run without committing the whole Experiment. With the default autosave policy, this creates a draft checkpoint.

```python
from ommx.experiment import Experiment

experiment = Experiment("example.com/book/checkpoint:demo")
with experiment.run() as run:
    run.log_parameter("trial", "first")
    run.log_json("result", {"value": 3})
```

You can end this Python process or Notebook kernel and continue with the code below. Use the same Local Registry. If you set `OMMX_LOCAL_REGISTRY_ROOT`, set the same value in the new process.

```python
from ommx.experiment import Experiment, list_experiment_checkpoints

name = "example.com/book/checkpoint:demo"
checkpoints = list_experiment_checkpoints(name)
assert checkpoints
print(checkpoints[0].status)

restored = Experiment.restore_from_checkpoint(name)
assert restored.runs[0].get_json("result") == {"value": 3}
with restored.run() as run:
    run.log_parameter("trial", "second")
    run.log_json("result", {"value": 2})
restored.commit()
assert len(restored.runs) == 2
```

Pass the originally requested Experiment name, not the generated internal checkpoint name. Committing makes the finalized record available under that name and deletes the checkpoint.

Restoration reaches only the latest saved checkpoint. It excludes an open Run's intermediate state. If you reduce autosave frequency, it may also exclude later closed Runs. The same API restores failed and interrupted checkpoints, but does not automatically retry failed calls.

## Fork a committed record

A committed Experiment is read-only. Fork it under another name to add trials while retaining the existing results. This example is independent of the previous section.

```python
from ommx.experiment import Experiment

with Experiment("example.com/book/fork:parent") as parent:
    with parent.run() as run:
        run.log_parameter("trial", "baseline")

with parent.fork("example.com/book/fork:child") as child:
    with child.run() as run:
        run.log_parameter("trial", "additional")

assert len(parent.runs) == 1
assert len(child.runs) == 2
```

The child retains the parent's attachments, Runs, Solves and Samplings, and saves them with its additional Run. The parent remains unchanged. Unchanged payloads are shared in the Local Registry; forking does not duplicate the same problem or result bytes.

To open the parent later, use `Experiment.load("example.com/book/fork:parent")`. For a received file, use `Experiment.import_archive(path)` and fork the returned object. Names also identify local records, so this procedure does not require a network push.

## Change autosave frequency when needed

For many short Runs, an editable Experiment can use a less frequent autosave policy.

```python
from ommx.experiment import AutosavePolicy, Experiment

experiment = Experiment("example.com/book/autosave:demo")
experiment.set_autosave_policy(AutosavePolicy.every_n_runs(25))
```

This checkpoints after each further 25 closed Runs, trading recovery granularity for fewer checkpoints. It is not needed for a first example. The setting belongs to the current session; set it again after restoration if required. See {py:class}`~ommx.experiment.AutosavePolicy` for intervals, disabling autosave and exceptional-exit conditions.

[Experiment records and lifecycle](../user_guide/experiment.md) explains record states. [Manage the Local Registry](./manage_local_registry.md) covers search and cleanup.
