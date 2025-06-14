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

## Current Implementation Status (Dec 2024)

### Python SDK v2 Migration Completed ✅

The project has completed its migration from Protocol Buffers auto-generated Python classes to high-performance Rust implementations with PyO3 bindings:

**Core Features Completed**:
- ✅ All mathematical objects (`Linear`, `Quadratic`, `Polynomial`, `Function`) use Rust implementations
- ✅ Instance class fully migrated to Rust backend with maintained API compatibility
- ✅ All solver adapters (Python-MIP, PySCIPOpt, HiGHS) migrated to v2 API
- ✅ Type-safe PyO3 enums (`Sense`, `Equality`, `Kind`) with Protocol Buffer conversion
- ✅ Comprehensive testing and documentation updated

**Key Benefits Achieved**:
- **Performance**: Native Rust operations for mathematical computations
- **Type Safety**: PyO3 enums with proper type checking
- **API Consistency**: Unified `ommx.v1` import pattern across all components
- **Maintainability**: Elimination of direct `_ommx_rust` usage in favor of Python SDK extensions

**Migration Pattern**:
```python
# Established pattern: Rust implementation wrapped in Python
class Linear(AsConstraint):
    raw: _ommx_rust.Linear  # Rust implementation
    
    def __init__(self, *, terms: dict[int, float], constant: float = 0):
        self.raw = _ommx_rust.Linear(terms=terms, constant=constant)
    
    def evaluate(self, state: State) -> float:
        return self.raw.evaluate(to_state(state).SerializeToString())
```

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
task python:test        # Python tests (includes linting, type checking, and pytest)
task rust:test         # Rust tests only
task python:test-ci    # CI mode (no pyright for main ommx package)
```

**Code Quality:**
```bash
# Format Python code
task python:format

# Python linting
task python:lint         # Run ruff check on all Python packages

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

## Testing Strategy

**Python Testing:**
- Main package: ruff linting + pytest + pyright type checking + doctests
- Adapters: ruff linting + pytest with solver-specific integration tests
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

## Important Notes for Development

1. **API Philosophy**: Avoid `_ommx_rust` direct imports; always use `ommx.v1` unified API. When needed functionality is missing, extend the Python SDK rather than using raw APIs
2. **Protocol Buffers Compatibility**: Ensure proper use of `ParseFromString()` method when converting from Protocol Buffers messages to Rust implementations
3. **Test Coverage**: The test suite includes comprehensive tests covering core functionality, QUBO conversion, MPS format handling, decision variable analysis, constraint wrappers, and doctests
4. **Performance**: Core mathematical operations are implemented in Rust for optimal performance while maintaining Python usability
5. **Error Handling**: Rust implementations provide detailed error messages for debugging mathematical programming issues

## Development Guidance

### General Development Principles
- When modifying Rust code, always make small incremental changes and ensure `cargo check` passes at each step
- Always run `task python:test` after making changes to ensure all tests pass
- Use incremental approach: implement one component at a time, test, then commit
- Maintain backward compatibility during migration phases
- **Important**: Prefer extending Python SDK over using raw APIs - this ensures API stability and better user experience

### Adapter Development Guidelines

When developing or modifying solver adapters:

1. **Import Standards**: Use only `ommx.v1` imports, avoid Protocol Buffer and `_ommx_rust` direct imports
2. **API Usage**: Use Python SDK methods instead of raw API calls
3. **Type Conversions**: Let Python SDK handle conversions between Rust and Python types
4. **Extension Pattern**: If needed functionality is missing, add it to Python SDK classes
5. **Testing**: Ensure all tests pass including doctests and pyright checks

**Example Pattern**:
```python
# Good: Using Python SDK
from ommx.v1 import Instance, DecisionVariable, Function, Solution

# Bad: Using raw or Protocol Buffer APIs
from ommx._ommx_rust import Function
from ommx.v1.solution_pb2 import Optimality
```

### Current Development Status (December 2024)
- **Core Migration ✅**: Protocol Buffer to Rust migration completed across all components
- **Adapter Support ✅**: All major adapters (Python-MIP, PySCIPOpt, HiGHS) migrated to v2 API
- **Documentation ✅**: Comprehensive migration guide and adapter specifications available
- **API Stability ✅**: Unified `ommx.v1` API established with proper extension patterns
- **Performance ✅**: Rust backend providing optimal performance for mathematical operations

## Development Notes

### 🚫 Critical Prohibitions
- **NEVER use `cd` command** - Work from repository root directory only
- **NEVER create inline tests** - Add tests to python/ommx-tests/tests only
- **NEVER run `python -c` directly** - Use proper test framework
- **NEVER modify v1_ext directory** - Contains deprecated Protocol Buffer implementations
- **NEVER import from `_ommx_rust` in adapters** - Use `ommx.v1` unified API instead

### 📁 Directory Guidelines  
- Most tasks should be performed from the repository root directory
- Return to root after completing any subtasks
- Use absolute paths when referencing files across packages

### 🧪 Testing Guidelines
- Add new Python test code to python/ommx-tests/tests directory only
- Run tests using pytest through task commands
- Verify test compatibility across adapter packages before committing

### 🔧 Code Quality
- Always run `task format` before committing changes
- Ensure `task python:test` passes completely
- Follow incremental development: small changes → test → commit