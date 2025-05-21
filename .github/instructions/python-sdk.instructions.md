---
applyTo: "python/**/*"
---

# Python SDK (ommx package) Development Guidelines

This document provides specific instructions for developing the Python SDK.
- The Python SDK uses `uv` for dependency management.
- The core `ommx` package, located in `python/ommx/`, is a mixed Rust/Python project built with PyO3 and maturin.

### Maturin Project Structure (for `python/ommx/`)
- The `python/ommx/` directory, which houses the core `ommx` Python package, is structured as a Maturin mixed Rust/Python project.
- Key configuration files, `pyproject.toml` (for Python packaging and Maturin settings) and `Cargo.toml` (for Rust crate definition), are located at the root of this directory (`python/ommx/`).
- When a user imports the package in Python (e.g., `import ommx`), the primary Python module loaded is `python/ommx/ommx/__init__.py` (and other Python files within `python/ommx/ommx/`).
- The Rust-native components, compiled by Maturin, are made available as a submodule, typically `ommx._ommx_rust`. This submodule contains the functions and structs exposed from the Rust side via PyO3.

## Python Package Structure
- The repository contains multiple Python packages. The core `ommx` package (located in `python/ommx/`) serves as a dependency for various adapter projects, such as `ommx-pyscipopt-adapter` and others following the `*-adapter` naming convention. Changes to the `python/ommx/` package may necessitate corresponding changes in these dependent adapter projects.
