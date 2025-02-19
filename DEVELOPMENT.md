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

::: tip
When you run `task` command, it will automatically detect the `Taskfile.yml` from current directory or parent directories.
If you run `task` command on `proto/` directory, `proto/Taskfile.yml` is used, i.e. commands only defined there are available.
Each sub-directory `Taskfile.yml` are included in the parent `Taskfile.yml`,
e.g. `task python` in `proto/` is exposed as `task proto:python` in the root directory.
:::

Code map
--------

- [`proto/`](./proto/)
  - OMMX Message schema is defined in `*.proto` files under this directory.
  - Managed by [`buf`](https://buf.build/docs/introduction), see its configuration in [`buf.yaml`](./proto/buf.yaml)
  - Defined tasks:

    ```shell
    task proto         # Run proto:python and proto:rust
    task proto:python  # Generate Python code from *.proto
    task proto:rust    # Generate Rust code from *.proto
    ```

- [`python/`](./python/)
  - OMMX Python SDK and adapter sub-projects
  - Managed by `uv`, see its configuration in [`pyproject.toml`](./python/ommx/pyproject.toml)
- [`rust/ommx/`](./rust/ommx/)
  - OMMX Rust SDK
  - Managed by `cargo`, see its configuration in the workspace [`Cargo.toml`](./Cargo.toml)
- [`rust/protogen/`](./rust/protogen/)
  - Rust code generator from `*.proto`. Used by `cargo run --bin protogen`. This is used only for development, and not published to crates.io.

OMMX Python SDK
----------------
[![PyPI - Version](https://img.shields.io/pypi/v/ommx)](https://pypi.org/project/ommx/)

### Setup development environment

OMMX Python SDK is Rust/Python mixed project. You need to install Rust and Python development tools.

#### Install Rust

See the [official guide](https://www.rust-lang.org/tools/install) for details.

#### virtualenv for Python

```shell
python -m venv .venv
source .venv/bin/activate
task install
```

TODO: Another Python development tools like `poetry` or `rye`

### Generate Python codes from `*.proto`

```shell
task protogen_python
```

### Generate API reference

```shell
task api_reference:build
```

### Release to PyPI

[!IMPORTANT] The version of `ommx` and the adapters should be in sync.
1. Update version in `python/ommx/pyproject.toml`
2. Update version in `python/ommx-python-mip-adapter/project.toml` 
3. Push Git tag with `python-x.y.z`, then the GitHub Actions will release to PyPI

OMMX Rust SDK
--------------
[![Crates.io Version](https://img.shields.io/crates/v/ommx)](https://crates.io/crates/ommx)

### Generate Rust codes from `*.proto`

```shell
task protogen_rust
```

### Release to crates.io

1. Push a new Git tag named `rust-x.y.z`, then the GitHub Actions will release to crates.io
2. Create a GitHub release.

Read the Docs Settings
----------------------

### Configuration Files

The Read the Docs configuration files are:

- `.readthedocs.yaml`
- `docs/en/.readthedocs.yaml`
- `docs/ja/.readthedocs.yaml`

### Purpose

These configuration files are used to build and deploy documentation to Read the Docs. They specify the settings and dependencies required for the documentation build process.

### Updating Settings

To update the Read the Docs settings, follow these steps:

1. Open the relevant `.readthedocs.yaml` file.
2. Modify the settings as needed.
3. Commit and push the changes to the repository.
4. The documentation will be automatically rebuilt and deployed with the new settings.

Read the Docs deployments
--------------------------

There are three different Read the Docs deployments for this project:

* [**API Reference**](https://readthedocs.org/projects/ommx/): This deployment builds and deploys the API reference documentation for the OMMX Python SDK. The configuration for this deployment can be found in the `.readthedocs.yaml` file.
* [**Japanese Jupyter Book**](https://readthedocs.org/projects/ommx-ja-book/): This deployment builds and deploys the Japanese version of the Jupyter Book. The configuration for this deployment can be found in the `docs/ja/.readthedocs.yaml` file.
* [**English Jupyter Book**](https://readthedocs.org/projects/ommx-en-book/): This deployment builds and deploys the English version of the Jupyter Book. The configuration for this deployment can be found in the `docs/en/.readthedocs.yaml` file.

These deployments ensure that the documentation is always up-to-date and accessible to users in different languages.
