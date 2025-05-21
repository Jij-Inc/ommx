# Coding Standards

- Write all comments in English.
- Add code comments only when necessary to explain complex logic or the reasoning behind a decision (the "why", not the "what"). Avoid obvious comments.
- Ensure comments are about the code itself, not about the author or the process of writing the code.

# Project Context

## Core Technologies & Languages
- This project, ommx, primarily uses Rust and Python.
- When generating Rust code, adhere to idiomatic Rust practices and the standard library.
- When generating Python code, follow PEP 8 guidelines and use type hints where appropriate.

## Architectural Principles
- The main logic of this project is primarily implemented in Rust, with Python acting as a wrapper or interface to utilize this Rust core. Future development should adhere to this principle.
- A core goal of OMMX is to standardize data formats for mathematical optimization using Protocol Buffers.
- Serialization and deserialization code for Rust and Python is generated from `.proto` files.

## Repository Structure Overview
The repository is organized as follows:
- **Root Directory**: Contains top-level configuration files (`Cargo.toml`, `pyproject.toml`, `Taskfile.yml`, `README.md`), license files, and general project documentation.
- **`proto/`**: Protocol Buffer (`.proto`) definitions for OMMX data structures, along with `buf.build` configuration and generation setup.
- **`python/`**: Houses all Python-related code:
    - `ommx/`: The core Python SDK for OMMX, implemented as a mixed Rust/Python project using PyO3/Maturin.
    - `ommx-*-adapter/`: Various adapter packages (e.g., `ommx-pyscipopt-adapter`, `ommx-highs-adapter`) that integrate OMMX with other optimization solvers or libraries. These depend on the core `ommx` package.
    - Utility scripts for development (e.g., versioning, code generation).
- **`rust/`**: Contains all Rust crates:
    - `ommx/`: The core Rust implementation of the OMMX library.
    - `protogen/`: A utility crate for generating Rust code from Protocol Buffer definitions.
    - `dataset/`: Convert existing datasets like MIPLIB as OMMX Artifacts, and push to GitHub container registry.
- **`docs/`**: Project documentation, including:
    - `api_reference/`: Generated API documentation for OMMX Python SDK and adapters.
    - `en/` & `ja/`: User guides, tutorials, and release notes in English and Japanese by Jupyter Book.
- **`notebooks/`**: (deprecated) Jupyter notebooks for demonstrating library usage, experiments, and cookbook-style examples.
- **`ci-scripts/`**: Scripts related to Continuous Integration processes.
- **`data/`**: Example data files, such as `.ommx` instances.
- **`target/`**: Rust's default build output directory (typically excluded from version control).

## Development Workflow & Tooling
- This project uses `Taskfile.yml` to manage and execute complex commands. To understand available commands, refer to the `Taskfile.yml` files located in the root directory and various subdirectories (e.g., `rust/`, `python/`, `docs/`).

## General Guidance
- If relevant, consider the ommx library's conventions and APIs. Please ask if you need more specific details about ommx.
