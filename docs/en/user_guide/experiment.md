# Experiment Recovery and Cleanup

{mod}`ommx.experiment` records optimization work as one OMMX Artifact. For the Experiment data model, a runnable logging example, sharing, inspection, and forked Experiments, see [Record and Share Experiments](../tutorial/experiment_management.md).

This guide focuses on the failure-time behavior: what OMMX writes before an Experiment is committed, how checkpoints are restored, and how Local Registry cleanup decides which blobs can be removed.

## Storage Boundaries

Experiment data is written in three layers.

| Layer | Stored as | Role |
|---|---|---|
| Blob | Content-addressed files in the Local Registry BlobStore | Payload bytes for attachments, Instances, Solutions, run parameters, configs, and manifests |
| Manifest | An OCI Image Manifest blob | The list of blobs that make one immutable OMMX Artifact |
| Ref | SQLite rows in the Local Registry index | The name or checkpoint pointer that makes a manifest reachable |

Logging methods such as {py:meth}`~ommx.experiment.Experiment.log_json` and {py:meth}`~ommx.experiment.Run.log_solve` write payload bytes to the BlobStore immediately. OMMX does not wait until the final commit to write all bytes. If the same content is already present, the existing CAS blob is reused and its modification time is touched so recent active writes remain protected by GC grace periods.

A successful {py:meth}`~ommx.experiment.Experiment.commit` writes the Experiment config and root manifest, then publishes the requested image reference in SQLite. Publishing a ref does not rewrite payload blobs. This ordering means a process can leave behind blob files that are not reachable from any manifest or ref; Local Registry GC handles that case.

## Context Managers Are Recovery Boundaries

Use both Experiment and Run objects as context managers.

```python
from ommx.experiment import Experiment

image_name = "ghcr.io/example/team/experiment:baseline"

with Experiment(image_name) as experiment:
    experiment.log_json("dataset", {"name": "demo"})
    with experiment.run() as run:
        run.log_parameter("capacity", 47)
```

The context-manager exits define what is recoverable.

| Event | Stored state |
|---|---|
| `Run` exits normally | The Run is recorded with status `"finished"` and a best-effort draft checkpoint is published. |
| `Run` exits with an exception | The Run is recorded with status `"failed"` or `"interrupted"` and a best-effort draft checkpoint is published. The exception still propagates. |
| `Experiment` exits normally | The final Experiment is committed, the requested image reference is published, and any local checkpoint for that Experiment is removed. |
| `Experiment` exits with an exception | The requested successful image reference is not advanced. A checkpoint Experiment is published with status `"failed"` or `"interrupted"`. |
| The process is killed before an open `Run` exits | Payload blobs written by that open Run may exist, but they are not part of recoverable Run state. Recovery starts from the latest checkpoint produced by a closed Run or by Experiment exception handling. |

`KeyboardInterrupt` is recorded as `"interrupted"` for both Run and Experiment status. Other exceptions are recorded as `"failed"`.

## Restoring a Checkpoint

Restore with the original Experiment image name.

```python
from ommx.experiment import Experiment

image_name = "ghcr.io/example/team/experiment:baseline"

with Experiment.restore_from_checkpoint(image_name) as experiment:
    with experiment.run() as run:
        run.log_parameter("capacity", 64)
```

Checkpoint refs are internal Local Registry refs derived from the original image name. They are intentionally not exposed as normal Artifact handles, so keep the original image name if you want to resume.

Restoration returns an uncommitted Experiment. On normal exit, it commits to the original requested image reference and removes the checkpoint. If the restored Experiment fails again, OMMX publishes a new failed or interrupted checkpoint instead of advancing the successful image reference.

## Reachability After Failure

Local Registry cleanup is based on reachability from SQLite refs.

| Data | Reachable? | Cleanup behavior |
|---|---|---|
| A committed Experiment image ref | Yes | `ommx gc` keeps its manifest, config, layers, and subject chain. |
| An Experiment checkpoint ref | Yes | `ommx gc` keeps the checkpoint so it can be restored. A successful commit removes the checkpoint. |
| A forked Experiment's parent manifest through OCI `subject` | Yes, if the child ref is kept | `ommx gc` walks the subject chain and keeps parent payloads reachable from kept children. |
| Anonymous artifact refs | Yes while the ref exists | `ommx prune-anonymous` removes these refs; a later `ommx gc` can reclaim their now-unreachable blobs. |
| Blobs written by a process that died before manifest/ref publication | No | `ommx gc` reports them as orphan candidates after the grace period. |
| Blobs written by a currently active process | Usually no until a checkpoint or commit exists | `ommx gc` defers them while they are newer than the grace period. |

OMMX does not store an orphan table in SQLite. Orphans are computed during each GC report by walking refs and manifests, then comparing that reachable set with the files in the BlobStore.

## Cleanup Workflow

Run cleanup commands in report mode first.

```bash
ommx prune-anonymous
ommx gc
```

Both commands are dry-run by default and mutate the registry only with `--delete`.

```bash
ommx prune-anonymous --delete
ommx gc --delete
```

Use {command}`ommx prune-anonymous` first when you have anonymous Artifact refs from temporary Artifact builds or unnamed archive imports. This command only removes matching SQLite refs; it does not unlink blobs. Those blobs become reclaimable by {command}`ommx gc` if no other ref reaches them.

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
