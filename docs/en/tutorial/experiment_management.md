---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: ommx-update-books (3.9.23)
  language: python
  name: python3
---

# Record and Share Experiments

In practical mathematical optimization, a workflow rarely ends by simply building a mathematical model and sending it to a solver. In many cases, you compare multiple formulations, relax some constraints, or try easier subproblems. In addition to managing modeled problems and solver results through adapters, OMMX provides APIs for recording these trial-and-error processes as experiments, then saving and sharing them.

{py:mod}`~ommx.experiment` is the API for storing such experiment units as OMMX Artifacts.

```{list-table}
:header-rows: 1

* - Concept
  - Role
* - {py:class}`~ommx.experiment.Experiment`
  - The whole experiment. It can have experiment-level Attachments and multiple Runs. It is the sharing unit, and it always has a container-style name.
* - {py:class}`~ommx.experiment.Run`
  - One trial within an experiment and the comparison unit. Since complex workflows often call solvers more than once, a Run can contain multiple solver calls (Solves). A Run can also have scalar parameters used as comparison axes, making it easy to compare Runs across the Experiment.
* - {py:class}`~ommx.experiment.Solve`
  - One solver call within a Run. It always stores the input {py:class}`~ommx.v1.Instance`, the Adapter used, and the options passed to the solver call. A finished Solve also stores the output {py:class}`~ommx.v1.Solution`; a failed or interrupted Solve has no output.
* - Attachment
  - An arbitrary payload attached to an Experiment or Run. It can store data types such as JSON, `numpy.ndarray`, {py:class}`~ommx.v1.Instance`, and {py:class}`~ommx.v1.Solution`, as well as arbitrary bytes with an explicit Media Type.
```

In this tutorial, we solve a simple knapsack problem twice under different conditions, then save and read the execution history as one {py:class}`~ommx.experiment.Experiment`.

+++

## Prepare the Mathematical Model

First, create the source data for a knapsack problem and a {py:class}`~ommx.v1.ParametricInstance` whose capacity is a parameter. Like {py:class}`~ommx.v1.Instance`, an OMMX {py:class}`~ommx.v1.ParametricInstance` can define an objective function and constraints, but it can place parameters where constants would otherwise appear. This is useful when you need to prepare multiple models that differ only in constants.

```{code-cell} ipython3
from ommx.v1 import DecisionVariable, Parameter, Instance, ParametricInstance

v = [10, 13, 18, 31, 7, 15]  # value of each item
w = [11, 25, 20, 35, 10, 33]  # weight of each item
N = len(v)

x = [
    DecisionVariable.binary(
        id=i,
        name="x",
        subscripts=[i],
    )
    for i in range(N)
]

capacity = Parameter(N, name="capacity")

pi = ParametricInstance.from_components(
    decision_variables=x,
    parameters=[capacity],
    objective=sum(v[i] * x[i] for i in range(N)),
    constraints={
        0: (sum(w[i] * x[i] for i in range(N)) <= capacity).add_name("weight limit")
    },
    sense=Instance.MAXIMIZE,
)
```

(experiment-management-attachable-data-formats)=
### Attachable Data Formats

The {py:class}`~ommx.v1.ParametricInstance` above is the OMMX-form mathematical model passed to solvers. To make the experiment easier to inspect later, you can also attach surrounding data such as the original modeling object or input files to the Experiment.

If the original model was written in a modeling package, keep that source model as an Attachment as well. For external payload types, OMMX defines only the attachment codec protocol and the `log_with_codec` / `get_with_codec` methods that invoke it. The concrete codec should live in the package that owns the object type. This tutorial defines a temporary `ProblemCodec` for JijModeling `Problem`; JijModeling is expected to provide an equivalent codec in the future.

```{code-cell} ipython3
import jijmodeling as jm


class ProblemCodec:
    media_type = "application/vnd.jijmodeling.problem+protobuf"

    @staticmethod
    def encode(problem: jm.Problem) -> bytes:
        return problem.to_protobuf()

    @staticmethod
    def decode(data: bytes) -> jm.Problem:
        return jm.Problem.from_protobuf(data)


@jm.Problem.define("Knapsack Problem", sense=jm.ProblemSense.MAXIMIZE)
def jij_problem(problem: jm.DecoratedProblem):
    N = problem.Length(description="Number of items")
    W = problem.Float(description="Capacity")
    w = problem.Float(shape=N, description="Weight of each item")
    v = problem.Float(shape=N, description="Value of each item")
    x = problem.BinaryVar(
        shape=N,
        description="Set x_i=1 iff item i is in the knapsack",
    )

    problem += jm.sum(v[i] * x[i] for i in N)
    problem += problem.Constraint(
        "weight limit",
        jm.sum(w[i] * x[i] for i in N) <= W,
    )
```

If the payload already exists as a file, attach that file directly instead. `log_file` copies the file bytes into the Experiment, and later readers can use `get_blob` to read the bytes or `write_attachment` to restore the file to disk. This is the usual path for Excel workbooks, solver logs, generated plots, and other files produced outside OMMX.

```python
import io
from pathlib import Path

experiment.log_file("input-spreadsheet", "input.xlsx")

spreadsheet_file = io.BytesIO(loaded_experiment.get_blob("input-spreadsheet"))
# Pass `spreadsheet_file` to a library that accepts a binary file-like object.
Path("restored").mkdir(parents=True, exist_ok=True)
loaded_experiment.write_attachment("input-spreadsheet", "restored/input.xlsx")
```

## Run the Experiment

This time, solve the knapsack problem above with two different capacities.

```{code-cell} ipython3
from ommx.experiment import Experiment
from ommx_highs_adapter import OMMXHighsAdapter

# Start an experiment. If no name is specified, one is assigned automatically.
with Experiment() as experiment:
    # Store the model as experiment-level information.
    experiment.log_parametric_instance("instance", pi)

    # Store the original JijModeling Problem through the temporary codec defined above.
    experiment.log_with_codec(
        ProblemCodec,
        "jijmodeling-problem",
        jij_problem,
    )

    # This example does not need it, but model metadata can also be stored as JSON.
    experiment.log_json(
        "source-data",
        {
            "description": "knapsack demo",
            "values": v,
            "weights": w,
        },
    )

    # Create two Runs with different capacities.
    for c in [47, 56]:
        # Materialize the model parameter.
        instance = pi.with_parameters({capacity.id: c})

        # Start a Run. A Run has setup and finalization, so using with is recommended.
        with experiment.run() as run:
            # Record capacity as a Run comparison parameter.
            run.log_parameter("capacity", c)

            # Call the HiGHS Adapter. The input Instance and output Solution are stored automatically.
            solution = run.log_solve(OMMXHighsAdapter, instance, verbose=False)

            # Confirm that the solver succeeded.
            assert solution.feasible

            # Also record the objective value as a Run comparison parameter.
            run.log_parameter("objective", solution.objective)

            # Leaving the with block finalizes the Run.

    # Leaving the experiment with block finalizes the Experiment.
```

All data stored during the experiment is saved in OMMX's *Local Registry*.

- The OMMX Local Registry is storage for efficiently keeping OMMX Artifact components. You can change its location with the `OMMX_LOCAL_REGISTRY_ROOT` environment variable. APIs such as {py:meth}`~ommx.experiment.Experiment.with_temp_local_registry` can create and use a temporary Local Registry.
- `log_json` and `log_solve` store data in the Local Registry immediately. They do not keep everything in memory and save it all at the end of the Experiment. Since storage paths are determined from the content of the data (SHA256 hash), identical data is stored only once per Local Registry.
- When the Experiment is finalized, OMMX stores JSON (the Artifact Manifest) that lists all data saved during the Experiment, and stores a tag in the Local Registry pointing to this Artifact Manifest under the Experiment name chosen at startup or generated automatically.

## Share the Experiment

To share an experiment, it needs a name that identifies it. The Experiment name can be specified at startup with `Experiment(name=...)`, or changed during or after the experiment with {py:meth}`~ommx.experiment.Experiment.rename`. If omitted, a default name is generated in the following format.

```text
bb040f6d.ommx.local/experiment:20260527T132713-e3c041e71f4b
|                              |               ^^^^^^^^^^^^ random string to prevent collisions
|                              ^^^^^^^^^^^^^^^ creation time (local time)
^^^^^^^^ Local Registry identifier
```

The default name contains `*.ommx.local`, so it cannot be pushed to an external container registry. It is mainly intended for temporary management. Some commands clean up Experiments with these default names, so assign an appropriate name for experiments you want to keep.

For example, to push an experiment to GitHub Container Registry (ghcr.io) and share it:

```python
# Name format: <container-registry>/<user>/<repository>:<tag>
experiment.rename("ghcr.io/jij-inc/ommx/tutorial/experiment:knapsack")

# Push to the container registry.
experiment.push()
```

Tutorial readers probably do not have permission to push to the OMMX repository, so replace the name as needed. OMMX delegates container-registry authentication to Docker, so you must log in to the registry with `docker login` beforehand.

### GitHub Container Registry

To be written.

### Google Cloud Artifact Registry

To be written.

### Export and Import as a File

You can also export an Experiment as a `.ommx` file without using a container registry. This is a supplementary way to hand off an experiment temporarily through file storage such as AWS S3.

```python
experiment.save("tutorial_experiment.ommx")
```

Import a received `.ommx` file into the Local Registry with {py:meth}`~ommx.experiment.Experiment.import_archive`, then open it.

```python
loaded_experiment = Experiment.import_archive(archive_path)
```

## Inspect a Shared Experiment

Since an Experiment is identified by name, a shared Experiment can be loaded by name with {py:meth}`~ommx.experiment.Experiment.load`.

```python
loaded_experiment = Experiment.load("ghcr.io/jij-inc/ommx/tutorial/experiment:knapsack")
```

This first searches the Local Registry by name. If it is not found, OMMX pulls it from the container registry, stores it in the Local Registry, and then loads it.

{py:meth}`~ommx.experiment.Experiment.load` and {py:meth}`~ommx.experiment.Experiment.import_archive` load an Experiment in the same state as an Experiment whose finalization has already completed. In this tutorial, we use the Experiment created above directly.

```{code-cell} ipython3
loaded_experiment = experiment
```

### Run Parameters

From a loaded Experiment, you can inspect the experiment information. First, {py:meth}`~ommx.experiment.Experiment.run_parameters_df` lists the parameters recorded with {py:meth}`~ommx.experiment.Run.log_parameter` for each Run as a `pandas.DataFrame`.

```{code-cell} ipython3
loaded_experiment.run_parameters_df()
```

For example, it should look like this.

```text
        capacity  objective
run_id
     0        47         41
     1        56         49
```

### Attachments

Experiment-level Attachments can be checked by name and retrieved by name. {py:meth}`~ommx.experiment.Experiment.get_attachment` checks the saved Media Type and returns JSON as a Python value, {py:class}`~ommx.v1.ParametricInstance` as that object, and so on. If you know the expected type, use type-specific methods such as {py:meth}`~ommx.experiment.Experiment.get_json` or {py:meth}`~ommx.experiment.Experiment.get_parametric_instance`; they raise an error if the Media Type does not match.

```{code-cell} ipython3
# Check the names of saved Attachments.
assert loaded_experiment.attachment_names == [
    "instance",
    "jijmodeling-problem",
    "source-data",
]

# Retrieve data saved as JSON.
source_data = loaded_experiment.get_json("source-data")
assert source_data == {
    "description": "knapsack demo",
    "values": v,
    "weights": w,
}

# get_attachment uses the Media Type to decode the payload.
pi = loaded_experiment.get_attachment("instance")
assert isinstance(pi, ParametricInstance)

# The codec validates the Media Type and decodes the original payload.
restored_jij_problem = loaded_experiment.get_with_codec(
    ProblemCodec,
    "jijmodeling-problem",
)
assert restored_jij_problem.name == jij_problem.name
```

### Runs and Solves

The list of Runs is available from {py:attr}`~ommx.experiment.Experiment.runs`. Finished Runs are ordered by creation time, and each Run exposes its Attachments and Solves.

If a Run was recorded with trace storage enabled, {py:attr}`~ommx.experiment.SealedRun.trace` returns the stored Run trace. Trace storage is an advanced feature; see {ref}`experiment-run-trace-storage` for details.

```{code-cell} ipython3
from typing import Any
from ommx.v1 import Solution

for run in loaded_experiment.runs:
    # Run IDs are assigned in execution order.
    assert run.run_id in [0, 1]

    # This example does not save run-level Attachments, so the count should be 0.
    assert len(run.attachment_names) == 0

    # Each Run calls the solver once, so the number of Solves should be 1.
    assert len(run.solves) == 1
    solve = run.solves[0]

    # Solve IDs are also assigned in execution order; here each Run has one Solve, so the ID should be 0.
    assert solve.solve_id == 0

    # Adapter name used for this Solve.
    assert solve.adapter.endswith("OMMXHighsAdapter")

    # Load input and output.
    input: Instance = solve.input
    output: Solution | None = solve.output
    assert output is not None

    # The knapsack problem should have been solved.
    assert output.feasible

    # Adapter options are also loaded.
    options: dict[str, Any] = solve.adapter_options
    assert "verbose" in options and options["verbose"] == False
```

## Fork an Experiment

Once an {py:class}`~ommx.experiment.Experiment` has been saved, it becomes immutable. You can still start a new Experiment from a saved Experiment. This operation is called a *Fork*. A forked Experiment inherits the same information as the original Experiment, but it starts again in an unfinalized running state, so you can add new Runs and Attachments. Use {py:meth}`~ommx.experiment.Experiment.fork` to fork an Experiment.

```{code-cell} ipython3
with loaded_experiment.fork() as forked_experiment:
    # The forked Experiment inherits existing Runs, so the new Run ID starts from 2.
    with forked_experiment.run() as run:
        assert run.run_id == 2

        c = 64
        instance = pi.with_parameters({capacity.id: c})

        run.log_parameter("capacity", c)
        solution = run.log_solve(OMMXHighsAdapter, instance, verbose=False)
        assert solution.feasible
        run.log_parameter("objective", solution.objective)
```

The original Experiment is not modified. The forked Experiment contains the original Runs plus the newly added Run.

```{code-cell} ipython3
assert list(loaded_experiment.run_parameters_df().index) == [0, 1]
assert list(forked_experiment.run_parameters_df().index) == [0, 1, 2]

forked_df = forked_experiment.run_parameters_df()
assert forked_df.loc[2, "capacity"] == 64
```

A forked Experiment inherits Solve and Attachment data, but the data itself is stored in the Local Registry based on its content. Forking does not duplicate that data. Only the Artifact Manifest, which lists the stored data, is duplicated, and the forked Experiment points to the same data as the original Experiment.

When you share a forked Experiment with {py:meth}`~ommx.experiment.Experiment.save` or {py:meth}`~ommx.experiment.Experiment.push`, what you share is the entire forked Experiment. Attachments, Runs, and Solves inherited from the original Experiment are also included in the forked Artifact's `layers`, so reading the forked Experiment does not require the original Experiment.
