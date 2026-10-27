# Experiment records and lifecycle

Experiment groups the conditions and data associated with several trials. It is optional: describing, transforming and evaluating problems or executing an Adapter does not require it. To record and read back a first example, start with the [two-trial Tutorial](../tutorial/experiment_management).

## Choose the recording scope

| Scope | Recorded data | Readback |
|---|---|---|
| Experiment | Shared inputs and descriptions, and a collection of Runs | `attachment_names`, typed getters, `runs` |
| Run | Scalar trial conditions, attachments, and optionally Solves and Samplings | `run_parameters_df()`, Run getters, `solves`, `samplings` |
| Solve | One SolverAdapter call's input Instance, Adapter name and options, and returned Solution | Each entry in `run.solves`: `input`, `output` |
| Sampling | One SamplerAdapter call's input Instance, Adapter name and options, and complete returned SampleSet | Each entry in `run.samplings`: `input`, `output` |

A Run may contain several Solves or Samplings. It may also contain neither: you can record evaluated candidates or values computed elsewhere. The Tutorial uses this minimal form.

Use `log_parameter` for scalar conditions such as numbers, strings and booleans that you want to compare across Runs. Only explicitly recorded parameters become columns of `run_parameters_df()`. Store JSON, Instances, Solutions, images and log files as attachments, either at Experiment or Run scope. See [Attach data to an Experiment](../how_to/attach_experiment_data.md).

## What an Adapter call records

An Adapter connects OMMX inputs and outputs to an execution backend. A SolverAdapter returns one Solution; a SamplerAdapter returns a SampleSet of candidates. A record-only Run needs neither. Use the following methods when also recording the call.

`run.log_solve(Adapter, instance, **options)` executes the Adapter's `solve()` and records a Solve. `run.log_sample(...)` executes `sample()` and records a Sampling. They associate the input Instance, Adapter name, supplied keyword arguments and returned result. Options belong to the individual call; they do not automatically become Run parameter columns.

The recorded input is **the Instance supplied to the logging method**. If the Adapter's easy API prepares an internal copy, that prepared Instance is not automatically saved. Nor does Experiment comprehensively collect the selected Policy, applied weights, direct backend Model settings, Python variables, package versions or OS information.

Explicitly record the information you will need later. For example, if you prepare the Instance yourself before passing it to the call, that is the recorded input. Attach the original problem or the reason for choosing a Policy separately when needed. Recording does not require an extra Instance copy when the original Instance itself is not needed later.

{py:meth}`~ommx.experiment.Run.open_solve` is available when you need to manipulate the backend Model directly. Use `log_solve` for ordinary recording; consult the API Reference for detailed operations, diagnostics and failure contracts.

## Completion status is not result quality

A Run, Solve or Sampling's `status` describes whether that recording scope finished normally, failed or was interrupted.

- `finished` guarantees neither feasibility nor optimality. A normally returned SampleSet with no feasible candidate still produces a finished Sampling.
- If a Run handles a child call's exception, the Run can be finished while containing a failed Solve or Sampling.
- Read feasibility and objective values from the Solution or SampleSet. Availability of termination reasons, bounds and gaps depends on the Adapter/backend and diagnostic settings.

[Solution](./solution.md) and [SampleSet](./sample_set.md) explain basic result interpretation; [Transformed problems and removed constraints](./removed_constraints.md) explains transformed results. Experiment adds no mathematical guarantee to those results.

## Editing, committing and continuing

An Experiment starts as an editable session. A successful `commit()` makes it a read-only record. Normal exit from `with Experiment(...) as experiment:` commits automatically, which is convenient for short scripts. In a Notebook, you can keep an Experiment open across cells, inspect closed Runs, add another Run, and commit explicitly at the end.

Use Runs through `with experiment.run() as run:`. Closing a Run adds that trial to its parent Experiment. An Experiment cannot be committed while a Run remains open.

| Situation | Operation |
|---|---|
| Add a trial to the current editable session | `experiment.run()` |
| Finalize a record and read it later | `commit()`, then `Experiment.load(name)` |
| Add trials without changing a committed record | `fork()` creates a child session |
| Continue interrupted editing from a saved point | `restore_from_checkpoint(name)` |
| Read a committed record from a file | `Experiment.import_archive(path)` |

A fork carries over existing Runs and attachments without modifying its parent. Restoration opens a saved checkpoint for editing. Neither resumes a backend's internal search state. See [Continue or restore an Experiment](../how_to/continue_experiment.md).

## Persistence boundaries

Logging writes each payload to the Local Registry. Writing payload bytes is not the same as establishing a saved Experiment state that can be retrieved later.

By default, closing a Run creates a draft checkpoint containing the closed Runs so far. This is a best-effort recovery point. `AutosavePolicy` can change the interval or disable this autosave; Runs after the latest checkpoint may then be absent from recovery. An open Run's intermediate state is not saved as a recoverable Run.

If `with Experiment(...)` exits with an exception, it saves a failed or interrupted checkpoint without advancing the success name. The exception propagates to the caller. An Experiment not used as a context manager does not automatically catch exceptions outside its scopes.

A successful commit publishes the finalized Experiment under its name and deletes that session's checkpoint. Exact conditions are documented by {py:class}`~ommx.experiment.Experiment` and {py:class}`~ommx.experiment.AutosavePolicy`.

## Readback is different from rerunning

A committed Experiment is stored as an OMMX Artifact. `save()` exports a file, and `import_archive()` imports it on the reader's side. Saved inputs and results can be read without the Python objects that created them.

Rerunning the same computation depends on the inputs, code and settings explicitly retained, and on an available execution environment. Successful file readback alone does not establish reproducible rerunning. The Tutorial checks readback.

See [Artifact storage and identity](./artifact.md) for storage and exchange without a trial hierarchy, and [Manage the Local Registry](../how_to/manage_local_registry.md) for searching and cleanup. Understanding internal caches or GC implementation is not a prerequisite for using Experiment.
