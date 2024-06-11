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
sphinx-build -b html ./python/ommx/docs/source ./python/ommx/docs/build
```

### Release to PyPI

1. Update version in `python/ommx/pyproject.toml`
2. Push Git tag with `python-x.y.z`, then the GitHub Actions will release to PyPI

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
