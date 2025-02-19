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

- Install [buf](https://buf.build/docs/cli/installation/)
- Install [protoc](https://grpc.io/docs/protoc-installation/)

### Generate Rust and Python codes from `*.proto`

```shell
task proto
```

[`rust/`](./rust/)
-------------------
- OMMX Rust SDK and sub tools written in Rust
  - [`rust/ommx/`](./rust/ommx/): OMMX Rust SDK
  - [`rust/protogen/`](./rust/protogen/): Rust code generator from `*.proto`
  - [`rust/dataset/`](./rust/dataset/): CLI tool for creating and uploading OMMX Artifact for dataset, e.g. MIPLIB2017
- Managed by `cargo`, see its configuration in the workspace [`Cargo.toml`](./Cargo.toml)

### Setup development environment

Install [rustup](https://www.rust-lang.org/tools/install):

```shell
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

`cargo` will setup everything when it is called.

### Run tests

```shell
task rust:test
```

Note that this only tests the Rust SDK, not the Rust codes in Python SDK.

### Versioning

The version of Rust SDK is [semantic versioning](https://semver.org/).
Note that the version of Rust SDK is independent from the schema version `ommx.v1` and that of Python SDK.

```shell
task rust:set-version -- 2.0.0
```

Note that this requires [cargo-edit](https://github.com/killercup/cargo-edit).

### Release Rust SDK

The Rust SDK is released to [crates.io](https://crates.io/) from the [GitHub Actions workflow](https://github.com/Jij-Inc/ommx/actions/workflows/release_rust.yml). What you have to do is just to push a tag in a format `rust-x.y.z`. Be sure that actual version is determined by `Cargo.toml` not by the tag name.

There are two mechanism to keep the version of `main` branch is kept latest:

- When Rust SDK is released, the patch version is automatically bumped up via a Pull Request.
- When a pull request contains breaking change, `cargo-semver-check` on GitHub Action will fail. So, this pull request should be merged with bumping up the version.

[`python/`](./python/)
----------------------

| Name | PyPI |
|:---- |:---- |
| [OMMX Python SDK](./python/ommx) | [![ommx](https://img.shields.io/pypi/v/ommx)](https://pypi.org/project/ommx/) |
| [OMMX Python-MIP Adapter](./python/ommx-python-mip-adapter) | [![ommx-python-mip-adapter](https://img.shields.io/pypi/v/ommx-python-mip-adapter)](https://pypi.org/project/ommx-python-mip-adapter/) |
| [OMMX PySCIPOpt Adapter](./python/ommx-pyscipopt-adapter) | [![ommx-pyscipopt-adapter](https://img.shields.io/pypi/v/ommx-pyscipopt-adapter)](https://pypi.org/project/ommx-pyscipopt-adapter/) |
| [OMMX OpenJij Adapter](./python/ommx-openjij-adapter) | [![ommx-openjij-adapter](https://img.shields.io/pypi/v/ommx-openjij-adapter)](https://pypi.org/project/ommx-openjij-adapter/) |

Python packages are managed by [uv workspace](https://docs.astral.sh/uv/concepts/projects/workspaces/)

### Setup development environment

First, install [uv](https://docs.astral.sh/uv/getting-started/installation/) and Rust toolchain as above, then run:

```shell
task python:sync
```

This command lets `uv` create a venv and install the required packages including the OMMX Python SDK and adapters.

### Run tests

```shell
task python:test
```

### Versioning

The version of Python SDK is [semantic versioning](https://semver.org/).
Note that the version of Python SDK is independent from the schema version `ommx.v1` and that of Rust SDK.

To update the version of Python SDK to `x.y.z`, run:

```shell
task python:set-version -- x.y.z
```

> [!NOTE]
> The version number is shared among all Python projects, `ommx` and other adapters.
> Above command sets the version number to `1.8.5` for all Python projects simultaneously.

### Release Python SDK and adapters

The Python SDK and adapters are released to [PyPI](https://pypi.org/) from the [GitHub Actions workflow](https://github.com/Jij-Inc/ommx/actions/workflows/release_python.yml). What you have to do is just to push a tag in a format `python-x.y.z`. Be sure that actual version is determined by `pyproject.toml` not by the tag name.

- The patch version is automatically bumped up via a Pull Request when Python SDK is released.
- Different from Rust SDK, there is no automatic semver check for Python SDK. So, you have to manually update the version number.

[`docs/`](./docs/)
-------------------

| Directory | Description | GitHub Pages  | Read the Docs |
|:----------|:------------|:--------------|:--------------|
| [`docs/en/`](./docs/en/) | English version of Jupyter Book | [![Book/en](https://img.shields.io/badge/Book-English-blue)](https://jij-inc.github.io/ommx/en) | [![RTD](https://readthedocs.org/projects/ommx-en-book/badge/?version=latest)](https://ommx-en-book.readthedocs.io/en/latest/?badge=latest) |
| [`docs/ja/`](./docs/ja/) | Japanese version of Jupyter Book | [![Book/ja](https://img.shields.io/badge/Book-日本語-blue)](https://jij-inc.github.io/ommx/ja) | [![RTD](https://readthedocs.org/projects/ommx-ja-book/badge/?version=latest)](https://ommx-ja-book.readthedocs.io/ja/latest/?badge=latest) |
| [`docs/api_reference/`](./docs/api_reference/) | API Reference of Python SDK and adapters | [![API Reference](https://img.shields.io/badge/API%20Reference-main-blue)](https://jij-inc.github.io/ommx/python/ommx/autoapi/index.html) | [![RTD](https://readthedocs.org/projects/ommx/badge/?version=latest)](https://ommx.readthedocs.io/en/latest/?badge=latest) |

### Build and open the book

```shell
task book_en
task book_ja
```

### Build and open the API Reference

```shell
task api_reference
```
