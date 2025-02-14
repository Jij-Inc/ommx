README for Developers of OMMX
==============================

Code map
--------

- [`proto/`](./proto/)
  - `*.proto` files defining OMMX Messages
  - Managed by [`buf`](https://buf.build/docs/introduction), see its configuration in [`buf.yaml`](./proto/buf.yaml)
- [`python/ommx/`](./python/ommx/)
  - OMMX Python SDK
  - Managed by `pip`, see its configuration in [`pyproject.toml`](./python/ommx/pyproject.toml)
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
pip install "python/ommx[dev]"
```

TODO: Another Python development tools like `poetry` or `rye`

### Generate Python codes from `*.proto`

```shell
cd proto
buf generate --template buf.gen.python.yaml
ruff format ../python
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
cargo run --bin protogen
```

### Release to crates.io

1. Push a new Git tag named `rust-x.y.z`, then the GitHub Actions will release to crates.io
2. Create a GitHub release.

Taskfile.yml
------------

### Purpose and Usage

`Taskfile.yml` is used to define and manage various development tasks in a standardized way. It helps automate repetitive tasks, making the development process more efficient.

### Common Tasks

Here are some common tasks that can be executed using `Taskfile.yml`:

- **Building Documentation**: 
  ```shell
  task build_book_ja
  task build_book_en
  ```

- **Running Tests**: 
  ```shell
  task test_python
  ```

- **Generating Code from `.proto` Files**: 
  ```shell
  task protogen
  ```

### Dependencies

To use `Taskfile.yml`, you need to have the following dependencies installed:

- `task`: A task runner for defining and running tasks.
- `fswatch`: A file change monitor that triggers tasks when files are modified.

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
