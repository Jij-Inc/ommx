# What is OMMX?

OMMX (Open Mathematical prograMming eXchange) is an open project that **defines solver-independent
semantics for optimization problems and candidates, and provides reusable operations on that common model**.
It connects problem description, evaluation, transformation, value reconstruction, and execution backends.

[What OMMX lets you reuse](getting_started/overview.md) explains shared work and the responsibilities
of modelers, OMMX, and Adapters. Start with [Versions and environments](getting_started/compatibility.md)
to install only what you need.

## Start from your goal

| Goal | First entry and useful stopping point |
|---|---|
| Prepare a problem and solve or sample it | [Build and solve a small problem](tutorial/solve_with_ommx_adapter), [load an existing problem](how_to/load_existing_instance.md), or [choose an Adapter](user_guide/supported_ommx_adapters.md). Go directly to a known Adapter's workflow |
| Evaluate an existing problem and candidate | [Evaluate an existing problem and candidate](tutorial/evaluate_existing_candidate). Load files and inspect objective values, feasibility, and failed conditions |
| Transform or inspect a problem | [Transform an Instance and inspect results](how_to/transform_instance_without_solver.md). Specify representation conditions and a Policy, then transform and evaluate without calling a backend |
| Record trials or exchange data | [Record trials with Experiment](tutorial/experiment_management) or [exchange Artifact files](tutorial/share_in_ommx_artifact.md). Choose only the workflow needed for your task |
| Connect a solver or another tool/language | [Implement an Adapter](developer_guide/adapter.md) or [connect another tool/language](developer_guide/integration.md). Reach Rust APIs and format specifications from common concepts |

Tutorials teach one small success; How-tos complete independent tasks. Guides explain common concepts
and result interpretation, while Reference provides exact operation conditions, formulas, and errors.
You do not need to read every page in order. Artifact and Experiment are not prerequisites
for constructing, evaluating, transforming, or executing a problem.

## Evaluating a candidate is different from finding one

For example, describe a problem in which two items cannot be selected together and check a candidate.

```python
from ommx import Instance

instance = Instance.maximize()
x = instance.new_binary("x")
y = instance.new_binary("y")
instance.objective = x + y
instance.add_constraint(x * y == 0)

solution = instance.evaluate({x.id: 1, y.id: 0})
assert solution.feasible
assert solution.objective == 1
```

`Instance` is the problem, an assignment is a candidate, and `Solution` is its evaluation result.
To search for better candidates, pass the problem to a backend through an Adapter.
Read feasibility, objective value, and optimality separately.
The [Solution Guide](user_guide/solution.md) explains the common evaluation model;
[SampleSet](user_guide/sample_set.md) covers several candidates.

## Transform into the representation you need

Backends accept different expressions, variable kinds, and constraints directly. OMMX separates a common
problem description defining condition semantics from the representation passed to an execution backend.
For example, a “choose exactly one” special constraint can be passed directly to a supporting backend
or lowered to an ordinary equality when needed.

For a first encounter with Adapters, these small examples introduce the workflow progressively:

1. [Solve without conversion using PySCIPOpt](tutorial/solve_with_ommx_adapter).
2. [Solve Indicator and SOS1 constraints directly](tutorial/solve_special_constraints_with_pyscipopt_adapter.md).
3. [Prepare special constraints for HiGHS, which does not handle them directly](tutorial/prepare_instance_for_adapter.md).

This sequence is not a prerequisite for every reader. For OpenJij, go directly to
[Sample a small problem](how_to/sample_with_openjij_adapter).
[TSP](tutorial/tsp_sampling_with_openjij_adapter) is an optional application.

Successful transformation, original-variable reconstruction, and original feasibility or optimality
are different guarantees. Consult [Transformation guarantees](user_guide/transformation_guarantees.md)
and [Transformed results](user_guide/removed_constraints.md) when you need those distinctions.

```{toctree}
:caption: Getting started
:maxdepth: 1
:hidden:

getting_started/overview
getting_started/compatibility
getting_started/install
```

```{toctree}
:caption: Tutorials / Solve a problem
:maxdepth: 1
:hidden:

tutorial/solve_with_ommx_adapter
tutorial/solve_special_constraints_with_pyscipopt_adapter
tutorial/prepare_instance_for_adapter
```

```{toctree}
:caption: Tutorials / Evaluate candidates
:maxdepth: 1
:hidden:

tutorial/evaluate_existing_candidate
```

```{toctree}
:caption: Tutorials / Record and exchange data
:maxdepth: 1
:hidden:

tutorial/experiment_management
tutorial/share_in_ommx_artifact
```

```{toctree}
:caption: Guides / Problems and evaluation
:maxdepth: 1
:hidden:

user_guide/instance
user_guide/function
user_guide/special_constraints
user_guide/solution
user_guide/sample_set
user_guide/variable_roles
user_guide/parametric_instance
```

```{toctree}
:caption: Guides / Transformations and solver integration
:maxdepth: 1
:hidden:

user_guide/transformation_guarantees
user_guide/removed_constraints
user_guide/instance_class
user_guide/preparation_policy
user_guide/supported_ommx_adapters
```

```{toctree}
:caption: Guides / Recording and sharing
:maxdepth: 1
:hidden:

user_guide/experiment
user_guide/artifact
```

```{toctree}
:caption: How-to / Prepare problems
:maxdepth: 1
:hidden:

how_to/load_existing_instance
how_to/transform_instance_without_solver
how_to/customize_preparation
```

```{toctree}
:caption: How-to / Use Adapters and inspect results
:maxdepth: 1
:hidden:

how_to/sample_with_openjij_adapter
how_to/compare_adapters
how_to/inspect_adapter_diagnostics
how_to/use_initial_state
how_to/trace_execution
tutorial/tsp_sampling_with_openjij_adapter
```

```{toctree}
:caption: How-to / Operate on recorded data
:maxdepth: 1
:hidden:

how_to/continue_experiment
how_to/share_artifact_registry
how_to/manage_local_registry
how_to/attach_experiment_data
```

```{toctree}
:caption: Developer guides
:maxdepth: 1
:hidden:

developer_guide/adapter
developer_guide/integration
```

```{toctree}
:caption: Reference
:maxdepth: 2
:hidden:

reference/index
api/index
```

```{toctree}
:caption: Migration and release information
:maxdepth: 1
:hidden:

migration/python_sdk_v2_to_v3
migration/python_sdk_v1_to_v2
release_note/ommx-3.0
release_note/ommx-2.5
release_note/ommx-2.4
release_note/ommx-2.3
release_note/ommx-2.2
release_note/ommx-2.1
release_note/ommx-2.0
release_note/ommx-1.9
release_note/ommx-1.8
release_note/ommx-1.7
release_note/ommx-1.6
release_note/ommx-1.5
```

```{toctree}
:caption: Auxiliary tools
:maxdepth: 1
:hidden:

user_guide/agent_skills
```
