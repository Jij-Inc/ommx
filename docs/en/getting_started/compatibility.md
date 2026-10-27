# Versions and environments

This Book's Python examples use public **`ommx==3.0.0b7`** as their common baseline.
Start with the SDK in [Installation](install.md), then add packages needed for your task.
The scope below was checked on macOS arm64 with Python 3.12.12. For wheel availability on
other operating systems or architectures, see the [published files](https://pypi.org/project/ommx/3.0.0b7/#files).

## Add dependencies by task

| Task | Additions to the SDK and required inputs | Not needed for the first success |
|---|---|---|
| Build and evaluate problems/candidates; transform Instances | Small models supplied in the Book | Adapter, solver, Experiment |
| Evaluate an existing candidate | QPLIB_0018 problem and candidate files; network access for the initial download | Building a model, Adapter |
| Obtain MIPLIB/QPLIB data | Network access to the public registry | Execution backend |
| First solve, special constraints, initial candidates, SCIP diagnostics | `ommx-pyscipopt-adapter==3.0.0b7` | Other Adapters |
| HiGHS Preparation and solving | `ommx-highs-adapter==3.0.0b7` | Installing PySCIPOpt or retaining earlier Tutorial runtime state |
| Compare Adapters | Both Adapters above, matplotlib | Experiment |
| Small OpenJij sampling workflow | `ommx-openjij-adapter==3.0.0b7`; weight specified in the example | SCIP, HiGHS |
| Optional TSP application | OpenJij environment, matplotlib | Earlier Tutorial runtime state |
| First tracing example | SDK and Notebook kernel; script APIs are also available | Experiment, Adapter |
| Record, save, and reopen an Experiment | SDK, the example's model and candidates, a file destination | Adapter, custom Codec, remote registry |
| Exchange an Artifact | SDK and self-contained example data; receiver uses a new process and empty Local Registry | Solver, sender's Registry, authentication |

When executing a Notebook, select the Python from this environment as its kernel. Pages focused on tables
or figures also store their execution results in `.ipynb` files. You do not need a Notebook environment
just to read the displayed outputs.

Principal additional package versions in these runs were pandas 2.3.3, PyArrow 22.0.0, matplotlib 3.11.2,
OpenJij 0.12.0, PySCIPOpt 6.3.0, and highspy 1.15.1. These record the environments used;
they do not restrict Adapter support to those versions.
Each page specifies cell order, external data, and necessary settings.

## Generated API Reference versus public packages

The API Reference is generated from this Book's development revision and can include features absent from public b7.
For example, named-State conversion and `used_decision_variable_ids` are separate from the public b7 examples.
Use the [public b7 source](https://github.com/Jij-Inc/ommx/tree/python-3.0.0b7)
when checking public API contracts against implementation. For development features, align the SDK,
Adapters, and Reference to the same revision.

## Python, Rust, and wire formats have separate versions

| Component | Reference baseline in this Book |
|---|---|
| Python SDK and same-repository Adapters | Public `3.0.0b7` examples; generated API from the Book's development revision |
| Rust SDK and standalone CLI | Public `3.0.0-beta.7`; versioned independently of Python |
| Message wire schema | `ommx.v1`/`ommx.v2` according to the payload's Media Type; not a Python import namespace |
| Artifact format | Manifest, Media Type, and archive specification; separate from contained Message versions |
| Dataset distributions | MIPLIB `v2.7` and QPLIB `v2.8`; not SDK versions |

For CLI use, follow the [standalone CLI procedure](../how_to/manage_local_registry.md).
You do not need to update the b7 Python environment just for CLI access. Authenticated registry sharing is
an optional operation using your own account and permissions, separate from the Book's local roundtrips.
[Reference](../reference/index.md) collects Rust/other-language entries and version-pinned specifications.
