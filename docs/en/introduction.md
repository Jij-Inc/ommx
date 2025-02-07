# What is OMMX?

OMMX (Open Mathematical prograMming eXchange) is an open data format and SDK designed to facilitate data exchange between software and people in the process of applying mathematical optimization to practical applications.

## Data Exchange in Mathematical Optimization

In the process of applying mathematical optimization techniques to practical applications, a lot of data is generated, and it is necessary to manage and share this data appropriately. Unlike the research process of mathematical optimization itself, the process of applying it to practical applications consists of multiple phases, as shown in the following figure, and it is necessary to use software appropriate for each task in each phase.

```{figure} ./assets/introduction_01.png
:alt: Overview of the mathematical optimization workflow

Mathematical optimization workflow.
```

For example, in data analysis, standard data science tools such as `pandas` and `matplotlib` are used, while in formulation, specialized tools for mathematical optimization such as `jijmodeling` and `JuMP` are used, and in optimization itself, solvers such as `Gurobi` and `SCIP` are used. Since these software handle data formats that are convenient for them, data conversion is necessary to operate them together. Such conversions become combinatorially complex as the number of tools increases. If there is a single standard data format, it is possible to connect with any tool by preparing mutual conversions with the standard data format for each tool, and overall efficiency can be greatly improved.

In addition, these tasks are generally divided among multiple people, and it is necessary to pass data between the responsible parties. In data exchange between humans, metadata is important to describe what the data represents and for what purpose it was created. For example, if the result of solving an instance of an optimization problem is saved as a file, it cannot be used for other purposes unless it is described which problem was solved by which solver with what settings. To solve this, it is necessary to attach metadata, but if the format of the metadata is not unified, data exchange becomes difficult.

## Components of OMMX

To solve these data exchange problems, OMMX was developed. OMMX consists of the following four components:

- OMMX Message
    
    A data format for exchanging data between software, independent of programming languages and OS
    
- OMMX Artifact
    
    A package format with metadata for exchanging data between humans
    
- OMMX SDK
    
    A framework for efficiently manipulating and generating OMMX Messages and OMMX Artifacts
    
- OMMX Adapters
    
    A group of software for mutual conversion between optimization software such as solvers and OMMX data formats
    

### OMMX Message

OMMX Message is a data format designed for exchanging data between software. By defining it using [Protocol Buffers](https://protobuf.dev/), it achieves a data format independent of programming languages and OS. OMMX Message defines schemas for representing data of optimization problems ([`ommx.v1.Instance`](./ommx_message/instance.ipynb)) and solutions ([`ommx.v1.Solution`](./ommx_message/solution.ipynb)).
Thanks to the functionality of Protocol Buffers, libraries for using OMMX Message can be automatically generated for most practical programming languages, and they are provided as part of the OMMX SDK, especially for Python and Rust.

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

Some solvers can directly read data defined by `ommx.v1.Instance`, but for solvers that cannot, OMMX Adapters are used to convert OMMX Messages into formats that solvers can handle. By creating OMMX Adapters for the necessary software, it is easy to connect with other software that works with OMMX.

```{figure} ./assets/introduction_02.png
:alt: Diagram showing the relationship between OMMX Message and OMMX Adapter
:width: 70%

Data exchange between software realized by OMMX.
```

### OMMX Artifact

OMMX Artifact is a package format with metadata designed for data exchange between humans. It is based on the OCI Artifact defined by the [OCI (Open Container Initiative)](https://opencontainers.org/), a standardization organization for containers (such as Docker). In OCI standardization, a container is realized by a regular Tar archive, and metadata such as the command to execute is saved along with the files inside the container. OCI Artifact is a specification for using this as a general-purpose package format.

In OCI Artifact, the contents of the package are managed in units called layers. A single container contains multiple layers and metadata called a Manifest. When reading a container, the Manifest is first checked, and the necessary data is extracted by reading the layers based on that information. Each layer is saved as binary data (BLOB) with metadata called [Media Type](https://www.iana.org/assignments/media-types/media-types.xhtml). For example, when saving a PDF file, the Media Type `application/pdf` is attached, so software reading OCI Artifacts can recognize it as a PDF file by looking at the Media Type.

In OMMX, Media Types such as `application/org.ommx.v1.instance` are defined for each OMMX Message, and OCI Artifacts containing binaries serialized as Protocol Buffers of OMMX Messages are called OMMX Artifacts. Strictly speaking, OMMX does not extend OCI Artifacts at all, so OMMX Artifacts can be treated as a type of OCI Artifact.

The advantage of using OCI Artifact as a package format is that it can be treated as a completely legitimate container. This means that [DockerHub](https://hub.docker.com/) and [GitHub Container Registry](https://docs.github.com/ja/packages/working-with-a-github-packages-registry/working-with-the-container-registry) can be used as they are for data management and distribution. Like many containers, it is easy to distribute benchmark sets that can be several gigabytes to an unspecified number of people. OMMX uses this feature to distribute the representative dataset [MIPLIB 2017](https://miplib.zib.de/) via [GitHub Container Registry](https://github.com/Jij-Inc/ommx/pkgs/container/ommx%2Fmiplib2017). For more details, see [Download MIPLIB Instances](./tutorial/download_miplib_instance.md).

```{figure} ./assets/introduction_03.png
:alt: Diagram showing the relationship between OMMX Message and OMMX Artifact

Data exchange between humans realized by OMMX.
```
