# Inspect saved Artifacts and manage the Local Registry

An Artifact is a package of saved problems, results, and other data. Use the
`ommx` command to inspect its contents and manage OMMX's local storage. These
operations are optional for solving, transformation, and candidate evaluation.

Follow the {ref}`CLI installation instructions <install-cli>`.
The commands below use the CLI included with the Python SDK; run them from the
working project or checkout where the SDK is installed. For the standalone CLI,
replace `uv run --no-sync ommx` with `ommx`. Check each command's `--help` for its
options.

## Inspect a file or a saved reference

Inspect the manifest (the list of stored data) in a received archive, or list the
names in the current Local Registry:

```sh
uv run --no-sync ommx inspect shared.ommx
uv run --no-sync ommx list
```

Inspection does not evaluate a problem or retrieve its Python objects. The
{class}`~ommx.artifact.Artifact` Reference gives a complete file-import and
retrieval example.

Replace the example name below with a saved reference to inspect its contents
and logical size:

```sh
uv run --no-sync ommx inspect example.com/team/result:trial-1
uv run --no-sync ommx size example.com/team/result:trial-1
```

The final `trial-1` is a tag. Names and tags can be updated, whereas a manifest
digest identifies its contents. Several names can refer to the same data.
`size` reports the original manifest's byte length, its config size, and its
unique layers' sizes; it excludes the parent `subject` chain. Shared data is
counted for each reference, so adding these values does not measure physical
disk usage. See {class}`~ommx.artifact.Artifact` for identity and storage details.

## Find records from Python

{func}`~ommx.artifact.list_artifacts` lists generic Artifacts and Experiments;
{func}`~ommx.experiment.list_experiments` supplies Experiment-specific records
and a complete example of filtering project annotations. The latter distinguishes
shared catalog metadata from conditions recorded per trial. Both examples work
without a solver.

Use {func}`~ommx.experiment.list_experiment_checkpoints` and
{meth}`~ommx.experiment.Experiment.restore_from_checkpoint` to find and restore
interrupted editing sessions. Keep the manifest digest when identifying exactly
which saved record was analyzed; a listed name may later point to other content.

(experiment-cleanup-workflow)=
## Remove and restore references, then reclaim storage

**Removing a name and reclaiming its data are separate operations.**
Replace `./local-registry` with the actual storage path. Each command explicitly
selects that registry with `--root`. Without it, OMMX uses
`OMMX_LOCAL_REGISTRY_ROOT`, or the platform default if the variable is unset.

### Inspect candidates first

```sh
uv run --no-sync ommx prune-anonymous --root ./local-registry --experiments --older-than 7d
uv run --no-sync ommx gc --root ./local-registry
```

Both commands report candidates without deleting anything by default.
`prune-anonymous` selects automatically named Artifacts; `--experiments` also
includes anonymous Experiments. `--older-than 7d` restricts their age. `gc`
(garbage collection) reports data no longer reachable from any saved reference.

### Remove or restore one name

Check the name and replace this example with the intended target. **`rm` removes
the reference immediately.** It does not delete the payload data.

```sh
uv run --no-sync ommx rm example.com/team/experiment:obsolete --root ./local-registry
```

The printed `Rollback` command contains the removed name, its immutable manifest
digest, and the selected root. Run that `ommx restore-ref ...` command to restore
the name, adding `uv run --no-sync` when using the Python SDK's CLI. Restoration
validates the complete stored data and refuses to overwrite a name that now
points to another digest.

Restoration requires the data to remain in the registry. Restore any needed
reference before reclaiming unreferenced data.

### Delete the inspected candidates

After inspecting the report, add `--delete` to remove matching anonymous names.
The command prints a rollback command for each removed name.

```sh
uv run --no-sync ommx prune-anonymous --root ./local-registry --experiments --older-than 7d --delete
```

Reclaim the underlying data separately:

```sh
uv run --no-sync ommx gc --root ./local-registry --delete
```

GC keeps data reachable from retained names, Experiment checkpoints, and parent
Artifacts referenced by retained children. Data left by an interrupted write
before publication can be unreachable. By default, only unreachable blobs whose
last modification is at least 24 hours old are eligible for deletion. This is
not a guaranteed 24-hour rollback window after removing a name: older blobs can
be reclaimed by the next GC pass. **Restoration may be impossible after GC.**

Keep a nonzero grace period while any process may be writing to the registry.
A custom `--grace-period` accepts `s`, `m`, `h`, and `d`; use `0s` only when no
OMMX process is writing there. Add `--show-digests` to investigate specific
missing, invalid, orphan, or deferred blobs. Normal reports show counts and
byte sizes.

```sh
uv run --no-sync ommx gc --root ./local-registry --grace-period 2h --show-digests
```

The Python APIs return structured reports: {func}`~ommx.artifact.remove_image`,
{func}`~ommx.artifact.restore_image`, {func}`~ommx.artifact.prune_anonymous`, and
{func}`~ommx.artifact.gc`. Their Reference entries own exact argument and
concurrency contracts. Implementation details of the CAS, index, and cache are
in the [Rust Local Registry documentation](https://github.com/Jij-Inc/ommx/blob/main/rust/ommx/src/artifact/local_registry/mod.rs).
