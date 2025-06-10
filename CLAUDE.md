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

### Protocol Buffers to Rust/PyO3 Migration
The project is actively migrating from Protocol Buffers auto-generated Python classes to high-performance Rust implementations with PyO3 bindings:

**Core Mathematical Objects** (Completed):
- `Linear`, `Quadratic`, `Polynomial`, `Function` classes now use Rust implementations
- `DecisionVariableAnalysis` and `Bound` classes provide comprehensive variable analysis
- `DecisionVariable` PyO3 wrapper implemented in `python/ommx/src/instance.rs`
- Located in `python/ommx/src/message.rs` and `python/ommx/src/instance.rs`, exposed via `_ommx_rust` module
- Python wrappers in `python/ommx/ommx/v1/__init__.py` use `.raw` attribute pattern
- All evaluation methods migrated to instance methods (`.evaluate()`, `.partial_evaluate()`)

**Migration Pattern**:
```python
# New pattern: Rust implementation wrapped in Python
class Linear(AsConstraint):
    raw: _ommx_rust.Linear  # Rust implementation
    
    def __init__(self, *, terms: dict[int, float], constant: float = 0):
        self.raw = _ommx_rust.Linear(terms=terms, constant=constant)
    
    def evaluate(self, state: State) -> float:
        return self.raw.evaluate(to_state(state).SerializeToString())
```

**Instance Migration Roadmap (Current Focus)**:
The Instance class migration follows this phased approach:

1. **Phase 1: ✅ Completed** - DecisionVariable PyO3 wrapper
   - Implemented `_ommx_rust.DecisionVariable` with proper Rust type usage
   - Factory methods for binary, integer, continuous variables
   - All tests passing and committed

2. **Phase 2: ✅ Completed** - Additional PyO3 wrappers
   - ✅ `Constraint` PyO3 wrapper implementation with factory methods and getters
   - ✅ `RemovedConstraint` PyO3 wrapper implementation with complete functionality
   - All wrappers follow native Rust type pattern for optimal performance
   - Comprehensive test suites (15 test cases total) covering all functionality

3. **Phase 3: 🔄 Next** - Rust Instance API extension
   - Add getter methods (`get_objective`, `get_sense`, `get_decision_variables`, `get_constraints`)
   - Add `from_components` constructor method to Rust Instance

4. **Phase 4: 📋 Planned** - Python Instance migration
   - Replace `Instance.raw` from Protocol Buffer to `_ommx_rust.Instance`
   - Update all Instance methods to use Rust implementation
   - Maintain backward compatibility with existing Python API

5. **Phase 5: 📋 Planned** - Testing and validation
   - Comprehensive testing of migrated Instance functionality
   - Performance validation and optimization

**Key Implementation Details**:
- Python classes are thin wrappers around Rust core types
- Protocol Buffers serialization/deserialization handled by Rust
- Mathematical operations (add, subtract, multiply) implemented in Rust
- Object-oriented evaluation API with instance methods for better encapsulation
- Use native Rust types (`ommx::DecisionVariable`, `ommx::Constraint`, `ommx::RemovedConstraint`) rather than Protocol Buffer types for better performance

**Phase 2 Completed Features**:
- **DecisionVariable wrapper**: Factory methods (binary, integer, continuous), property getters, proper type validation
- **Constraint wrapper**: Direct constructor, factory methods (equal_to_zero, less_than_or_equal_to_zero), function access
- **RemovedConstraint wrapper**: Complete parameter handling, original constraint access, convenience methods
- **Type Safety**: Full pyright type checking support with auto-generated stub files
- **Testing**: 15 comprehensive test cases covering all wrapper functionality
- **Performance**: Native Rust type usage eliminates Protocol Buffer conversion overhead

**Migration Progress**: 
- ✅ Mathematical functions (`Linear`, `Quadratic`, `Polynomial`, `Function`)
- ✅ Decision variable analysis (`DecisionVariableAnalysis`, `Bound`)
- ✅ DecisionVariable PyO3 wrapper (Phase 1 complete)
- ✅ Constraint PyO3 wrapper (Phase 2 complete)
- ✅ RemovedConstraint PyO3 wrapper (Phase 2 complete)
- 🔄 Instance migration (Phase 3 in progress - Rust API extension)
- 📋 Python Instance class migration (Phase 4 planned)
- Deprecated global evaluation functions removed

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

1. **Protocol Buffers Compatibility**: During the migration period, ensure proper use of `ParseFromString()` method when converting from Protocol Buffers messages to Rust implementations
2. **Test Coverage**: The test suite includes 44 tests covering core functionality, QUBO conversion, MPS format handling, decision variable analysis, constraint wrappers, and doctests
3. **Performance**: Core mathematical operations are implemented in Rust for optimal performance while maintaining Python usability
4. **Error Handling**: Rust implementations provide detailed error messages for debugging mathematical programming issues

## Development Guidance

### General Development Principles
- Rustのコードを変更する時は必ず小さい単位で変更を行い、都度cargo checkが通ることを確認してください
- Always run `task python:test` after making changes to ensure all tests pass
- Use incremental approach: implement one component at a time, test, then commit
- Maintain backward compatibility during migration phases

### Instance Migration Guidelines
When working on the Protocol Buffer to Rust Instance migration:

1. **Small Incremental Changes**: Add one PyO3 wrapper at a time (DecisionVariable ✅, Constraint ✅, RemovedConstraint ✅)
2. **Test-Driven Development**: Ensure `cargo check` passes and all tests pass before each commit
3. **Use Native Rust Types**: Prefer `ommx::DecisionVariable`, `ommx::Constraint` over Protocol Buffer types (ommx::v1::*)
4. **Proper Error Handling**: Use `anyhow::Result` for proper error propagation in PyO3 wrappers
5. **API Consistency**: Follow established patterns from completed wrapper implementations
6. **Type Safety**: Always regenerate stub files and run pyright after adding new wrappers

### Current Development Status (December 2024)
- **Phase 1 ✅**: DecisionVariable PyO3 wrapper with factory methods and proper type conversions
- **Phase 2 ✅**: Constraint and RemovedConstraint PyO3 wrappers with comprehensive functionality
- **Phase 3 🔄**: Next - Extend Rust Instance API with getter methods and from_components constructor
- **Key Achievements**: 44 tests passing, full type safety, native Rust performance
- **Key Principles**: Small changes, test coverage, and maintaining compatibility throughout the migration

## Memories
- ほとんどの作業はrootで行うように記述されています。ここのタスクが終わったらrootにcdするようにしてください