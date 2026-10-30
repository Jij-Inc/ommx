# What is OMMX?

OMMX (Open Mathematical prograMming eXchange) is an open project that aims to **make preprocessing and problem-transformation techniques reusable through a common representation of mathematical optimization problems**. Placing that representation between modeling tools and solvers lets different solving methods use the same transformation and evaluation machinery.

For example, developing a new solving method becomes harder if you must also reimplement problem conversion and candidate checking. Defining what a problem and a candidate mean independently of a solver lets those tasks be shared. OMMX provides a data format, Python and Rust SDKs for construction, evaluation, transformation, and storage, and adapters that connect to solvers.

## Why use OMMX today?

Until the 3.0.0 stable release, both this Book and the API Reference describe `main`. The [installation guide](./getting_started/install.md) covers the latest published prerelease and installation from `main`; use `main` for features that have not been released yet. The current implementation supports the following tasks on a common problem representation.

| Your goal | What OMMX can handle | What you decide |
|---|---|---|
| Try another solver or solving method | Supported special-constraint conversions and integer encodings; exchange with backends through adapters | The method, permitted transformations, and solver settings |
| Check whether a candidate is usable | Evaluate variable domains, constraints, and the objective; reconstruct original variable values from information retained by supported transformations | How to model the requirements of your application and whether to accept the candidate |
| Pass problems and results to another process | Read and write a common format; save and share Artifacts; record settings and results in Experiments | What to record and what to compare |

You can use only the parts you need. Evaluating a candidate from your own algorithm with the SDK requires neither an adapter nor an Artifact. Modeling turns application requirements into variables, objectives, and constraints; OMMX supplies common processing of that problem; a solver or sampler searches for candidates.

## Evaluate a candidate you already have

Variable values alone do not tell you what an optimization result means. You also need the variables' domains, the constraints they must satisfy, and the objective used for comparison. For example, you can describe a problem in which two items cannot both be selected and check a candidate:

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

This candidate satisfies the constraint and has objective value 1. You can check candidates against the same problem whether they come from your own algorithm, a solver, or a person. Evaluating whether a candidate is feasible and searching for a better candidate are different operations. Evaluation alone does not establish optimality.

## Separate a problem's meaning from its representation for solving

Solvers differ in which objectives and constraints they can handle directly. OMMX lets you change a problem's representation when needed instead of committing to one solver's representation at the outset.

For example, the requirement to select exactly one of three binary variables can be represented as a OneHot special constraint. If ordinary linear constraints are needed, it can be converted to `x[0] + x[1] + x[2] == 1`. Keeping the explicit "select exactly one" structure makes its meaning available to later analysis and transformations. In v3, special constraints are objects that can be constructed, evaluated, and transformed, just like regular constraints.

Which structures can remain explicit depends on the solving method. For supported models, the {class}`PySCIPOpt Adapter <ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter>` passes Indicator and SOS1 constraints directly to SCIP. For another backend, OMMX can {meth}`lower special constraints to regular constraints <ommx.Instance.lower_special_constraints>` when the conversion conditions hold. [Transform a problem and connect it to a solving method](./user_guide/transform_and_solve.md) explains how to choose between these paths.

What a transformation preserves depends on the operation. Some express the same condition in another form; others change the conditions imposed or the objective being compared. Feasibility, objective values, and optimality before and after a transformation are distinct guarantees. [Evaluate candidates and interpret results](./user_guide/evaluate_and_interpret.md) explains how to read transformed results. {meth}`~ommx.Instance.lower_special_constraints` covers supported conversions; individual operations' conditions and guarantees belong to the {doc}`API Reference <api/index>`.

## Accumulate transformation techniques on a common representation

Implementing preprocessing and transformations against a common representation lets modeling tools and solving methods reuse them. A new method can connect its inputs and outputs to that representation and benefit from the techniques already available. This is why OMMX develops both data exchange and shared processing of that data.

Today, OMMX supplies specific operations and policies for selecting supported transformations and evaluating candidates. For example, it can {meth}`tighten variable bounds using constraints <ommx.Instance.tighten_bounds_simultaneously_once>`, but this operation performs one update; it does not automatically optimize an entire preprocessing workflow.

The development direction is to combine these techniques and select or repeat them according to a problem's structure. Accumulating both individual techniques and the knowledge of when to apply them should let the shared foundation take on more of the preparation that people perform manually today.

## Start with your purpose

| Your task | Start here | What you will learn |
|---|---|---|
| Solve a problem with a solver | [Solve with an OMMX Adapter](./tutorial/solve_with_ommx_adapter.md) | Create an Instance, solve it, and read the result |
| Evaluate a candidate you already have | [Evaluate candidates and interpret results](./user_guide/evaluate_and_interpret.md) | Align the problem, objective, and conditions being evaluated; distinguish feasibility from optimality |
| Evaluate metrics beyond the objective | {class}`~ommx.NamedFunction` | Compute distance or other reporting metrics for each candidate separately from the objective |
| Extract results by name and subscript | {meth}`Named variable values <ommx.Solution.extract_decision_variables>` | Obtain evaluated variable values as dictionaries or tables for further processing |
| Understand the problem representation | {class}`~ommx.Instance` | Describe variables, objectives, and constraints |
| Construct and evaluate expressions | {class}`~ommx.Function` | Build scalar expressions and distinguish their values from a candidate's feasibility |
| Inspect expressions using variable labels | {meth}`Expression formatting <ommx.Instance.format_function>` | Read expressions using the names and subscripts held by an Instance |
| Change numeric conditions in a fixed problem structure | {class}`~ommx.ParametricInstance` | Reuse the structure and variable/constraint IDs with different parameter values |
| Use special constraints | {class}`~ommx.IndicatorConstraint`, {class}`~ommx.OneHotConstraint`, {class}`~ommx.Sos1Constraint` | Understand Indicator, OneHot, and SOS1 constraints and their solver integration |
| Transform a problem and choose a method | [Transform a problem and connect it to a solving method](./user_guide/transform_and_solve.md) | Preserve structure and distinguish input conditions, transformation guarantees, and settings |
| Tighten variable bounds from constraints | {meth}`Bound tightening <ommx.Instance.tighten_bounds_simultaneously_once>` | Perform one update and inspect the changed bounds |
| Compare multiple candidates | {class}`~ommx.SampleSet` | Read sample evaluations and select among the candidates obtained |
| Sample a QUBO with OpenJij | [TSP sampling example](./tutorial/tsp_sampling_with_openjij_adapter) | Follow a QUBO sampling application and inspect its results |
| Obtain existing problems | [MIPLIB](./tutorial/download_miplib_instance.md) or [QPLIB](./tutorial/download_qplib_instance.md) | Download a problem as an OMMX Instance |
| Record and compare experiments | [Experiment management](./tutorial/experiment_management.md) | Associate settings with results |
| Save and share problems and results | [Sharing an Artifact](./tutorial/share_in_ommx_artifact.md) | Package data together with metadata |
| Connect your own solving tool | [Implement an Adapter](./tutorial/implement_adapter.md) | Translate backend inputs and outputs through the common adapter interface |

This Book explains what OMMX can do, why and when to use it, what to choose, and how to interpret results. Use small examples to understand the meaning, then consult the {doc}`API Reference <api/index>` for detailed calls, arguments, preconditions, and guarantees. You do not need to read every page in order.

If you already have a problem and candidate values, begin with [Evaluate candidates and interpret results](./user_guide/evaluate_and_interpret.md). Use {class}`~ommx.Solution` for executable evaluation examples. Running an adapter tutorial is not a prerequisite.

## Use or implement an Adapter

After the setup above, start with the small knapsack model in [Solve with an OMMX Adapter](./tutorial/solve_with_ommx_adapter.md). The {doc}`Adapter packages and input contracts </api/adapters>` page lists the available integrations. Continue to {class}`~ommx.IndicatorConstraint`, {class}`~ommx.OneHotConstraint`, {class}`~ommx.Sos1Constraint` for constraints that a backend can handle directly, or {meth}`~ommx.Instance.lower_special_constraints` for conversions and their conditions.

To compare solvers, use [Solve with multiple adapters and compare the results](./tutorial/switching_adapters.md). To try sampling, use the [OpenJij TSP example](./tutorial/tsp_sampling_with_openjij_adapter). Each has its own model; you do not need the runtime state of a previous tutorial.

If you are connecting your own solving tool, the [adapter implementation guide](./tutorial/implement_adapter.md) explains how to translate its input and output through OMMX's common interface.

## Support data exchange and sharing

Giving problems and results common meanings helps both software and people work with the same data. OMMX supports these handoffs through the following components:

| Component | Role |
|---|---|
| OMMX Message | A language- and OS-independent data format for problems and results |
| OMMX SDK | Construction, evaluation, transformation, reading, and writing of problems and results |
| OMMX Adapter | Connection between OMMX and a solver's inputs and outputs |
| OMMX Artifact | A package for saving and sharing problems, results, and attachments with metadata |

Message schemas are defined with [Protocol Buffers](https://protobuf.dev/), with SDKs provided for Python and Rust. In Python user code, use the public API, such as `from ommx import Instance`. The [schema sources](https://github.com/Jij-Inc/ommx/tree/main/proto/ommx) define the wire formats; a [generated v1 Message Schema](https://jij-inc.github.io/ommx/protobuf.html) is also available.

```{figure} ./assets/introduction_02.png
:alt: Multiple software tools connected through a common OMMX representation
:width: 70%

Connect software through a common representation of problems and results.
```

Alongside problems and results, Artifacts can record what was solved and with which settings. They use [OCI standards](https://opencontainers.org/), so they can also be distributed through existing registries such as GitHub Container Registry. OMMX uses this mechanism to publish [MIPLIB](./tutorial/download_miplib_instance.md) and [QPLIB](./tutorial/download_qplib_instance.md) problems. The [Artifact format specification](https://github.com/Jij-Inc/ommx/blob/main/rust/ommx/doc/artifact_design.md) defines manifests, layers, and media types for tools that need to read or write the format directly.

```{toctree}
:caption: Get started
:maxdepth: 1
:hidden:

getting_started/install
```

```{toctree}
:caption: Tutorial
:maxdepth: 1
:hidden:

tutorial/solve_with_ommx_adapter
tutorial/tsp_sampling_with_openjij_adapter
tutorial/switching_adapters
tutorial/experiment_management
tutorial/share_in_ommx_artifact
tutorial/download_miplib_instance
tutorial/download_qplib_instance
tutorial/implement_adapter
```

```{toctree}
:caption: Transform problems and connect solvers
:maxdepth: 1
:hidden:

user_guide/transform_and_solve
```

```{toctree}
:caption: Evaluate candidates and interpret results
:maxdepth: 1
:hidden:

user_guide/evaluate_and_interpret
```

```{toctree}
:caption: User Guide
:maxdepth: 1
:hidden:

user_guide/cli
user_guide/agent_skills
user_guide/adapter_diagnostics
user_guide/adapter_initial_state
user_guide/experiment
user_guide/tracing
```

```{toctree}
:caption: API Reference
:maxdepth: 2
:hidden:

api/index
```

```{toctree}
:caption: Migration Guide
:maxdepth: 1
:hidden:

migration/python_sdk_v2_to_v3
migration/python_sdk_v1_to_v2
```

```{toctree}
:caption: Release Note
:maxdepth: 1
:hidden:

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
