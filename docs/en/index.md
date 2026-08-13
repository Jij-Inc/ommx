# What is OMMX?

OMMX (Open Mathematical prograMming eXchange) is an open data format and SDK designed to simplify data exchange between software and people when applying mathematical optimization to real-world problems.

## Choose a Path by Goal

Adapter users and Adapter authors need different knowledge and should follow different paths through the documentation. Each Tutorial introduces one new workflow at a time. The User Guide explains mathematical concepts, the responsibility boundary between users and Adapters, and the `Instance` state model. Precise contracts for conversion formulas, preconditions, generated artifacts, errors, and atomicity belong in the API Reference.

### Solve Problems with OMMX Adapters

| Step | Read | What you learn | Deferred until later |
|---|---|---|---|
| 1 | [Solve a 0-1 Knapsack Problem with the PySCIPOpt Adapter](./tutorial/solve_with_ommx_adapter) | Pass an `Instance` directly to `solve()` and inspect the result with pandas | Special constraints, `INPUT_CLASS`, and Preparation |
| 2 | [Solve Special Constraints Directly with the PySCIPOpt Adapter](./tutorial/solve_special_constraints_with_pyscipopt_adapter) | What Indicator and SOS1 constraints mean, and that a supporting Adapter needs no conversion | The exact-input definition and the formulas and preconditions for lowering |
| 3 | [Prepare an Instance for an Adapter](./tutorial/prepare_instance_for_adapter) | Check `INPUT_CLASS`, obtain the recommended Policy, copy the model, and call `prepare()` yourself | Phase ordering, responsibility boundaries, failure state, and exact conversion contracts |
| 4 | [Sample with the OpenJij Adapter](./tutorial/tsp_sampling_with_openjij_adapter) | Treat QUBO conversion as Preparation, choose a penalty, and check feasibility | Every Policy field, the expanded penalty formula, and individual conversion APIs |
| 5 | [Compare Results Across Adapters](./tutorial/switching_adapters) | Reuse the same `Instance` when it is an exact input for every Adapter; otherwise prepare a copy per Adapter | Adapter-specific features such as diagnostics and initial states |

### Implement an OMMX Adapter

Adapter authors do not need to read every Tutorial first. Start with [Developer Guide: Implement an OMMX Adapter](./developer_guide/adapter), review the following three shared contracts and their API references, and use the PySCIPOpt Adapter as the reference implementation for Solver Adapters.

1. [Adapter Exact Inputs (`INPUT_CLASS`)](./user_guide/adapter_input_class)
2. [Instance Preparation and `PreparationPolicy`](./user_guide/preparation_policy)
3. [Removed Constraints and Feasibility](./user_guide/removed_constraints)

## Data Exchange in Mathematical Optimization

When applying mathematical optimization to practical use cases, a large amount of data is often generated, requiring both effective management and sharing. Unlike the research phase of optimization, the application phase is divided into multiple stages, each necessitating specialized tools. Consequently, data must be converted to formats appropriate for each tool, making the overall process increasingly complex. By establishing one common format, it becomes easier to integrate multiple tools through a single conversion path to and from that format.

```{figure} ./assets/introduction_01.png
:alt: Overview of the mathematical optimization workflow

Mathematical optimization workflow.
```
Moreover, these tasks are typically carried out by separate individuals and teams, requiring data handoffs. Metadata is critical in these handoffs to clarify the data's meaning and intention. For example, if a solution file for an optimization problem lacks details regarding which problem was solved, which solver was used, or what settings were chosen, the file cannot be reused or validated effectively. Standardized metadata helps streamline collaboration and data handling.

## Components of OMMX

To address these data exchange challenges, OMMX was developed. It consists of four main components:

- OMMX Message  
  A data format, independent of programming languages and OS, for exchanging information among software

- OMMX Artifact  
  A package format with metadata that is convenient for exchanging data among people

- OMMX SDK  
  A framework for efficiently creating and manipulating OMMX Messages and OMMX Artifacts

- OMMX Adapters  
  Tools for converting between solver-specific formats and OMMX

### OMMX Message

OMMX Message is a data format defined with [Protocol Buffers](https://protobuf.dev/) to ensure language-agnostic and OS-independent data exchange. It encapsulates schemas for optimization problems (`ommx.v1.Instance` on the protobuf wire format, [`ommx.Instance`](./user_guide/instance.md) in the Python SDK) and solutions (`ommx.v1.Solution` on the wire format, [`ommx.Solution`](./user_guide/solution.md) in the Python SDK). Protocol Buffers allow automatic generation of libraries in many languages, which OMMX SDK provides, especially for Python and Rust.

Data structures such as the protobuf `ommx.v1.Instance` schema are called Messages, and each Message has multiple fields. For example, `Instance` has the following fields (some are omitted for simplicity):

```protobuf
message Instance {
  // Decision variables
  repeated DecisionVariable decision_variables = 2;
  // Objective function
  Function objective = 3;
  // Constraints
  repeated Constraint constraints = 4;
  // Maximization or minimization
  Sense sense = 5;
}
```

Messages such as `ommx.v1.DecisionVariable` representing decision variables and `ommx.v1.Function` representing mathematical functions used as objective functions and constraints are defined under the protobuf namespace `ommx.v1`. In Python user code, import the corresponding SDK domain classes from top-level `ommx`, for example `from ommx import Instance, DecisionVariable, Function`. A list of Messages defined in OMMX is summarized in [OMMX Message Schema](https://jij-inc.github.io/ommx/protobuf.html).

Some solvers can directly read OMMX `Instance` payloads. For those that cannot, OMMX Adapters can be used to convert OMMX Message data into formats the solvers can handle. This makes it simpler to integrate various tools that support OMMX.

```{figure} ./assets/introduction_02.png
:alt: Diagram showing the relationship between OMMX Message and OMMX Adapter
:width: 70%

Data exchange between software realized by OMMX.
```

### OMMX Artifact

OMMX Artifact is a metadata-rich package format based on the [OCI (Open Container Initiative)](https://opencontainers.org/) standard. An OCI Artifact manages its content as layers and a manifest, assigning a specific [Media Type](https://www.iana.org/assignments/media-types/media-types.xhtml) to each layer. OMMX defines its own Media Types (e.g., `application/org.ommx.v1.instance`), and when these formats are included in OCI Artifacts, they are called OMMX Artifacts.

In OCI Artifact, the contents of the package are managed in units called layers. A single container contains multiple layers and metadata called a Manifest. When reading a container, the Manifest is first checked, and the necessary data is extracted by reading the layers based on that information. Each layer is saved as binary data (BLOB) with metadata called [Media Type](https://www.iana.org/assignments/media-types/media-types.xhtml). For example, when saving a PDF file, the Media Type `application/pdf` is attached, so software reading OCI Artifacts can recognize it as a PDF file by looking at the Media Type.

One major benefit of OCI Artifact compatibility is that standard container registries, such as [DockerHub](https://hub.docker.com/) or [GitHub Container Registry](https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry), can be used to store and distribute data. OMMX uses this mechanism to distribute representative benchmark sets such as [MIPLIB 2017](https://miplib.zib.de/) and [QPLIB](https://qplib.zib.de/) through [GitHub Container Registry](https://github.com/orgs/Jij-Inc/packages?repo_name=ommx). For details, see [Download Benchmark Instances](./tutorial/download_benchmark_instance.md).

```{figure} ./assets/introduction_03.png
:alt: Diagram showing the relationship between OMMX Message and OMMX Artifact

Data exchange between humans realized by OMMX.
```

```{toctree}
:caption: Tutorial
:maxdepth: 1
:hidden:

tutorial/solve_with_ommx_adapter
tutorial/solve_special_constraints_with_pyscipopt_adapter
tutorial/prepare_instance_for_adapter
tutorial/tsp_sampling_with_openjij_adapter
tutorial/switching_adapters
tutorial/experiment_management
tutorial/share_in_ommx_artifact
tutorial/download_benchmark_instance
```

```{toctree}
:caption: User Guide
:maxdepth: 1
:hidden:

user_guide/supported_ommx_adapters
user_guide/adapter_input_class
user_guide/preparation_policy
user_guide/adapter_diagnostics
user_guide/adapter_initial_state
user_guide/function
user_guide/instance
user_guide/parametric_instance
user_guide/special_constraints
user_guide/removed_constraints
user_guide/solution
user_guide/sample_set
user_guide/experiment
user_guide/tracing
```

```{toctree}
:caption: Developer Guide
:maxdepth: 1
:hidden:

developer_guide/adapter
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
