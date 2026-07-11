# Experiment Discovery, Recovery, and Cleanup

{mod}`ommx.experiment` records optimization work as one OMMX Artifact. For the Experiment data model, a runnable logging example, sharing, inspection, and forked Experiments, see [Record and Share Experiments](../tutorial/experiment_management.md).

This guide covers Local Registry workflows around committed and interrupted Experiments: finding a relevant Experiment by project-defined metadata, restoring checkpoints, and deciding which blobs cleanup can remove.

## Inspect the Artifact Catalog without Opening Manifest Blobs

{py:func}`ommx.artifact.list_artifacts` lists every matching OMMX Artifact ref,
including generic Artifacts and Experiments. Each record contains the image
name, Manifest and Config digests, update time, `artifactType`, Manifest
annotations, and the complete OCI Manifest as a Python dictionary.

```python
from ommx.artifact import list_artifacts

refs = list_artifacts("example.com/optimization")
for ref in refs:
    print(ref.image_name, ref.artifact_type, ref.annotations)
```

The Local Registry reads these records by joining its SQLite `refs` and
digest-addressed Manifest cache. A missing Manifest row is validated and
backfilled from the content-addressed blob store on the first listing. Later
listings return the same immutable Manifest JSON from SQLite without opening
the Manifest blob. Use the optional `prefix` to limit both backfill and returned
records to a registry namespace or partial full image reference.

Internal Local Registry refs, including rolling Experiment checkpoints, are
excluded by default. `list_artifacts(..., include_internal=True)` is a
diagnostic escape hatch for inspecting those refs; use the dedicated checkpoint
API below for recovery workflows.

The CAS remains the source of truth for the listing cache. If one cached
Manifest is invalid, the default listing repairs it from the CAS and emits a
`RuntimeWarning`. If its CAS blob is also unavailable, the listing warns, skips
that ref, and returns the other records. Pass `strict=True` when a diagnostic or
validation workflow should fail on the first invalid ref. Database-wide schema
and query failures, and SQLite cache-write failures, always fail the whole
listing.

Use {py:func}`~ommx.experiment.list_experiments` when the catalog should contain
only Experiments and also needs Experiment status, run/solve counts, or the
complete Experiment Config.

## Catalog and Filter Experiments with Annotations

Suppose a team runs a continuing QAP solver comparison. Each committed
Experiment represents one batch for a particular problem instance, solver,
formulation, and source revision. The image name places all batches under one
registry namespace, while manifest annotations describe the dimensions that
the project expects to search later.

Define these project-specific fields with reverse-DNS annotation keys and set
them before committing the Experiment. Keys under `org.ommx.*` are reserved by
OMMX, and annotation values are strings.

```python
from ommx.experiment import Experiment

image_name = "example.com/optimization/qap-experiments:tai20a-highs-20260710"

with Experiment(image_name) as experiment:
    experiment.set_annotation("com.example.study", "qap-solver-comparison")
    experiment.set_annotation("com.example.instance", "tai20a")
    experiment.set_annotation("com.example.solver", "highs")
    experiment.set_annotation("com.example.formulation", "assignment")
    experiment.set_annotation("com.example.git-revision", "a1b2c3d")

    with experiment.run() as run:
        run.log_parameter("seed", 42)
        run.log_parameter("time_limit_seconds", 300)
```

Use annotations for Experiment-level catalog fields shared by the whole
artifact. Values that vary between Runs, such as a seed or time limit in this
example, belong in {py:meth}`~ommx.experiment.Run.log_parameter` instead.

Later, list the registry namespace and project each project's annotation
schema into ordinary DataFrame columns. Built on the same Manifest cache as
`list_artifacts`, {py:func}`~ommx.experiment.list_experiments` also joins the
Experiment Config cache. It returns the image name, immutable Manifest and
Config digests, update time, status, run/solve counts, Manifest annotations,
and the complete Experiment Config for each matching Experiment ref.

```python
import pandas as pd

from ommx.experiment import Experiment, list_experiments

annotation_columns = {
    "study": "com.example.study",
    "instance": "com.example.instance",
    "solver": "com.example.solver",
    "formulation": "com.example.formulation",
    "git_revision": "com.example.git-revision",
}

refs = list_experiments("example.com/optimization/qap-experiments")
rows = []
for ref in refs:
    row = {
        "image_name": ref.image_name,
        "manifest_digest": ref.manifest_digest,
        "config_digest": ref.config_digest,
        "updated_at": ref.updated_at,
        "status": ref.status,
        "run_count": ref.run_count,
        "solve_count": ref.solve_count,
        "sampling_count": ref.sampling_count,
    }
    row.update(
        {
            column: ref.annotations.get(annotation_key)
            for column, annotation_key in annotation_columns.items()
        }
    )
    rows.append(row)

catalog = pd.DataFrame.from_records(
    rows,
    columns=[
        "image_name",
        "manifest_digest",
        "config_digest",
        "updated_at",
        "status",
        "run_count",
        "solve_count",
        "sampling_count",
        *annotation_columns,
    ],
)
catalog["updated_at"] = pd.to_datetime(catalog["updated_at"], utc=True)

candidates = catalog.loc[
    (catalog["status"] == "finished")
    & (catalog["study"] == "qap-solver-comparison")
    & (catalog["instance"] == "tai20a")
    & (catalog["formulation"] == "assignment")
    & catalog["solver"].isin(["highs", "scip"])
].sort_values("updated_at", ascending=False)

selected_experiments = [
    Experiment.load(image_name) for image_name in candidates["image_name"]
]
```

The `prefix` argument is the coarse Local Registry filter and matches the full
image-reference string. Annotation-aware filtering is intentionally performed
after listing: each project owns its annotation vocabulary, column types, and
missing-value policy, so it can adapt the DataFrame projection without changing
the registry schema. Missing annotations appear as `None` in the example.

The complete config is available as `ref.config`. It contains the Run and Solve
structure, so consumers can build additional project-owned tables without
adding columns to the Local Registry schema. For example, this projects one row
per Solve for adapter and status analysis:

```python
import json

solve_rows = []
for ref in refs:
    for run in ref.config["runs"]:
        for solve in run.get("solves", []):
            solve_rows.append(
                {
                    "manifest_digest": ref.manifest_digest,
                    "run_id": run["run_id"],
                    "solve_id": solve["solve_id"],
                    "status": solve["status"],
                    "adapter": solve["adapter"],
                    "adapter_options": json.loads(solve["adapter_options"]),
                }
            )

solves = pd.json_normalize(solve_rows)
```

The config contains references to payload layers, not the payload values
themselves. In particular, scalar Run parameter values are stored in the Run
parameter layer; load selected Experiments and use
{py:meth}`~ommx.experiment.Experiment.run_parameters_df` when those values are
needed.

An image name is a mutable ref and may later point to another commit. Use
`manifest_digest` as the immutable identity when deduplicating rows, recording
which exact Experiment was analyzed, or comparing catalogs captured at
different times. The same manifest can appear more than once when several refs
point to it.

## Storage Boundaries

Experiment data is stored in the CAS, with refs and listing caches in SQLite.

| Layer | Stored as | Role |
|---|---|---|
| Blob | Content-addressed files in the Local Registry | Payload bytes for attachments, Instances, Solutions, run parameters, configs, and manifests |
| Manifest | An OCI Image Manifest blob | The list of blobs that make one immutable OMMX Artifact |
| Ref | SQLite rows in the Local Registry index | The name or checkpoint pointer that makes a manifest reachable |
| Listing cache | SQLite rows keyed by manifest or config digest | Original Manifest and Experiment Config JSON used by registry listings |

The cache stores the original JSON bytes under their content digest and verifies
that digest when reading them. A missing cache row is populated from the CAS on
listing, so the first listing after a v1 Local Registry migration or an older
write path may read Manifest and Config blobs. Once populated, listing reads
those JSON values from SQLite without constructing each Experiment. Replacing
or deleting refs removes cache rows that are no longer reachable from any ref.

In this page, **publish** means updating a Local Registry ref so it points to
an already-written manifest. This is a local SQLite operation. It does not mean
pushing an Artifact to a remote container registry.

Logging methods such as {py:meth}`~ommx.experiment.Experiment.log_json` and {py:meth}`~ommx.experiment.Run.log_solve` write payload bytes to the Local Registry immediately. OMMX does not wait until the final commit to write all bytes. If the same content is already present, the existing CAS blob is reused and its modification time is touched so recent active writes remain protected by GC grace periods.

A successful {py:meth}`~ommx.experiment.Experiment.commit` writes the Experiment config and root manifest, then publishes the requested image reference in SQLite. Publishing a ref does not rewrite payload blobs. This ordering means a process can leave behind blob files that are not reachable from any manifest or ref; Local Registry GC handles that case.

## Run Contexts and Experiment Commit

Use `Run` objects as context managers. A Run is one trial, and closing it is
the recovery boundary that adds the closed Run to the parent Experiment's
uncommitted state. By default, after the Run is closed, OMMX writes a draft
checkpoint for that parent Experiment and publishes the checkpoint ref.

An Experiment does not have to be a context manager. In notebooks, a typical
workflow keeps one Experiment open across multiple cells: run one trial,
inspect plots and tables, decide the next condition, run another trial, and
commit explicitly when the human workflow is finished.

```python
from ommx.experiment import Experiment

image_name = "ghcr.io/example/team/experiment:baseline"

experiment = Experiment(image_name)
experiment.log_json("dataset", {"name": "demo"})

with experiment.run() as run:
    run.log_parameter("capacity", 47)

# Inspect results, make plots, and decide the next condition.

with experiment.run() as run:
    run.log_parameter("capacity", 64)

artifact = experiment.commit()
```

For a parameter sweep with many short Runs, checkpointing the growing
Experiment after every Run can create many superseded config and run-parameter
blobs before the final commit. Set an autosave policy on the unsealed session
to choose a different recovery/storage tradeoff:

```python
from ommx.experiment import AutosavePolicy, Experiment

experiment = Experiment(image_name)
experiment.set_autosave_policy(AutosavePolicy.every_n_runs(25))
```

`every_n_runs(n)` checkpoints after each group of `n` additional closed Runs.
`min_interval(seconds)` attempts to checkpoint the first subsequently closed
Run and then at most once per interval; a failed publish attempt also waits for
the interval before retrying. `disabled()` skips Run-close draft checkpoints, and
`every_run_close()` restores the default. Changing policy starts a fresh
schedule at the current closed-Run count. The policy belongs to the current
unsealed session and is not persisted in a checkpoint or committed Experiment.
Failed and interrupted checkpoints produced by an exceptional Experiment
context exit are not disabled by this policy.

For batch scripts where all Runs are known in advance, `with Experiment(...)`
is a convenience: normal exit calls `commit()`, and exceptional exit publishes
a failed or interrupted checkpoint instead of advancing the successful image
reference.

| Operation or event | Stored state |
|---|---|
| `Run` exits normally | The closed Run is added to the parent Experiment with status `"finished"`. A best-effort draft checkpoint is published when the autosave policy is due. |
| `Run` exits with an exception | The closed Run is added to the parent Experiment with status `"failed"` or `"interrupted"`. A best-effort draft checkpoint is published when the autosave policy is due. The exception still propagates. |
| `experiment.commit()` succeeds | The final Experiment is committed, the requested image reference is published, and any local checkpoint for that Experiment is removed. |
| `with Experiment(...)` exits normally | Equivalent to calling `commit()` at the end of the block. |
| `with Experiment(...)` exits with an exception | The requested successful image reference is not advanced. A checkpoint Experiment is published with status `"failed"` or `"interrupted"`. |
| A notebook kernel or process dies after a Run has closed but before `commit()` | Recovery starts from the latest Experiment draft checkpoint allowed by the autosave policy; Runs closed after that checkpoint must be repeated. |
| A notebook kernel or process dies before an open `Run` exits | Payload blobs written by that open Run may exist, but they are not part of recoverable Run state. Recovery starts from the latest checkpoint before that Run. |

`KeyboardInterrupt` is recorded as `"interrupted"` for both Run and Experiment status. Other exceptions are recorded as `"failed"`.

Run status records how the Run scope was closed. It is not an aggregate status
of child Solve records, so a Run with status `"finished"` may still contain
failed Solve attempts when the adapter errors were handled inside the Run.

If you do not use Experiment as a context manager, exceptions outside a Run do
not automatically publish a failed Experiment checkpoint. The usual interactive
workflow relies on Experiment draft checkpoints produced after Run closes and an
explicit {py:meth}`~ommx.experiment.Experiment.commit` when the Experiment is
ready to publish.

## Restoring a Checkpoint

Use {py:func}`~ommx.experiment.list_experiment_checkpoints` to find recoverable
Experiments by their original requested image name. A `"draft"` checkpoint is
the rolling autosave written after a Run closes and is also the recovery point
after a hard process or notebook-kernel exit. `"failed"` and `"interrupted"`
checkpoints record exceptions that OMMX observed while closing an Experiment.

```python
from ommx.experiment import list_experiment_checkpoints

checkpoints = list_experiment_checkpoints(
    "ghcr.io/example/team",
    statuses=["draft", "failed", "interrupted"],
)
for checkpoint in checkpoints:
    print(
        checkpoint.requested_image_name,
        checkpoint.status,
        checkpoint.updated_at,
    )
```

The `prefix` matches `requested_image_name`, rather than the hashed internal
checkpoint ref. Omit `statuses` to include all three checkpoint statuses. As
with the other catalog functions, individual cache failures warn and skip by
default; pass `strict=True` to fail on the first invalid checkpoint.

Restore by passing the selected checkpoint's original requested image name.

```python
from ommx.experiment import Experiment, list_experiment_checkpoints

checkpoint = list_experiment_checkpoints(
    "ghcr.io/example/team/experiment:baseline"
)[0]

experiment = Experiment.restore_from_checkpoint(checkpoint.requested_image_name)

with experiment.run() as run:
    run.log_parameter("capacity", 64)

artifact = experiment.commit()
```

Checkpoint refs are derived from the original image name and remain internal
Local Registry implementation details. `checkpoint_image_name` is exposed on
the listing record for registry diagnostics; recovery uses
`requested_image_name`.

Restoration returns an uncommitted Experiment, so it can be kept open across
notebook cells just like a newly created Experiment. Calling `commit()` publishes
the original requested image reference and removes the checkpoint. If the
restored Experiment is used as a context manager and fails again, OMMX publishes
a new failed or interrupted checkpoint instead of advancing the successful image
reference.

## Reachability After Failure

Local Registry cleanup is based on reachability from SQLite refs.

| Data | Reachable? | Cleanup behavior |
|---|---|---|
| A committed Experiment image ref | Yes | `ommx gc` keeps its manifest, config, layers, and subject chain. |
| An Experiment checkpoint ref | Yes | `ommx gc` keeps the checkpoint so it can be restored. A successful commit removes the checkpoint. |
| A forked Experiment's parent manifest through OCI `subject` | Yes, if the child ref is kept | `ommx gc` walks the subject chain and keeps parent payloads reachable from kept children. |
| Anonymous Artifact and Experiment refs | Yes while the ref exists | `ommx prune-anonymous` removes anonymous Artifact refs; add `--experiments` to include anonymous Experiments. A later `ommx gc` can reclaim their now-unreachable blobs. |
| Blobs written by a process that died before manifest/ref publication | No | `ommx gc` reports them as orphan candidates after the grace period. |
| Blobs written by a currently active process | Usually no until a checkpoint or commit exists | `ommx gc` defers them while they are newer than the grace period. |

OMMX does not store an orphan table in SQLite. Orphans are computed during each GC report by walking refs and manifests, then comparing that reachable set with the CAS files in the Local Registry.

## Cleanup Workflow

Run cleanup commands in report mode first.

```bash
ommx prune-anonymous
ommx gc
```

Both commands are dry-run by default and mutate the registry only with `--delete`.
Include anonymous Experiments explicitly, and use `--older-than` for age-based
retention.

```bash
ommx prune-anonymous --experiments --older-than 7d
ommx prune-anonymous --delete --experiments --older-than 7d
ommx gc --delete
```

Remove a specific named or anonymous ref with `ommx rm`. This removes only the
mutable ref. Blob reclamation remains a separate `ommx gc --delete` operation,
with the normal GC grace period applying.

```bash
ommx rm example.com/team/experiment:obsolete
```

After each deletion, the CLI prints a copyable rollback command containing the
removed ref's immutable Manifest digest:

```text
     Removed example.com/team/experiment:obsolete
    Rollback ommx restore-ref 'example.com/team/experiment:obsolete' 'sha256:...'
     Storage Unreferenced data remains until a later `ommx gc --delete` removes it after the grace period.
```

`restore-ref` validates the stored Manifest and its complete
config/layer/subject closure, and refuses to overwrite the ref if it now points
to a different digest. Validation and ref publication are serialized against
deleting GC passes across processes. Restoring an Experiment also republishes
its validated listing projection atomically with the ref. Rollback requires the
complete closure to remain in the Local Registry CAS. A later
`ommx gc --delete` may reclaim it once it is unreachable and past the grace
period. `prune-anonymous --delete` prints one rollback command per removed ref.

The same operations are available from the Python SDK. Python returns
structured reports instead of formatted CLI output.

```python
from ommx.artifact import gc, prune_anonymous, remove_image, restore_image

prune_report = prune_anonymous(experiments=True, older_than="7d")
gc_report = gc()

prune_deleted = prune_anonymous(
    delete=True,
    experiments=True,
    older_than="7d",
)
removed_digest = remove_image("example.com/team/experiment:obsolete")
assert removed_digest is not None
restored = restore_image(
    "example.com/team/experiment:obsolete",
    removed_digest,
)
gc_deleted = gc(delete=True)
```

Use `root=...` to inspect a non-default Local Registry and
`grace_period="2h"` to override the GC grace period.

Use {command}`ommx prune-anonymous` first when you have anonymous Artifact refs from temporary Artifact builds or unnamed archive imports. Add `--experiments` for anonymous Experiment sessions. This command only removes matching SQLite refs; it does not unlink blobs. Those blobs become reclaimable by {command}`ommx gc` if no other ref reaches them. Pruning uses compare-and-delete semantics so a ref replaced after candidate selection is not removed as the stale candidate.

{command}`ommx gc` performs a mark-sweep pass:

- Roots are all SQLite refs, including Experiment checkpoint refs.
- For each reachable manifest, GC marks the manifest blob, config blob, layer blobs, and OCI `subject` manifest chain.
- Blob files outside the marked set are unreachable.
- Unreachable blobs older than `--grace-period` are reported as orphan candidates.
- Unreachable blobs newer than `--grace-period` are reported as deferred.
- With `--delete`, only orphan candidates are unlinked, and each candidate is checked again immediately before deletion.

The default grace period is `24h`. The option accepts `s`, `m`, `h`, and `d` suffixes.

```bash
ommx gc --grace-period 2h
ommx gc --grace-period 0s
```

Use `0s` only when you know no OMMX process is writing to that registry. For a shared or default Local Registry, keep a nonzero grace period so open Runs and interrupted imports are not deleted while they are still being written.

Normal reports show counts and byte sizes rather than raw digests. Add `--show-digests` when investigating a specific missing, invalid, orphan, or deferred blob.

```bash
ommx gc --show-digests
ommx gc --delete --show-digests
```

Use `--root <path>` to inspect or clean a non-default Local Registry.
