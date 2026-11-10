A collection of optimization trial records stored as one OMMX Artifact.

An Experiment owns shared attachments and a sequence of closed {class}`Run`
records. Each Run represents one trial, with scalar parameters, attachments,
and zero or more {class}`Solve` or {class}`Sampling` records. Solving,
transforming, or evaluating a problem does not require an Experiment.

# Record two candidates and read them back

This complete example evaluates two supplied candidates without a solver. The
shared problem belongs to the Experiment; each Run stores its own condition and
result. A temporary Local Registry keeps the example separate from your saved
data. Use `Experiment(image_name)` instead when you want a persistent local
record that can later be opened by {meth}`load`.

```pycon
>>> from ommx import Instance
>>> from ommx.experiment import Experiment
>>> problem = Instance.maximize()
>>> x = problem.new_binary("x")
>>> y = problem.new_binary("y")
>>> problem.objective = 3 * x + 2 * y
>>> _ = problem.add_constraint(x + y <= 1)
>>> with Experiment.with_temp_local_registry() as experiment:
...     experiment.log_instance("problem", problem)
...     for label, candidate in [
...         ("choose-x", {x.id: 1, y.id: 0}),
...         ("choose-y", {x.id: 0, y.id: 1}),
...     ]:
...         with experiment.run() as run:
...             solution = problem.evaluate(candidate)
...             run.log_parameter("candidate", label)
...             run.log_parameter("objective", solution.objective)
...             run.log_parameter("feasible", solution.feasible)
...             run.log_solution("evaluation", solution)
>>> experiment.run_parameters_df()[["candidate", "objective", "feasible"]].to_dict(
...     "records"
... )
[{'candidate': 'choose-x', 'objective': 3.0, 'feasible': True}, {'candidate': 'choose-y', 'objective': 2.0, 'feasible': True}]
>>> [
...     (run.run_id, run.status, run.get_solution("evaluation").objective)
...     for run in experiment.runs
... ]
[(0, 'finished', 3.0), (1, 'finished', 2.0)]

```

The table contains only values explicitly passed to {meth}`Run.log_parameter`.
A JSON attachment, an Adapter option, and an in-memory Python variable do not
automatically become table columns. The saved Solutions also retain their
candidate values. They record evaluation, not a proof of optimality.

Save the committed record to a new `.ommx` file. The following reader runs in a
new Python process with an empty Local Registry; it needs only that file, not
`experiment`, `problem`, or the sender's registry. It reads the stored results
without rerunning either a solver or the candidate evaluation.

```pycon
>>> import os
>>> from pathlib import Path
>>> import subprocess
>>> import sys
>>> from tempfile import TemporaryDirectory
>>> reader = """
... import sys
... from ommx.experiment import Experiment
... loaded = Experiment.import_archive(sys.argv[1])
... problem = loaded.get_instance("problem")
... assert len(problem.decision_variables) == 2
... for run in loaded.runs:
...     result = run.get_solution("evaluation")
...     print(run.run_id, run.status, result.objective, result.feasible)
... """
>>> with TemporaryDirectory() as directory:
...     archive = Path(directory) / "candidate-trials.ommx"
...     experiment.save(archive)
...     result = subprocess.run(
...         [sys.executable, "-c", reader, str(archive)],
...         env={
...             **os.environ,
...             "OMMX_LOCAL_REGISTRY_ROOT": str(Path(directory) / "receiver"),
...         },
...         check=True,
...         capture_output=True,
...         text=True,
...     )
>>> print(result.stdout, end="")
0 finished 3.0 True
1 finished 2.0 True

```

{meth}`save` does not overwrite an existing archive. Use a new path on each
export. {meth}`import_archive` imports the file into the reader's Local Registry
and opens a read-only Experiment. Reading stored results does not establish
that the original computation can be reproduced: preserve required source data,
code, settings, and environment information explicitly for that purpose.

# Choose a record's scope

| Record | What it holds | Read it through |
| --- | --- | --- |
| Experiment | Shared attachments and closed Runs | `attachment_names`, typed getters, `runs` |
| Run | Scalar comparison parameters, trial attachments, Solve/Sampling records | `run_parameters_df()`, {class}`SealedRun` |
| Solve | Input Instance, Adapter class/options, and a Solution on success | {class}`Solve` |
| Sampling | Input Instance, Adapter class/options, and the complete SampleSet on success | {class}`Sampling` |

{meth}`Run.log_solve` and {meth}`Run.log_sample` execute an Adapter and associate
its input with its output. The stored Instance is the one supplied to the
logging call. An Adapter's internally prepared copy is not automatically saved.
Policy choices, transformation weights, backend settings made through direct
model access, Python variables, and package/OS versions are not comprehensively
collected. Log additional inputs or explanations when later interpretation needs
them. {meth}`Run.open_solve` supports direct backend-model access and explicit
option recording.

The `finished`, `failed`, and `interrupted` statuses describe how recording
scopes ended. They do not establish feasibility or optimality. A successful
Sampling may contain no feasible candidate. A finished Run may contain a failed
Solve if its exception was handled inside the Run. Read candidate feasibility
and objectives from the Solution/SampleSet, and backend termination/bounds/gaps
from available Adapter diagnostics.

# Attachments

Use experiment-level attachments for common inputs and run-level attachments
for trial-specific data. Names are local to that scope: two Runs can each have
an attachment called `evaluation`. Standard methods include `log_json`,
`log_instance`, `log_parametric_instance`, `log_solution`, and `log_sample_set`.
{meth}`log_file` gives a complete file-and-model attachment example; it copies
bytes at logging time rather than retaining a link to the source file.

Typed getters reject a mismatched media type. {meth}`get_attachment` dispatches
on media type, and {meth}`get_blob` returns bytes. `compression="zstd"` is
available for JSON, codec, raw-byte, and file logging: getters and file export
return decompressed content and the original logical media type. OMMX marks
storage compression with reserved metadata, including when the logical media
type itself ends in `+zstd`. Use {meth}`log_with_codec` only for types requiring
a custom encoding; concrete codecs belong with the package that owns the type.

# Commit, recover, or fork

New Experiments are editable (unsealed). Close each Run as a context manager;
{meth}`commit` requires every open Run to be closed. Successful commit writes a
config and manifest, publishes the chosen name in the Local Registry, and makes
this object read-only. `with Experiment(...)` commits on normal exit. In a
Notebook, keep the Experiment open across cells, inspect closed Runs, add the
next trial, and commit explicitly when finished.

Logging writes payload bytes immediately. Payload storage alone does not make
a recoverable Experiment. By default, closing a Run also attempts to publish a
best-effort `draft` checkpoint containing closed Runs. {class}`AutosavePolicy`
controls that schedule. A process that dies with an open Run may have written
bytes, but that Run's partial state is not a recoverable closed Run. Runs closed
after the latest checkpoint are also absent from that recovery point.

Exceptional exit from an Experiment context tries to publish a `failed` or
`interrupted` checkpoint without advancing its successful image name;
`KeyboardInterrupt` uses `interrupted`. The exception propagates. Exceptions
outside a non-context-managed Experiment are not automatically captured.
Successful commit removes that Experiment's checkpoint.

- Use {meth}`restore_from_checkpoint` to resume editing the latest saved closed
  Runs after interruption; this does not resume a backend's search state.
- Use {meth}`fork` to add trials while preserving an already committed parent.
- Use {func}`list_experiments` for catalog and annotation filtering, and
  {func}`list_experiment_checkpoints` to discover recovery points.

An omitted image name creates an anonymous local name containing `.ommx.local`.
It can be exported as a file, but must be renamed to a remote registry reference
before {meth}`push`. Name/tag references can later point to different contents;
the Artifact manifest digest identifies an immutable record. File exchange and
remote registry distribution are alternatives, not requirements for recording.
{class}`ommx.artifact.Artifact` documents the common storage/identity model.

# Errors

Attachment lookup by an absent name raises {class}`KeyError`. Invalid
caller-provided image references, autosave values, media types, or JSON inputs
raise {class}`ValueError`. Registry, archive, storage, and lifecycle-state
failures raise {class}`RuntimeError`. Exceptions raised by Python codecs,
Adapters, tracing hooks, and data libraries pass through unchanged.
