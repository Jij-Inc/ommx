# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

OMMX (Open Mathematical prograMming eXchange) is an open ecosystem for mathematical programming and optimization. It consists of:

- **Protocol Buffers Schema**: Message definitions in `proto/` for optimization problems, solutions, and metadata
- **Rust SDK**: Core library and tools in `rust/ommx/` with PyO3 bindings for Python
- **Python SDK**: Main package `ommx` and multiple solver adapters in `python/`
- **Documentation**: Jupyter Books in English/Japanese and API references

## Development Commands

This project uses [Taskfile](https://taskfile.dev/) for task management. Run `task -l` to see all available commands.

### Essential Commands

**Setup and Dependencies:**
```bash
# Python development environment
task python:sync

# Install/upgrade dependencies
task python:upgrade
```

**Testing:**
```bash
# Run all tests
task python:test        # Python tests (includes type checking)
task rust:test         # Rust tests only
task python:test-ci    # CI mode (no pyright for main ommx package)
```

**Code Quality:**
```bash
# Format Python code
task python:format

# Rust linting
task rust:clippy

# Type checking (Python)
task python:ommx:pyright
```

**Building and Documentation:**
```bash
# Generate code from protobuf
task proto

# Build documentation
task api_reference     # Python API docs
task rust:doc         # Rust docs
task book_en          # English Jupyter Book
task book_ja          # Japanese Jupyter Book
```

### Package-Specific Commands

**Core OMMX:**
- `task python:ommx:test` - Test main Python package
- `task python:ommx:pytest` - Run pytest only (no type checking)

**Adapters:**
- `task python:ommx-openjij-adapter:test`
- `task python:ommx-python-mip-adapter:test`
- `task python:ommx-pyscipopt-adapter:test`
- `task python:ommx-highs-adapter:test`

## Architecture

### Multi-Language Design
- **Rust**: Core implementation with performance-critical operations
- **Python**: High-level API and solver integrations via PyO3 bindings
- **Protocol Buffers**: Language-agnostic message schema for interoperability

### Workspace Structure
- **Rust Workspace**: `rust/ommx/`, `rust/protogen/`, `rust/dataset/`
- **Python Workspace**: Multiple packages under `python/` managed by uv
- **Shared Versioning**: Python packages versioned together, Rust independent

### Key Components
1. **Message Schema** (`proto/`): Core data structures for optimization problems, constraints, solutions
2. **Rust SDK** (`rust/ommx/`): Core algorithms, parsing, evaluation, artifact management
3. **Python SDK** (`python/ommx/`): PyO3 bindings to Rust + Python-specific utilities
4. **Solver Adapters** (`python/ommx-*-adapter/`): Integrations with OSS optimization solvers

### Protocol Buffer Generation
- Rust: Generated at build time via `build.rs`
- Python: Pre-generated files committed to repo, regenerated via `task proto:python`

## Testing Strategy

**Python Testing:**
- Main package: pytest + pyright type checking + doctests
- Adapters: pytest with solver-specific integration tests
- CI mode available for environments without pyright

**Rust Testing:**
- Standard cargo test with extensive property-based testing (proptest)
- Benchmarks using criterion/codspeed

## Development Workflow

1. **Protocol Changes**: Modify `.proto` files → `task proto` → update implementations
2. **Rust Changes**: Develop in `rust/ommx/` → Python bindings auto-generated via PyO3
3. **Python Changes**: Work in `python/ommx/` or adapter packages
4. **Documentation**: Jupyter notebooks in `docs/en/` and `docs/ja/` directories

## Version Management

**Python**: All packages versioned together via `task python:set-version -- x.y.z`
**Rust**: Independent versioning via `task rust:set-version -- x.y.z`

## Lint and Format Configuration

- **Python**: Ruff for formatting and linting (excludes .ipynb files)
- **Rust**: Standard clippy lints
- **Type Checking**: pyright for Python type checking

When making changes, always run the appropriate linting/testing commands before committing.