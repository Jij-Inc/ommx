# What is OMMX?

OMMX (Open Mathematical prograMming eXchange) lets you describe **what you want to solve** separately from **how you solve it**. You can revise the formulation you send to a solver and still judge its candidates by the constraints and objective of the original problem. Connecting results back to those conditions helps you understand what to revisit in your requirements or input data.

A modeling tool expresses your requirements. OMMX provides a common data format and Python and Rust SDKs to transform the problem and evaluate candidates. An adapter connects that representation to the solver or sampler that searches for candidates and interprets its reports as OMMX results about the problem actually solved. Implementing transformations on this common representation makes them reusable across modeling tools and solving methods.

## Why use OMMX today?

Until the 3.0.0 stable release, both this Book and the API Reference describe `main`. The [installation guide](./getting_started/install.md) covers the latest published prerelease and installation from `main`; use `main` for features that have not been released yet. The current implementation supports the following tasks on a common problem representation.

| Your goal | What OMMX can handle | What you decide |
|---|---|---|
| Try a different formulation or solver | Supported special-constraint conversions and integer encodings; exchange with backends through adapters | The method, permitted transformations, and solver settings |
| Check whether a candidate is usable | Evaluate variable domains, constraints, and the objective; reconstruct original variable values from information retained by supported transformations | How to model the requirements of your application and whether to accept the candidate |
| Revisit requirements or input data using results | Identify objects by labels and retain the provenance of ordinary constraints generated from special constraints | How the modeler or application maps objects to source declarations and data, and what to revise |
| Pass problems and results to another process | Read and write a common format; save and share Artifacts; record settings and results in Experiments | What to record and what to compare |

The problem can come from a modeling tool, a saved file, or the SDK. If the same model structure serves several sets of input values, a {class}`~ommx.ParametricInstance` lets you supply the parameters for each case.

You choose the requirements the model must satisfy, including when an AI tool helps write the model. You can use the SDK just to check a candidate from your own algorithm.

## Evaluate a candidate you already have

A production line can make up to 100 units per hour, but only 60 in reduced mode. A plan that calls for 65 units in an hour of reduced operation violates that limit, whether the plan came from a solver or from a colleague. OMMX can evaluate either plan against the same problem. [Evaluate candidates and interpret results](./book/evaluate_and_interpret.md) explains how to distinguish checking this rule, checking the whole model, and establishing optimality. It also explains conclusions when no candidate is returned and how to connect results to the original conditions and data for revision. The {class}`Solution Reference <ommx.Solution>` shows how to evaluate a candidate and inspect the result.

## Separate a problem's meaning from its representation for solving

You can express the reduced-mode limit as an **indicator constraint**, which applies a condition when a binary variable takes a specified value. The {class}`PySCIPOpt Adapter <ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter>` can pass this linear indicator directly to SCIP. You can also use the known upper bound on output to express the same rule as an ordinary inequality and try that formulation with SCIP. In either case, the planner specifies the production rule once. [Transform a problem and connect it to a solving method](./book/transform_and_solve.md) derives the inequality and explains which guarantees to check for other transformations.

The development direction is to extend the structures you can declare beyond today's special constraints to evaluations such as assignment costs or route lengths, and to objectives composed of several evaluation axes. For example, the aim is to retain cost, delay, and shortage counts as individual objectives, choose a weighted sum or lexicographic ordering to suit the requirements, and evaluate results on each original objective. A customer's stated priorities remain requirements that this choice must satisfy. This planned support for objective structure and its realization is distinct from today's evaluation of additional metrics with {class}`~ommx.NamedFunction`.

In this direction, users, AI tools, and modelers clarify natural-language intent, while shared transformations expand the agreed declarations into mathematical formulations. Connecting modelers such as JijModeling to OMMX should reduce the work of having people or AI tools construct and check that expansion separately for each application.

## Accumulate analysis and transformation techniques on a common representation

Implementing preprocessing and transformations against a common representation lets modeling tools and solving methods reuse them. A new method can connect its inputs and outputs to that representation and benefit from the techniques already available. This is why OMMX develops both data exchange and shared processing of that data.

Today, OMMX supplies specific operations and policies for selecting supported transformations and evaluating candidates. For example, it can {meth}`tighten variable bounds using constraints <ommx.Instance.tighten_bounds_simultaneously_once>`, but this operation performs one update; it does not automatically optimize an entire preprocessing workflow.

The development direction separates analyses that inspect a problem's properties from transformations that rewrite it, and shares the reuse of analysis results and the selection and repetition of transformations. Users would choose the guarantees they need; the processing system would compose the guarantees of individual transformations to interpret results in the original problem. This should also reduce the work of checking each combination manually. The {ref}`end of the transformation chapter <shared-analysis-and-transformations>` connects today's operations to this direction.

## Start with your purpose

This introduction explains why you might adopt OMMX. The other three Book chapters can each be read on their own.

| What do you need to decide? | Book chapter |
|---|---|
| Can the solver use my model directly? If I change its formulation, which properties must I preserve? | [Transform a problem and connect it to a solving method](./book/transform_and_solve.md) |
| What can I conclude from a candidate, its absence, or a solver report? How do results connect back to requirements and data? | [Evaluate candidates and interpret results](./book/evaluate_and_interpret.md) |
| What should I keep so someone else can inspect a result or repeat a trial? | [Save and share problems, results, and trials](./book/record_and_share.md) |

To run something yourself, [install the SDK and any adapter you need](./getting_started/install.md). Choose an example when you want to follow a whole task, such as [sampling and plotting routes for a traveling salesperson](./examples/tsp_sampling_with_openjij_adapter).

For an individual operation, start in the {doc}`Python API Reference <api/index>`. For example, {class}`~ommx.Solution` shows how to inspect a candidate and {func}`~ommx.qplib.load_solution` reads one from a file. The {doc}`adapter catalogue <api/adapters>` lists solver inputs and links to complete solve examples. To connect your own backend, see {class}`~ommx.adapter.SolverAdapter` or {class}`~ommx.adapter.SamplerAdapter`.

To inspect a solver's search progress and termination report, see the {class}`SCIP <ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer>` or {class}`HiGHS <ommx_highs_adapter.HighsDiagnosticsAnalyzer>` diagnostic reference.

To work with saved data, [inspect a local Artifact](./how_to/manage_local_registry.md) or [share it through a registry](./how_to/share_artifact_registry.md). To connect an external tool, [install an agent plugin](./how_to/agent_skills.md) or see {mod}`ommx.tracing` to send spans to an OpenTelemetry collector.

## Support data exchange and sharing

Giving problems and results common meanings helps both software and people work with the same data. OMMX supports these handoffs through the following components:

| Component | Role |
|---|---|
| OMMX Message | A language- and OS-independent data format for problems and results |
| OMMX SDK | Construction, evaluation, transformation, reading, and writing of problems and results |
| OMMX Adapter | Connection between OMMX and a solver's inputs and outputs, and interpretation of reports about the problem solved |
| OMMX Artifact | A package for saving and sharing problems, results, and attachments with metadata |

Message schemas are defined with [Protocol Buffers](https://protobuf.dev/), with SDKs provided for Python and Rust. In Python user code, use the public API, such as `from ommx import Instance`. The [schema sources](https://github.com/Jij-Inc/ommx/tree/main/proto/ommx) define the wire formats; a [generated v1 Message Schema](https://jij-inc.github.io/ommx/protobuf.html) is also available.

```{figure} ./assets/introduction_02.png
:alt: Multiple software tools connected through a common OMMX representation
:width: 70%

Connect software through a common representation of problems and results.
```

Alongside problems and results, Artifacts can record what was solved and with which settings. They use [OCI standards](https://opencontainers.org/), so they can also be distributed through existing registries such as GitHub Container Registry. OMMX uses this mechanism to publish MIPLIB and QPLIB problems. The [Artifact format specification](https://github.com/Jij-Inc/ommx/blob/main/rust/ommx/doc/artifact_design.md) defines manifests, layers, and media types for tools that need to read or write the format directly.

```{toctree}
:caption: Understanding and using OMMX
:maxdepth: 1
:hidden:

book/transform_and_solve
book/evaluate_and_interpret
book/record_and_share
```

```{toctree}
:caption: Set up your environment
:maxdepth: 1
:hidden:

getting_started/install
how_to/cli
```

```{toctree}
:caption: Work through examples
:maxdepth: 1
:hidden:

examples/tsp_sampling_with_openjij_adapter
```

```{toctree}
:caption: Inspect and share Artifacts
:maxdepth: 1
:hidden:

how_to/manage_local_registry
how_to/share_artifact_registry
```

```{toctree}
:caption: Connect external tools
:maxdepth: 1
:hidden:

how_to/agent_skills
```

```{toctree}
:caption: Look up APIs and formats
:maxdepth: 1
:hidden:

api/index
Rust API (published releases) <https://docs.rs/ommx/>
Message schemas <https://github.com/Jij-Inc/ommx/tree/main/proto/ommx>
Artifact format specification <https://github.com/Jij-Inc/ommx/blob/main/rust/ommx/doc/artifact_design.md>
```

```{toctree}
:caption: Migrate between SDK versions
:maxdepth: 1
:hidden:

migration/python_sdk_v2_to_v3
migration/python_sdk_v1_to_v2
```

```{toctree}
:caption: Changes by release
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
