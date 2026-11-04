Restore an unsealed Experiment from its latest checkpoint.

Pass the original requested Experiment image name, not the generated internal
checkpoint ref. Accepted checkpoint statuses are `draft`, `failed`, and
`interrupted`. The result is editable and uses the original requested image
name recorded in checkpoint metadata. Discover recovery points with
{func}`list_experiment_checkpoints` and pass their `requested_image_name` here.

A new process must use the same Local Registry as the process that recorded the
checkpoint. If `OMMX_LOCAL_REGISTRY_ROOT` was set, give the reader the same value.
This self-contained example uses two separate processes and a temporary
registry. The first process closes one Run without committing the Experiment;
the default autosave policy writes the recovery point. The second process
restores that closed Run, adds another, and commits.

```pycon
>>> import os
>>> from pathlib import Path
>>> import subprocess
>>> import sys
>>> from tempfile import TemporaryDirectory
>>> writer = """
... from ommx.experiment import Experiment
... experiment = Experiment("example.com/checkpoint:demo")
... with experiment.run() as run:
...     run.log_parameter("trial", "first")
...     run.log_json("result", {"value": 3})
... """
>>> reader = """
... from ommx.experiment import Experiment, list_experiment_checkpoints
... name = "example.com/checkpoint:demo"
... checkpoints = list_experiment_checkpoints(name)
... assert len(checkpoints) == 1 and checkpoints[0].status == "draft"
... restored = Experiment.restore_from_checkpoint(checkpoints[0].requested_image_name)
... assert restored.runs[0].get_json("result") == {"value": 3}
... with restored.run() as run:
...     run.log_parameter("trial", "second")
...     run.log_json("result", {"value": 2})
... restored.commit()
... assert list_experiment_checkpoints(name) == []
... print(Experiment.load(name).run_parameters_df()["trial"].to_list())
... """
>>> with TemporaryDirectory() as directory:
...     env = {**os.environ, "OMMX_LOCAL_REGISTRY_ROOT": str(Path(directory) / "registry")}
...     _ = subprocess.run([sys.executable, "-c", writer], env=env, check=True)
...     result = subprocess.run(
...         [sys.executable, "-c", reader],
...         env=env,
...         check=True,
...         capture_output=True,
...         text=True,
...     )
>>> print(result.stdout, end="")
['first', 'second']

```

Only Runs included in the latest saved checkpoint are recovered. Open Runs are
not included; if autosave was throttled, some later closed Runs can also be
absent. This operation neither retries failed calls nor resumes a solver's
internal search. {meth}`Experiment.commit` publishes the original name and
removes its checkpoint. A restored session used as a context manager can publish
a new failed/interrupted checkpoint if it ends exceptionally again.

{class}`AutosavePolicy` belongs to a session and is not persisted. Reapply a
nondefault policy to the restored session if needed.
