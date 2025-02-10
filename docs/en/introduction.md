# What is OMMX?

OMMX (Open Mathematical prograMming eXchange) is an open data format and SDK designed to simplify data exchange between software and people when applying mathematical optimization to real-world problems.

## Data Exchange in Mathematical Optimization

When applying mathematical optimization to practical use cases, a large amount of data is often generated, requiring both effective management and sharing. Unlike the research phase of optimization, the application phase is divided into multiple stages, each necessitating specialized tools. Consequently, data must be converted to formats appropriate for each tool, making the overall process increasingly complex. By establishing one common format, it becomes easier to integrate multiple tools through a single conversion path to and from that format.

```{figure} ./assets/introduction_01.png
:alt: Overview of the mathematical optimization workflow

Mathematical optimization workflow.
```
Moreover, these tasks are typically carried out by separate individuals and teams, requiring data handoffs. Metadata is critical in these handoffs to clarify the data’s meaning and intention. For example, if a solution file for an optimization problem lacks details regarding which problem was solved, which solver was used, or what settings were chosen, the file cannot be reused or validated effectively. Standardized metadata helps streamline collaboration and data handling.

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

OMMX Message is a data format defined with [Protocol Buffers](https://protobuf.dev/) to ensure language-agnostic and OS-independent data exchange. It encapsulates schemas for optimization problems ([`ommx.v1.Instance`](./ommx_message/instance.ipynb)) and solutions ([`ommx.v1.Solution`](./ommx_message/solution.ipynb)). Protocol Buffers allow automatic generation of libraries in many languages, which OMMX SDK provides, especially for Python and Rust.

Data structures such as `ommx.v1.Instance` are called Messages, and each Message has multiple fields. For example, `ommx.v1.Instance` has the following fields (some are omitted for simplicity):

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

Messages such as `ommx.v1.DecisionVariable` representing decision variables and `ommx.v1.Function` representing mathematical functions used as objective functions and constraints are defined under the namespace `ommx.v1`. A list of Messages defined in OMMX is summarized in [OMMX Message Schema](https://jij-inc.github.io/ommx/protobuf.html).

Some solvers can directly read `ommx.v1.Instance`. For those that cannot, OMMX Adapters can be used to convert OMMX Message data into formats the solvers can handle. This makes it simpler to integrate various tools that support OMMX.

```{figure} ./assets/introduction_02.png
:alt: Diagram showing the relationship between OMMX Message and OMMX Adapter
:width: 70%

Data exchange between software realized by OMMX.
```

### OMMX Artifact

OMMX Artifact is a metadata-rich package format based on the [OCI (Open Container Initiative)](https://opencontainers.org/) standard. An OCI Artifact manages its content as layers and a manifest, assigning a specific [Media Type](https://www.iana.org/assignments/media-types/media-types.xhtml) to each layer. OMMX defines its own Media Types (e.g., `application/org.ommx.v1.instance`), and when these formats are included in OCI Artifacts, they are called OMMX Artifacts.

In OCI Artifact, the contents of the package are managed in units called layers. A single container contains multiple layers and metadata called a Manifest. When reading a container, the Manifest is first checked, and the necessary data is extracted by reading the layers based on that information. Each layer is saved as binary data (BLOB) with metadata called [Media Type](https://www.iana.org/assignments/media-types/media-types.xhtml). For example, when saving a PDF file, the Media Type `application/pdf` is attached, so software reading OCI Artifacts can recognize it as a PDF file by looking at the Media Type.

One major benefit of OCI Artifact compatibility is that standard container registries, such as [DockerHub](https://hub.docker.com/) or [GitHub Container Registry](https://docs.github.com/ja/packages/working-with-a-github-packages-registry/working-with-the-container-registry), can be used to store and distribute data. OMMX uses this mechanism to share large datasets like [MIPLIB 2017](https://miplib.zib.de/), made available at [GitHub Container Registry](https://github.com/Jij-Inc/ommx/pkgs/container/ommx%2Fmiplib2017). For additional details, see [Download MIPLIB Instances](./tutorial/download_miplib_instance.md).

```{figure} ./assets/introduction_03.png
:alt: Diagram showing the relationship between OMMX Message and OMMX Artifact

Data exchange between humans realized by OMMX.
```
