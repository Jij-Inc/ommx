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

## Development Workflow & Tooling
- This project uses `Taskfile.yml` to manage and execute complex commands. To understand available commands, refer to the `Taskfile.yml` files located in the root directory and various subdirectories (e.g., `rust/`, `python/`, `docs/`).

## General Guidance
- If relevant, consider the ommx library's conventions and APIs. Please ask if you need more specific details about ommx.
