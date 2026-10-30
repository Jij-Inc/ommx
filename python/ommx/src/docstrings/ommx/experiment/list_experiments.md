List Experiment refs using the Local Registry's digest-addressed JSON cache.

Each {class}`ExperimentRef` exposes its name, immutable Manifest and Config
digests, update time, lifecycle status, Run/Solve/Sampling counts, Manifest
annotations, and complete Experiment Config. {func}`ommx.artifact.list_artifacts`
also lists non-Experiment Artifacts; use this function for the Experiment
projection.

`prefix` matches the full image-reference string and limits both listing and
cache backfill. Missing Manifest or Config rows are validated and backfilled
from the content-addressed store (CAS). Invalid entries are repaired when
possible; repaired or skipped refs emit {class}`RuntimeWarning`. Pass
`strict=True` to fail on the first invalid ref. Database/schema/query and
cache-write failures fail the whole listing. `root` selects a nondefault Local
Registry; otherwise the process-wide default is used.

# Record catalog fields, then select trials

Use project-owned reverse-DNS annotation keys for dimensions shared by an
Experiment, such as dataset, formulation, solver, or source revision. Values
are strings and `org.ommx.*` keys are reserved. Values that vary between Runs
belong in {meth}`Run.log_parameter` instead. The following complete example
records two batches and selects one by annotation. A temporary registry in a
separate process makes its setup independent of your saved records.

```pycon
>>> import os
>>> from pathlib import Path
>>> import subprocess
>>> import sys
>>> from tempfile import TemporaryDirectory
>>> script = """
... import pandas as pd
... from ommx.experiment import Experiment, list_experiments
... namespace = "example.com/study/batches"
... for dataset in ["small", "large"]:
...     with Experiment(f"{namespace}:{dataset}") as experiment:
...         experiment.set_annotation("com.example.study", "candidate-comparison")
...         experiment.set_annotation("com.example.dataset", dataset)
...         experiment.set_annotation("com.example.git-revision", "a1b2c3d")
...         with experiment.run() as run:
...             run.log_parameter("seed", 42)
... refs = list_experiments(namespace)
... columns = {"study": "com.example.study", "dataset": "com.example.dataset"}
... rows = []
... for ref in refs:
...     row = {
...         "image_name": ref.image_name,
...         "manifest_digest": ref.manifest_digest,
...         "config_digest": ref.config_digest,
...         "updated_at": ref.updated_at,
...         "status": ref.status,
...         "run_count": ref.run_count,
...     }
...     row.update({column: ref.annotations.get(key) for column, key in columns.items()})
...     rows.append(row)
... catalog = pd.DataFrame.from_records(rows, columns=[
...     "image_name", "manifest_digest", "config_digest", "updated_at", "status", "run_count", *columns,
... ])
... catalog["updated_at"] = pd.to_datetime(catalog["updated_at"], utc=True)
... selected = catalog.loc[
...     (catalog["status"] == "finished") & (catalog["dataset"] == "small")
... ].sort_values("updated_at", ascending=False)
... loaded = [Experiment.load(name) for name in selected["image_name"]]
... assert len(loaded) == 1
... print(loaded[0].run_parameters_df()["seed"].to_list())
... """
>>> with TemporaryDirectory() as directory:
...     result = subprocess.run(
...         [sys.executable, "-c", script],
...         env={
...             **os.environ,
...             "OMMX_LOCAL_REGISTRY_ROOT": str(Path(directory) / "registry"),
...         },
...         check=True,
...         capture_output=True,
...         text=True,
...     )
>>> print(result.stdout, end="")
[42]

```

The project owns the annotation vocabulary, column types, and missing-value
policy. Missing annotation keys produce `None` in this projection. No registry
schema change is required to add a project-specific column.

`ref.config["runs"]` contains the Run/Solve/Sampling structure. For example,
iterate each Run's `solves` list and project `run_id`, `solve_id`, `status`,
`adapter`, and `json.loads(solve["adapter_options"])` into a per-Solve table.
The config refers to payload layers rather than embedding their values: read
scalar Run parameters through {meth}`Experiment.run_parameters_df`, and use the
loaded Run's typed getters for inputs/results and attachments.

An image name is mutable; several names can identify one manifest. Use
`manifest_digest` to deduplicate catalog rows and to identify the exact record
an analysis used. Loading by a tag later can open changed contents. Recoverable
uncommitted sessions are listed separately by
{func}`list_experiment_checkpoints`.
