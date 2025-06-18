# Project Context

## Core Technologies & Languages
- This project, ommx, primarily uses Rust and Python.
- Use idiomatic Rust practices and PEP 8 guidelines for Python.
- Write comments in English only when necessary to explain complex logic.

## Architectural Principles
- Core logic is implemented in Rust, with Python as a wrapper.
- Standardize mathematical optimization data formats using Protocol Buffers.
- Use `Taskfile.yml` for complex commands - check root and subdirectory Taskfiles.

## Repository Structure
- **`proto/`**: Protocol Buffer definitions
- **`python/ommx/`**: Mixed Rust/Python project using PyO3/Maturin
- **`python/ommx-*-adapter/`**: Adapter packages for optimization solvers
- **`rust/ommx/`**: Core Rust implementation
- Python SDK uses `uv` for dependency management

# Rust Development Guidelines

## Idiomatic vs. Protobuf Structures
- Use idiomatic Rust structs (e.g., `ommx::Instance`) for core logic and APIs
- Use protobuf structs (e.g., `ommx::v1::Instance`) only for serialization/deserialization
- Convert between protobuf and idiomatic structs at serialization boundaries

## Common Practices
- Use `Coefficient::try_from(value).unwrap()` for creating coefficients, or `coeff!` macro in tests
- Use convenience macros in test code: `linear!`, `quadratic!`, `monomial!`, `assign!`
- For property-based tests, use existing `arbitrary_xxx` generators when available
