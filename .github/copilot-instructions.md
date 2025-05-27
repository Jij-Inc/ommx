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
- **`proto/`**: Protocol Buffer definitions for OMMX data structures
- **`python/`**: Python-related code
    - `ommx/`: Core Python SDK (mixed Rust/Python project using PyO3/Maturin)
    - `ommx-*-adapter/`: Adapter packages for various optimization solvers
- **`rust/`**: Rust crates
    - `ommx/`: Core Rust implementation of the OMMX library
    - `protogen/`: Utility for generating Rust code from Protocol Buffer definitions
    - `dataset/`: Convert existing datasets like MIPLIB as OMMX Artifacts
- **`docs/`**: Project documentation (API reference, user guides)

## Development Workflow & Tooling
- This project uses `Taskfile.yml` to manage and execute complex commands. To understand available commands, refer to the `Taskfile.yml` files located in the root directory and various subdirectories (e.g., `rust/`, `python/`, `docs/`).

# Python SDK Development Guidelines

## Dependency Management
- The Python SDK uses `uv` for dependency management.
- The core `ommx` package, located in `python/ommx/`, is a mixed Rust/Python project built with PyO3 and maturin.

## Package Structure
- The repository contains multiple Python packages. The core `ommx` package serves as a dependency for various adapter projects (`ommx-*-adapter`).
- The `python/ommx/` directory is structured as a Maturin mixed Rust/Python project.
- When a user imports the package in Python (e.g., `import ommx`), the primary Python module loaded is `python/ommx/ommx/__init__.py`.
- The Rust-native components are made available as a submodule, typically `ommx._ommx_rust`.

# Rust SDK Development Guidelines

## Core Design Principle: Idiomatic Rust Structures vs. Protobuf-Generated Structures

A fundamental architectural decision for the Rust SDK (version 2.0.0 and onwards) is the separation of concerns between idiomatic Rust structures and structures auto-generated from Protocol Buffer (`.proto`) definitions.

**1. Idiomatic Rust Structures (e.g., `ommx::Instance`, `ommx::Function`):**
- These are the primary structures to be used for all **core logic, internal operations, and public-facing APIs** of the SDK.
- They should be designed following Rust best practices and idiomatic patterns.
- **Goal:** Provide a developer-friendly, type-safe, and efficient interface for working with OMMX data within Rust.

**2. Protobuf-Generated Structures (e.g., `ommx::v1::Instance`, `ommx::v1::Function`):**
- These structures are automatically generated from the `.proto` files (e.g., those in `proto/ommx/v1/`).
- Their **sole purpose** is for **serialization and deserialization** of data to and from the OMMX binary format.
- They should **not** be used directly for implementing core SDK logic or in public APIs beyond the raw serialization/deserialization layer.

## Data Flow Expectation

**Reading OMMX data:**
1. Deserialize binary data into Protobuf-generated structs (e.g., `ommx::v1::Instance`).
2. Convert/map these Protobuf-generated structs into their corresponding idiomatic Rust structs (e.g., `ommx::Instance`).
3. Use the idiomatic Rust structs for all subsequent operations.

**Writing OMMX data:**
1. Convert/map idiomatic Rust structs (e.g., `ommx::Instance`) into their corresponding Protobuf-generated structs (e.g., `ommx::v1::Instance`).
2. Serialize the Protobuf-generated structs into binary data.

## Transition and Current State

- This architectural shift is an **ongoing process**.
- You may find parts of the existing codebase where Protobuf-generated structs (`ommx::v1::*`) are still used directly in logic that should ideally use idiomatic Rust structs.
- **When developing new features or refactoring existing code:**
  - Prioritize the use of idiomatic Rust structs for all logic.
  - Implement the necessary conversions to/from the `ommx::v1::*` Protobuf structs at the serialization boundaries.
  - If you encounter legacy code directly using `ommx::v1::*` for logic, and it's within the scope of your task, consider refactoring it to align with this new principle.

## Summary of Key Types

- `ommx::Instance`: Preferred idiomatic Rust struct for representing an optimization instance.
- `ommx::v1::Instance`: Protobuf-generated struct, use only for serializing/deserializing `ommx::Instance`.
- This pattern applies to other core OMMX entities like `Function`, `Constraint`, `Variable`, etc. (e.g., `ommx::Function` vs. `ommx::v1::Function`).

## Common Pitfalls / Frequently Made Mistakes

- **Incorrect `Coefficient` Creation:** When creating a `Coefficient` from an `f64` value, always use `Coefficient::try_from(value).unwrap()` instead of `Coefficient::from(value)`.

## General Guidance
- If relevant, consider the ommx library's conventions and APIs. Please ask if you need more specific details about ommx.
