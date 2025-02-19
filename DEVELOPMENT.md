README for Developers of OMMX
==============================

Taskfile
---------
This project uses [Taskfile](https://taskfile.dev/#/) to manage development tasks. See https://taskfile.dev/installation/ for installation.
You can get a list of available tasks by running `task -l` in the root directory of the project like below:

```text
$ task -l
task: Available tasks for this project:
* api_reference:build:                       Build the API Reference of Python SDK
* api_reference:default:                     Build and Open the API Reference of Python SDK      (aliases: api_reference)
* api_reference:open:                        Open the API Reference of Python SDK
* api_reference:pip_compile:                 Compile pyproject.toml into requirements.txt for Read the Docs
* book_en:build:                             Build the book in docs/en
* book_en:default:                           Build and open the book in docs/en      (aliases: book_en)
* book_en:open:                              Open the book in docs/en
* book_en:watch:                             Watch the book in docs/en
* book_ja:build:                             Build the book in docs/ja
* book_ja:default:                           Build and open the book in docs/ja      (aliases: book_ja)
* book_ja:open:                              Open the book in docs/ja
* book_ja:watch:                             Watch the book in docs/ja
* proto:default:                             Generate code from Protobuf definitions      (aliases: proto)
* proto:python:                              Generate Python code from Protobuf definitions
* proto:rust:                                Generate Rust code from Protobuf definitions
* python:default:                            Sync and Run tests for all Python projects      (aliases: python)
* python:doc:                                Build and Open API reference documentation
* python:format:                             Format Python code
* python:ommx-openjij-adapter:test:          Run tests for OMMX OpenJij Adapter
* python:ommx-pyscipopt-adapter:test:        Run tests for OMMX PySCIPOpt Adapter
* python:ommx-python-mip-adapter:test:       Run tests for OMMX Python-MIP Adapter
* python:ommx:pyright:                       Type check OMMX Python SDK
* python:ommx:pytest:                        Run pytest for OMMX Python SDK
* python:ommx:test:                          Run tests for OMMX Python SDK
* python:set-version:                        Set the version for all Python projects
* python:stubgen:                            Generate stubs files for Rust extension
* python:sync:                               Setup Python development environment, Rebuild Python SDK
* python:test:                               Run tests for all Python projects
* rust:check:                                Run check for Rust SDK
* rust:clippy:                               Run clippy for Rust SDK
* rust:doc:                                  Generate and Open documentation for Rust SDK
* rust:test:                                 Run tests for Rust SDK
```

> [!NOTE]
> When you run `task` command, it will automatically detect the `Taskfile.yml` from current directory or parent directories.
> If you run `task` command on `proto/` directory, `proto/Taskfile.yml` is used, i.e. commands only defined there are available.
> Each sub-directory `Taskfile.yml` are included in the parent `Taskfile.yml`,
> e.g. `task python` in `proto/` is exposed as `task proto:python` in the root directory.

Code Map
========

[`proto/`](./proto/)
--------------------
- OMMX Message schema is defined in `*.proto` files under this directory.
- Managed by [`buf`](https://buf.build/docs/introduction), see its configuration in [`buf.yaml`](./proto/buf.yaml).

### Setup development environment

TBW: Install `protoc` and `buf`

### Generate Rust and Python codes from `*.proto`

```shell
task proto
```

[`python/`](./python/)
----------------------
- OMMX Python SDK and adapter sub-projects
  - [`python/ommx/`](./python/ommx/): OMMX Python SDK
  - [`python/ommx-python-mip-adapter/`](./python/ommx-python-mip-adapter/): OMMX Python-MIP Adapter
  - [`python/ommx-pyscipopt-adapter/`](./python/ommx-pyscipopt-adapter/): OMMX PySCIPOpt Adapter
  - [`python/ommx-openjij-adapter/`](./python/ommx-openjij-adapter/): OMMX OpenJij Adapter
- Managed by `uv`, see its configuration in workspace [`pyproject.toml`](./pyproject.toml)

### Setup development environment

TBW: Install `uv`

### Setup development environment

```shell
task python:sync
```

### Run tests

```shell
task python:test
```

### Update version

```shell
task python:set-version -- 1.8.5
```

### Release Python SDK and adapters

TBW

[`rust/`](./rust/)
-------------------
- OMMX Rust SDK and sub tools written in Rust
  - [`rust/ommx/`](./rust/ommx/): OMMX Rust SDK
  - [`rust/protogen/`](./rust/protogen/): Rust code generator from `*.proto`
  - [`rust/dataset/`](./rust/dataset/): CLI tool for creating and uploading OMMX Artifact for dataset, e.g. MIPLIB2017
- Managed by `cargo`, see its configuration in the workspace [`Cargo.toml`](./Cargo.toml)

### Setup development environment

TBW: Install `rustup`

### Run tests

```shell
task rust:test
```

### Release Rust SDK

TBW

[`docs/`](./docs/)
-------------------
- API Reference of Python SDK, and Jupyter Book for tutorials
  - [`docs/en/`](./docs/en/): English version of Jupyter Book
  - [`docs/ja/`](./docs/ja/): Japanese version of Jupyter Book
  - [`docs/api_reference/`](./docs/api_reference/): API Reference of Python SDK and adapters

### Read the Docs deployments

There are three different Read the Docs deployments for this project:

* [**API Reference**](https://readthedocs.org/projects/ommx/): This deployment builds and deploys the API reference documentation for the OMMX Python SDK. The configuration for this deployment can be found in the `.readthedocs.yaml` file.
* [**Japanese Jupyter Book**](https://readthedocs.org/projects/ommx-ja-book/): This deployment builds and deploys the Japanese version of the Jupyter Book. The configuration for this deployment can be found in the `docs/ja/.readthedocs.yaml` file.
* [**English Jupyter Book**](https://readthedocs.org/projects/ommx-en-book/): This deployment builds and deploys the English version of the Jupyter Book. The configuration for this deployment can be found in the `docs/en/.readthedocs.yaml` file.

These deployments ensure that the documentation is always up-to-date and accessible to users in different languages.
