# Coding Standards
- Write all comments in English.
- Add code comments only when necessary to explain complex logic or the reasoning behind a decision (the "why", not the "what"). Avoid obvious comments.
- Ensure comments are about the code itself, not about the author or the process of writing the code.

# Project Context

OMMX is a library for standardizing mathematical optimization data formats using Protocol Buffers. The core logic is implemented in Rust with Python wrappers, creating a mixed Rust/Python project using PyO3/Maturin.

## Core Technologies & Languages
- The project primarily uses Rust and Python.
- Serialization and deserialization use Protocol Buffers (`.proto` files).
- Python integration is handled via PyO3/Maturin.
- Python dependencies are managed with `uv`, Rust dependencies with `cargo`.
- Complex build/test tasks are managed via `Taskfile.yml`.

## Architectural Principles
- The main logic is primarily implemented in Rust, with Python as a wrapper or interface.
- Protobuf-generated structs (e.g., `ommx::v1::Instance`) are for serialization/deserialization only.
- Idiomatic Rust structs (e.g., `ommx::Instance`) are used for core logic, internal operations, and public-facing APIs.
- Data flow:
  1. When reading OMMX data: Deserialize to Protobuf structs → Convert to idiomatic Rust structs → Use internally.
  2. When writing: Convert idiomatic Rust structs to Protobuf structs → Serialize.

## Repository Structure Overview
- **`proto/`**: Protocol Buffer definitions
- **`python/`**: Python-related code  
  - `ommx/`: Core Python SDK (mixed Rust/Python)  
  - `ommx-*-adapter/`: Adapter packages for various solvers
- **`rust/`**: Rust crates  
  - `ommx/`: Core Rust implementation  
  - `protogen/`: Utility for generating Rust code from `.proto`  
  - `dataset/`: Tools for converting existing datasets (e.g., MIPLIB)
- **`docs/`**: Project documentation

## Testing & Build Commands
- **Python**  
  - `task python`: Sync dependencies and run all Python tests  
  - `task python:test`: Run tests for all Python packages  
  - `task python:ommx:test`: Test main SDK (pytest + type checking)  
  - `task python:ommx:pyright`: Type checking only  
- **Rust**  
  - `task rust:test`: Run Rust tests  
  - `task rust:check`: `cargo check`  
  - `task rust:clippy`: Lint with clippy  
  - `task rust:doc`: Generate Rust documentation

## Development Workflow & Tooling
- `Taskfile.yml` is used for running common commands (build, test, lint).
- For Python projects, `ommx` is built with PyO3/Maturin and published as a mixed Rust/Python package.
- Protobuf files are in `proto/`, with generated code in `rust/protogen` or `python/ommx/_proto`.

# Python SDK Development Guidelines

## Dependency Management
- Python dependencies are managed by `uv`.
- The `python/ommx/` directory is a mixed Rust/Python project.

## Package Structure
- `ommx` acts as the core Python SDK.
- `ommx/_ommx_rust` exposes Rust-native components.
- Other packages (e.g., `ommx-*-adapter`) depend on this core SDK.

# Rust SDK Development Guidelines

## Core Design Principle
- Use idiomatic Rust structs (e.g., `ommx::Instance`, `ommx::Function`) for all logic.
- Use Protobuf-generated structs (e.g., `ommx::v1::Instance`) only at serialization/deserialization boundaries.

## Property-Based Testing Guidelines
- Use existing `arbitrary_xxx` functions for generating test data.
- Verify evaluation equivalence with tolerance-based comparisons using the `approx` crate.

## Common Pitfalls
- Create `Coefficient` via `Coefficient::try_from(value).unwrap()` or use the `coeff!` macro.
- Avoid exact floating-point comparisons; use tolerance-based checks.

## Macros for Test Code
- `coeff!(value)` to create a Coefficient.
- `linear!(id)`, `quadratic!(...)`, `monomial!(...)`, `assign! { ... }` for concise test cases.
