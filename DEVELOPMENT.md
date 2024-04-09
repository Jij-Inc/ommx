README for Developers of OMMX
==============================

## Code map

- `proto/`
  - `*.proto` files defining OMMX data schema
  - Managed by [`buf`](https://buf.build/docs/introduction), see its configuration in [`buf.yaml`](./proto/buf.yaml)
- `python/`
  - Python package for OMMX
  - Managed by `pip`, see its configuration in [`pyproject.toml`](./python//pyproject.toml)
- `rust/`
  - Rust crate for OMMX
  - Managed by `cargo`, see its configuration in [`Cargo.toml`](./rust/Cargo.toml)

## Python package
[![PyPI - Version](https://img.shields.io/pypi/v/ommx)](https://pypi.org/project/ommx/)

### First of all
```shell
python -m venv .venv
source .venv/bin/activate
pip install "python/[dev]"
```

### How to generate python codes
```shell
cd proto
buf generate --template buf.gen.python.yaml
```

### How to generate documents for python package
```shell
sphinx-build -b html ./python/docs/source ./python/docs/build
```

### How to release to PyPI

1. Update version in `python/pyproject.toml`
  - This process will be automated
2. Push Git tag with `python-x.y.z`, then the GitHub Actions will release to PyPI

## Rust crate
[![Crates.io Version](https://img.shields.io/crates/v/ommx)](https://crates.io/crates/ommx)

### How to generate Rust codes

```shell
cd rust
cargo run --bin protogen
```
