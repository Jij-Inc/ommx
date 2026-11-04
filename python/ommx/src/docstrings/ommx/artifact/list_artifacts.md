List local Artifact references and their manifest metadata.

The result contains {class}`~ommx.artifact.ArtifactRef` records for both generic
Artifacts and Experiments. Each record supplies the full image name, manifest
and config digests, update time, Artifact type, manifest annotations, and the
parsed OCI manifest as a Python dictionary. Listing does not decode layer
payloads or fetch remote data. Use {func}`~ommx.experiment.list_experiments` when
you need only Experiments, together with their status, Run counts, and config.

`prefix` matches the start of the full image-reference string. It limits both
returned records and missing-cache backfill to the selected namespace. `root`
selects a Local Registry directory; omitting it uses the default registry.
Internal references, including Experiment checkpoints, are hidden by default.
`include_internal=True` is a registry diagnostic option; use
{func}`~ommx.experiment.list_experiment_checkpoints` to discover recoverable
Experiment sessions.

# Create a local record and inspect its catalog entry

This complete example creates one Artifact in a child process with a temporary
registry, then lists that registry from the current process. The image name
selects a local reference; no remote upload or authentication is involved.

```pycon
>>> import os
>>> import subprocess
>>> import sys
>>> from tempfile import TemporaryDirectory
>>> from ommx.artifact import list_artifacts
>>> create = """
... from ommx.artifact import ArtifactDraft
... draft = ArtifactDraft.new("example.com/optimization/candidates:trial-1")
... draft.add_annotation("com.example.study", "candidate-evaluation")
... draft.add_json({"objective": 3}, title="analysis")
... draft.commit()
... """
>>> with TemporaryDirectory() as registry:
...     result = subprocess.run(
...         [sys.executable, "-c", create],
...         env={**os.environ, "OMMX_LOCAL_REGISTRY_ROOT": registry},
...         capture_output=True,
...         text=True,
...     )
...     assert result.returncode == 0, result.stderr
...     refs = list_artifacts("example.com/optimization", root=registry)
...     assert len(refs) == 1
...     ref = refs[0]
...     print(ref.image_name)
...     print(ref.annotations["com.example.study"])
...     assert ref.manifest_digest.startswith("sha256:")
...     assert ref.config_digest == ref.manifest["config"]["digest"]
example.com/optimization/candidates:trial-1
candidate-evaluation

```

Filter annotations after listing according to the vocabulary your project
defines. Manifest annotations apply to the whole Artifact. Run-specific
conditions belong to {meth}`~ommx.experiment.Run.log_parameter`, rather than
being inferred from Artifact names or annotations.

# Cache and failure behavior

The Local Registry lists references through a digest-addressed manifest cache.
Missing manifest rows are validated and backfilled from the content-addressed
blob store before records are returned; later listings can read the cached
manifest without reopening its blob. The content-addressed store remains the
source for repairing the listing cache.

By default, an invalid cached record is repaired from that store when possible.
OMMX emits {class}`RuntimeWarning` for repaired or skipped references and returns
the remaining records. If the corresponding blob is also unavailable, that
reference is skipped. Pass `strict=True` to fail on the first invalid reference.
Database-wide schema/query errors and cache-write failures fail the whole
listing. Registry, manifest, and storage failures raise {class}`RuntimeError`.
