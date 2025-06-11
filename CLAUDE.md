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

3. **Phase 3: ✅ Ready** - Rust Instance API extension
   - ✅ Getter methods (`get_objective`, `get_sense`, `get_decision_variables`, `get_constraints`) already implemented
   - ✅ `from_components` constructor method already implemented in Rust Instance
   - ✅ Serialization methods (`to_bytes`, `from_bytes`) available

4. **Phase 4: ✅ Completed** - Python Instance migration
   - ✅ Replaced `Instance.raw` from Protocol Buffer `_Instance` to `_ommx_rust.Instance`
   - ✅ Updated property accessors to use Rust getters: `self.raw.sense`, `self.raw.objective`, `self.raw.decision_variables`, `self.raw.constraints`
   - ✅ Implemented new Rust SDK method: `ommx::Instance::set_objective()` with validation
   - ✅ Added PyO3 objective setter `_ommx_rust.Instance.objective = value`
   - ✅ Updated `from_components` to use `_ommx_rust.Instance.from_components`
   - ✅ Migrated all ParseFromString calls to direct Rust instance assignment
   - ✅ Updated getter methods while maintaining API compatibility (lists vs dicts)
   - ✅ UserAnnotationBase functionality remains unchanged (OMMX Artifact metadata)
   - ✅ Instance wrapper tests passing, core Instance functionality working

5. **Phase 5: 🔄 In Progress** - Final cleanup and validation
   - 🔄 Fix remaining ParametricInstance Protocol Buffer type conflicts
   - 📋 Complete pyright type checking success
   - 📋 Performance benchmarking and optimization

**Key Implementation Details**:
- Python classes are thin wrappers around Rust core types
- Protocol Buffers serialization/deserialization handled by Rust
- Mathematical operations (add, subtract, multiply) implemented in Rust
- Object-oriented evaluation API with instance methods for better encapsulation
- Use native Rust types (`ommx::DecisionVariable`, `ommx::Constraint`, `ommx::RemovedConstraint`) rather than Protocol Buffer types for better performance

**Phase 2 Completed Features**:
- **DecisionVariable wrapper**: Factory methods (binary, integer, continuous), property getters, proper type validation
- **Constraint wrapper**: Direct constructor, comprehensive metadata management, encode/decode methods, function access
- **RemovedConstraint wrapper**: Complete parameter handling, original constraint access, convenience methods, encode/decode methods
- **Metadata Management**: Full support for constraint names, descriptions, subscripts, and parameters with efficient Rust implementation
- **Type Safety**: Full pyright type checking support with auto-generated stub files
- **Testing**: Comprehensive test suites (221 test cases for constraint metadata) covering all wrapper functionality
- **Performance**: Native Rust type usage eliminates Protocol Buffer conversion overhead

**Migration Progress**: 
- ✅ Mathematical functions (`Linear`, `Quadratic`, `Polynomial`, `Function`)
- ✅ Decision variable analysis (`DecisionVariableAnalysis`, `Bound`)
- ✅ DecisionVariable PyO3 wrapper (Phase 1 complete)
- ✅ Constraint PyO3 wrapper (Phase 2 complete)
- ✅ RemovedConstraint PyO3 wrapper (Phase 2 complete)
- ✅ Rust Instance API extension (Phase 3 complete)
- ✅ Enum implementation (`Sense`, `Equality`) with type safety and Protocol Buffer conversion
- ✅ Python Instance class migration (Phase 4 complete - core functionality working)
- 🔄 Final cleanup and type checking (Phase 5 in progress)
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
2. **Test Coverage**: The test suite includes comprehensive tests covering core functionality, QUBO conversion, MPS format handling, decision variable analysis, constraint wrappers (221 test cases for metadata management), and doctests
3. **Performance**: Core mathematical operations are implemented in Rust for optimal performance while maintaining Python usability
4. **Error Handling**: Rust implementations provide detailed error messages for debugging mathematical programming issues

## Development Guidance

### General Development Principles
- When modifying Rust code, always make small incremental changes and ensure `cargo check` passes at each step
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

### Phase 4 Implementation Plan
The next phase involves migrating Python Instance class to use Rust implementation:

1. **Replace Instance.raw**: Change from Protocol Buffer `_Instance` to `_ommx_rust.Instance`
2. **Update Property Methods**: Modify getters (`objective`, `sense`, `decision_variables`, `constraints`) to call Rust methods
3. **Migrate from_components**: Update static method to use `_ommx_rust.Instance.from_components`
4. **Maintain API Compatibility**: Ensure existing test patterns continue to work
5. **Handle Annotations**: Address `UserAnnotationBase` functionality separately (Protocol Buffer dependent)

**Implementation Considerations**:
- All prerequisite PyO3 wrappers are complete and tested
- Rust Instance API provides all necessary methods
- Test compatibility verified across adapter packages
- Type conversion patterns established in existing wrappers

### Current Development Status (December 2024)
- **Phase 1 ✅**: DecisionVariable PyO3 wrapper with factory methods and proper type conversions
- **Phase 2 ✅**: Constraint and RemovedConstraint PyO3 wrappers with comprehensive metadata management, encode/decode methods, and full functionality
- **Phase 3 ✅**: Rust Instance API complete with all required methods (`from_components`, getters, serialization)
- **Enum Implementation ✅**: Type-safe `Sense` and `Equality` enums with Protocol Buffer conversion support
- **Phase 4 ✅**: Python Instance migration completed - `Instance.raw` successfully migrated from Protocol Buffer to `_ommx_rust.Instance`
- **Key Achievements**: 
  - Core Instance functionality working with Rust backend
  - Objective setter implemented with proper validation
  - All ParseFromString calls migrated to direct Rust instance assignment
  - Instance wrapper tests passing
  - Performance improvements from native Rust operations
- **Phase 5 🔄**: Final cleanup in progress - ParametricInstance type conflicts, complete pyright success

## Development Notes

### 🚫 Critical Prohibitions
- **NEVER use `cd` command** - Work from repository root directory only
- **NEVER create inline tests** - Add tests to python/ommx-tests/tests only
- **NEVER run `python -c` directly** - Use proper test framework
- **NEVER modify v1_ext directory** - Contains deprecated Protocol Buffer implementations

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

