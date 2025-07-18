# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

OMMX (Open Mathematical prograMming eXchange) is an open ecosystem for mathematical programming and optimization. It consists of:

- **Protocol Buffers Schema**: Message definitions in `proto/` for optimization problems, solutions, and metadata
- **Rust SDK**: Core library and tools in `rust/ommx/` with PyO3 bindings for Python
- **Python SDK**: Main package `ommx` and multiple solver adapters in `python/`
- **Documentation**: Jupyter Books in English/Japanese and API references

## Architecture

### Multi-Language Design
- **Rust**: Core implementation with performance-critical operations
- **Python**: High-level API and solver integrations via PyO3 bindings
- **Protocol Buffers**: Language-agnostic message schema for interoperability

### Key Components
1. **Message Schema** (`proto/`): Core data structures for optimization problems, constraints, solutions
2. **Rust SDK** (`rust/ommx/`): Core algorithms, parsing, evaluation, artifact management
3. **Python SDK** (`python/ommx/`): PyO3 bindings to Rust + Python-specific utilities
4. **Solver Adapters** (`python/ommx-*-adapter/`): Integrations with OSS optimization solvers

## Current Implementation Status

### Rust SDK v2 Migration Completed ✅

The project has completed its migration from Protocol Buffers auto-generated Python classes to high-performance Rust implementations with PyO3 bindings.

**Design Philosophy**:
- Replace `ommx::v1::*` (protobuf auto-generated) with `ommx::*` (Rust native types)
- Improve type safety and reduce runtime errors
- Enable efficient data structures for deduplication

**IMPORTANT: Feasibility Semantics**:
- **`feasible`**: Considers ALL constraints (including removed ones with `removed_reason.is_some()`)
- **`feasible_relaxed`**: Only considers active constraints where `removed_reason.is_none()`
- **`feasible_unrelaxed`**: Always identical to `feasible` (deprecated field maintained for backward compatibility)
- This distinction is critical for constraint relaxation scenarios where removed constraints should not affect relaxed feasibility

**Implementation Details**: See actual code in `rust/ommx/src/` for current type definitions and API.

## Development Commands

This project uses [Taskfile](https://taskfile.dev/) for task management. **⚠️ All commands must be run from the project root directory.**

**Essential Commands:**
```bash
# Setup
task python:sync               # Setup Python development environment
task python:upgrade           # Install/upgrade dependencies

# Testing
task python:test              # Run all Python tests (linting, type checking, pytest)
task rust:test               # Run Rust tests only
task format                  # Format all code (Python and Rust)

# Development
task proto                   # Generate all protobuf code
task rust:check             # Run cargo check
task rust:clippy            # Run Rust linting
task python:lint            # Run Python linting

# Documentation
task api_reference          # Build and open Python API docs
task book_en                # Build and open English Jupyter Book
```

**Package-Specific Testing:**
- `task python:ommx:test` - Test main Python package
- `task python:ommx-*-adapter:test` - Test specific adapter

Run `task -l` to see all available commands.

## Development Guidelines

### Core Development Principles
1. **API Philosophy**: Always use `ommx.v1` unified API, avoid direct `_ommx_rust` imports
2. **Incremental Development**: Make small changes → test → commit
3. **Always run `task python:test` after making changes**
4. **Prefer extending Python SDK over using raw APIs**

### Adapter Development
When developing solver adapters:
- Use only `ommx.v1` imports, avoid Protocol Buffer direct imports
- Let Python SDK handle type conversions
- Add missing functionality to Python SDK classes, not raw API usage

**Good Pattern:**
```python
from ommx.v1 import Instance, DecisionVariable, Function, Solution
```

**Bad Pattern:**
```python
from ommx._ommx_rust import Function
from ommx.v1.solution_pb2 import Optimality
```

## Critical Development Rules

### 🚫 Prohibitions
- **NEVER use `cd` command** - Work from repository root only
- **NEVER import from `_ommx_rust` in adapters** - Use `ommx.v1` unified API
- **NEVER modify v1_ext directory** - Contains deprecated implementations

### 🧪 Testing & Quality
- Add tests to `python/ommx-tests/tests` only
- Always run `task format` before committing
- Ensure `task python:test` passes completely

### 📊 Performance Claims Policy
- **NEVER claim performance improvements without benchmarks**
- Only make performance claims backed by concrete measurements
- Use terms like "improved type safety" or "better API design" for non-performance benefits
- If making performance claims, include benchmark data and methodology

### 📝 Rust SDK Testing Guidelines
- Use `assert_abs_diff_eq!` to compare entire polynomials instead of checking individual terms with `get`
- Include clear comments in test cases explaining the intent and expected behavior
- **Test Design Requirements**:
  - Document what each test is checking with clear test names and comments
  - Avoid redundant tests - check for overlapping test coverage
  - Consider using helper functions to reduce duplication
  - Group related assertions together
- **Test Redundancy Prevention**:
  - Before adding a new test, review existing tests to ensure it provides unique value
  - If multiple tests share similar setup code, extract it into helper functions
  - Consolidate tests that verify the same behavior with different inputs into parameterized tests where appropriate
  - Each test should have a single clear purpose