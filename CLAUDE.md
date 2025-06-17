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

## Current Implementation Status

### Python SDK v2 Migration Status

The project has completed its core migration from Protocol Buffers auto-generated Python classes to high-performance Rust implementations with PyO3 bindings. Additional performance optimizations are ongoing.

**Completed Core Migration ✅**:
- ✅ All mathematical objects (`Linear`, `Quadratic`, `Polynomial`, `Function`) use Rust implementations
- ✅ Instance class fully migrated to Rust backend with maintained API compatibility
- ✅ All solver adapters (Python-MIP, PySCIPOpt, HiGHS) migrated to v2 API
- ✅ Type-safe PyO3 enums (`Sense`, `Equality`, `Kind`) with Protocol Buffer conversion
- ✅ Comprehensive testing and documentation updated

**Ongoing Performance Migration 🔄**:
- ⏳ PyO3 implementations for `Solution`, `SampleSet`, and `State` to eliminate serialization overhead
- ⏳ Enhanced properties and methods for better Python API integration

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

This project uses [Taskfile](https://taskfile.dev/) for task management. **Always run task commands from the project root directory**. Run `task -l` to see all available commands.

### Essential Commands

**Setup and Dependencies:**
```bash
# From project root: /Users/termoshtt/github.com/Jij-Inc/ommx
task python:sync        # Setup Python development environment, rebuild Python SDK
task python:upgrade     # Upgrade uv dependencies
```

**Testing:**
```bash
# Run all tests (from project root)
task python:test        # Python tests (includes linting, type checking, and pytest)
task rust:test         # Rust tests only
task python:test-ci     # CI mode (no pyright for main ommx package)
```

**Code Quality:**
```bash
# Format and linting (from project root)
task format             # Format all code (Python + Rust)
task python:format      # Format Python code only
task python:lint        # Run ruff check on all Python packages
task rust:clippy        # Run clippy for Rust SDK
task python:ommx:pyright # Type checking for main Python package
```

**Building and Documentation:**
```bash
# Generate code from protobuf (from project root)
task proto              # Generate code from Protobuf definitions

# Build documentation (from project root)
task api_reference      # Build and open Python API docs
task rust:doc          # Generate and open Rust docs
task book_en           # Build and open English Jupyter Book
task book_ja           # Build and open Japanese Jupyter Book
```

### Package-Specific Commands

**Core OMMX:**
- `task python:ommx:test` - Run tests for OMMX Python SDK
- `task python:ommx:pytest` - Run pytest only (no type checking)
- `task python:ommx:test-ci` - CI mode (without pyright)
- `task python:ommx:bench` - Run benchmarks for OMMX Python SDK

**Adapters:**
- `task python:ommx-openjij-adapter:test` - Run all tests for OMMX OpenJij Adapter
- `task python:ommx-python-mip-adapter:test` - Run all tests for OMMX Python-MIP Adapter  
- `task python:ommx-pyscipopt-adapter:test` - Run all tests for OMMX PySCIPOpt Adapter
- `task python:ommx-highs-adapter:test` - Run all tests for OMMX Highs Adapter

**Development Utilities:**
- `task python:stubgen` - Generate stubs files for Rust extension
- `task python:bench` - Run benchmarks for OMMX Python SDK
- `task codspeed:trigger` - Trigger GitHub Actions workflow for Codspeed on current branch

## Testing Strategy

**Python Testing:**
- Main package: ruff linting + pytest + pyright type checking + doc tests
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
3. **Test Coverage**: The test suite includes comprehensive tests covering core functionality, QUBO conversion, MPS format handling, decision variable analysis, constraint wrappers, and doc tests
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
5. **Testing**: Ensure all tests pass including doc tests and pyright checks

**Example Pattern**:
```python
# Good: Using Python SDK
from ommx.v1 import Instance, DecisionVariable, Function, Solution

# Bad: Using raw or Protocol Buffer APIs
from ommx._ommx_rust import Function
from ommx.v1.solution_pb2 import Optimality
```

### Current Development Status
- **Core Migration ✅**: Protocol Buffer to Rust migration completed across all components
- **Adapter Support ✅**: All major adapters (Python-MIP, PySCIPOpt, HiGHS) migrated to v2 API
- **Documentation ✅**: Comprehensive migration guide and adapter specifications available
- **API Stability ✅**: Unified `ommx.v1` API established with proper extension patterns
- **Performance Optimization 🔄**: Ongoing PyO3 performance improvements for data structures

## PyO3 Performance Enhancement Plan

### Background
Benchmark results for `evaluate_samples` revealed performance optimization opportunities through better PyO3 integration. We are enhancing the existing PyO3 implementations with richer functionality to eliminate unnecessary serialization overhead.

### Current PyO3 Implementation Status

**Basic PyO3 Wrappers (Need Enhancement)**:
- ⏳ `Solution` - Basic wrapper, needs core properties (objective, state, feasible, etc.)
- ⏳ `Samples` - Basic wrapper with `from_bytes`/`to_bytes`  
- ⏳ `SampleSet` - Basic wrapper, needs enhanced functionality (objectives dict, feasible dicts, etc.)
- ⏳ `State` - PyO3 implementation exists but reverted to protobuf, needs re-migration

**Enhancement Goals**:
- Rich Python properties for all PyO3 data structures
- Direct memory access between Python and Rust without serialization overhead
- Maintaining API compatibility through incremental migration

### Migration Strategy

**Phase 1: Enhance PyO3 Implementations** ✅
- ✅ Split PyO3 classes into separate files for better organization
  - ✅ Split message.rs into linear.rs, quadratic.rs, polynomial.rs, function.rs
  - ✅ Extract Solution, SampleSet, Samples from instance.rs into separate files
- ✅ Add core properties to `_ommx_rust.Solution` (objective, state, feasible status, etc.)
- ✅ Add core properties to `_ommx_rust.SampleSet` (objectives dict, feasible dicts, etc.)
- ✅ `_ommx_rust.State` already has PyO3 implementation with full functionality

**Phase 2: Incremental Python SDK Migration** 🔄
- ✅ **COMPLETED**: Migrate ommx.v1.State to use _ommx_rust.State via .raw
  - Strategy: Replace protobuf import with _ommx_rust.State, fix errors iteratively
  - ✅ Changed import: `State = _ommx_rust.State` in __init__.py
  - ✅ **Fixed all pyright errors (13 total)**:
    - ✅ SerializeToString() → to_bytes() method calls (9 locations)
    - ✅ ParseFromString() → from_bytes() method calls (1 location)
    - ✅ Type mismatches in protobuf Solution construction (2 locations)
    - ✅ Incompatible State type in return (1 location)
  - ✅ Added helper function `_state_to_protobuf()` for legacy compatibility
  - ✅ All pyright errors resolved (0 errors)
  - ✅ All 98 tests pass - migration successful!
  - ✅ Fixed import order to resolve lint errors (E402)
  - ✅ **ADAPTER FIXES COMPLETED**: All major adapters (OpenJij, PyScipOpt) fixed for State constructor changes
  - ✅ **State Constructor Enhancement**: Enhanced to accept both `dict[int, float]` and `Iterable[tuple[int, float]]`
- ⏳ Update ommx.v1.Solution to use PyO3 properties via .raw  
- ⏳ Update ommx.v1.SampleSet to use PyO3 properties via .raw
- Maintain serialization compatibility for safe migration
- Keep all tests passing throughout the process

### API Evolution Pattern

**Enhanced Solution Access**:
```python
# Current (basic PyO3 wrapper)
solution = Solution.from_bytes(bytes_data)

# Target (rich PyO3 properties)
solution.objective    # Direct property access
solution.state        # Direct State object
solution.feasible     # Direct boolean
```

**Enhanced SampleSet Access**:
```python
# Current (basic methods)
sample_set.get(sample_id)  # Returns Solution

# Target (rich properties) 
sample_set.objectives      # Dict[int, float]
sample_set.feasible        # Dict[int, bool]
```

### Current Analysis: Solution and SampleSet Migration

**Current State (Based on Analysis)**:
- `State` - 🔄 **IN PROGRESS**: PyO3 implementation exists, currently migrating Python SDK wrapper
- `Solution`, `Samples`, `SampleSet` - ✅ Enhanced PyO3 implementations with rich properties completed

**Current PyO3 Implementation Status**:
```rust
// Current _ommx_rust implementations (basic wrappers only)
class Solution:
    @staticmethod
    def from_bytes(bytes: bytes) -> Solution
    def to_bytes(self) -> bytes

class SampleSet:
    @staticmethod 
    def from_bytes(bytes: bytes) -> SampleSet
    def to_bytes(self) -> bytes
    def get(self, sample_id: int) -> Solution
    def num_samples(self) -> int
    def sample_ids(self) -> set[int]
    def feasible_ids(self) -> set[int]
    def feasible_unrelaxed_ids(self) -> set[int]
    def best_feasible(self) -> Solution
    def best_feasible_unrelaxed(self) -> Solution

class Samples:
    @staticmethod
    def from_bytes(bytes: bytes) -> Samples
    def to_bytes(self) -> bytes
```

**Python SDK Current Implementation Pattern**:
```python
# Current Python wrapper pattern
class Solution:
    def __init__(self, raw: _Solution):
        self.raw = raw  # Protocol Buffer message
    
    @property
    def objective(self) -> float:
        return self.raw.objective
    
    @property
    def state(self) -> State:
        return self.raw.state
```

**Key Attributes Needed in PyO3 Solution**:
- `objective: float` - Objective function value
- `state: State` - Decision variable values  
- `feasible: bool` - Feasibility status
- `feasible_relaxed: Optional[bool]` - Relaxed feasibility
- `feasible_unrelaxed: bool` - Unrelaxed feasibility
- `optimality: Optimality` - Optimization status enum
- `relaxation: Relaxation` - Relaxation status enum

**Key Attributes Needed in PyO3 SampleSet**:
- `sample_ids: set[int]` - ✅ Already implemented
- `objectives: dict[int, float]` - Objective values per sample
- `feasible: dict[int, bool]` - Feasibility per sample  
- `feasible_relaxed: dict[int, Optional[bool]]` - Relaxed feasibility per sample
- `feasible_unrelaxed: dict[int, bool]` - Unrelaxed feasibility per sample

### Migration Steps for Adapters

**🔄 Serialization Compatibility**: PyO3 implementations can always convert to/from their corresponding protobuf structures through serialization (`to_bytes()` / `from_bytes()`). This allows for incremental migration while keeping tests passing throughout the process.

1. **Phase 1**: Complete all `_ommx_rust.*` PyO3 implementations
   - Add core properties to PyO3 Solution (objective, state, feasible, etc.)
   - Add core properties to PyO3 SampleSet (objectives, feasible dicts, etc.)
   - Add core properties to PyO3 State (migrate from protobuf)
   - Ensure all PyO3 classes have feature parity with current protobuf usage

2. **Phase 2**: Incremental Python SDK migration (keeping tests passing)
   - Update `ommx.v1.*` classes one by one to use PyO3 properties via `.raw`
   - Use serialization compatibility to ensure tests continue passing during migration
   - Update imports and remove protobuf dependencies gradually
   - Update tests to use new API patterns as each component is migrated

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

### 🛠️ Development Techniques
- **PyO3 Migration Pattern**: When migrating from protobuf to PyO3 implementations:
  1. Change import statements (e.g., `State = _ommx_rust.State`)
  2. Run `task python:ommx:pyright` to get precise error locations
  3. Fix errors iteratively: `SerializeToString()` → `to_bytes()`, `ParseFromString()` → `from_bytes()`
  4. Add compatibility helpers for legacy protobuf interactions
  5. Verify with `task python:ommx:pytest` that all tests pass