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

### Python SDK v2 Migration Completed ✅

The project has completed its migration from Protocol Buffers auto-generated Python classes to high-performance Rust implementations with PyO3 bindings:

### Rust SDK v2 Design (Completed ✅)

The Rust SDK v2 introduces strongly-typed Rust alternatives to protobuf-generated `ommx::v1::*` types:

**Design Philosophy**:
- Replace `ommx::v1::*` (protobuf auto-generated) with `ommx::*` (Rust native types)
- Improve type safety and reduce runtime errors
- Enable efficient data structures for deduplication

**Implemented Types**:
- ✅ `Sampled<T>` - Efficient representation for `ommx::v1::Samples` with deduplication
  - `SampleID(u64)` - Type-safe sample identifier
  - Supports both `Sampled<v1::State>` and `Sampled<f64>` 
  - Efficient storage: multiple sample IDs can share the same data

**Constraint Types Implementation (Completed ✅)**:

```rust
// Auxiliary metadata for constraints (excluding essential id and equality)
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ConstraintMetadata {
    pub name: Option<String>,
    pub subscripts: Vec<i64>,
    pub parameters: FnvHashMap<String, String>,
    pub description: Option<String>,
    pub used_decision_variable_ids: Vec<u64>,
    pub removed_reason: Option<String>,
    pub removed_reason_parameters: FnvHashMap<String, String>,
}

// Single evaluation result with data integrity guarantees
#[derive(Debug, Clone, PartialEq, Getters)]
pub struct EvaluatedConstraint {
    #[getset(get = "pub")]
    id: ConstraintID,                    // Essential: constraint identifier
    #[getset(get = "pub")]
    equality: Equality,                  // Essential: constraint type (== 0 or <= 0)
    pub metadata: ConstraintMetadata,    // Auxiliary metadata
    #[getset(get = "pub")]
    evaluated_value: f64,                // Protected: evaluation result
    #[getset(get = "pub")]
    dual_variable: Option<f64>,          // Protected: dual variable value
    #[getset(get = "pub")]
    feasible: bool,                      // Protected: pre-computed feasibility
}

// Multiple sample evaluation results with deduplication
#[derive(Debug, Clone, Getters)]
pub struct SampledConstraint {
    #[getset(get = "pub")]
    id: ConstraintID,                    // Essential: constraint identifier
    #[getset(get = "pub")]
    equality: Equality,                  // Essential: constraint type
    pub metadata: ConstraintMetadata,    // Auxiliary metadata
    #[getset(get = "pub")]
    evaluated_values: Sampled<f64>,      // Protected: evaluation results
    #[getset(get = "pub")]
    dual_variables: Option<Sampled<f64>>, // Protected: dual variable values
    #[getset(get = "pub")]
    feasible: FnvHashMap<u64, bool>,     // Protected: feasibility map
}
```

**Key Design Decisions**:
- **Data Integrity**: Essential fields (`id`, `equality`) and evaluation data are private with getters only
- **Metadata Separation**: `ConstraintMetadata` contains only auxiliary information, not essential constraint properties
- **Feasibility Pre-computation**: `feasible` field stores pre-computed feasibility to avoid repeated calculations
- **Type Safety**: Uses `getset` crate for clean getter methods while preventing external modification
- **Efficient Storage**: `Sampled<f64>` enables deduplication when multiple samples share results

**Benefits**:
- **Data Integrity**: Prevents external modification of critical constraint evaluation data
- **Performance**: Pre-computed feasibility avoids repeated tolerance-based calculations
- **Type Safety**: Strong typing with private fields and controlled access via getters
- **Memory Efficiency**: `Sampled<T>` enables efficient storage with deduplication
- **Clean API**: Separation of essential properties from auxiliary metadata

**Core Features Completed**:
- ✅ All mathematical objects (`Linear`, `Quadratic`, `Polynomial`, `Function`) use Rust implementations
- ✅ Instance class fully migrated to Rust backend with maintained API compatibility
- ✅ All solver adapters (Python-MIP, PySCIPOpt, HiGHS) migrated to v2 API
- ✅ Type-safe PyO3 enums (`Sense`, `Equality`, `Kind`) with Protocol Buffer conversion
- ✅ Comprehensive testing and documentation updated
- ✅ **Constraint evaluation system with data integrity guarantees**
- ✅ **Sampled data structures with efficient deduplication**
- ✅ **Parse trait implementations for Protocol Buffer conversion**

**Recently Implemented**:
- ✅ `Sampled<T>` with `get`/`get_mut` methods and `UnknownSampleIDError` error handling
- ✅ `EvaluatedConstraint` and `SampledConstraint` with private fields and getset getters
- ✅ `ConstraintMetadata` separation for auxiliary data with `Default` implementation
- ✅ Pre-computed feasibility fields to improve performance
- ✅ `Parse` trait implementations for `v1::EvaluatedConstraint` and `v1::SampledConstraint`
- ✅ Type-safe constraint evaluation with proper error handling
- ✅ Efficient constraint feasibility checking methods (`feasible_ids`, `infeasible_ids`)
- ✅ **Solution and SampleSet types with data integrity guarantees**
- ✅ **Parse trait implementations for v1::Solution and v1::SampleSet**
- ✅ **Instance evaluation methods updated to use new Solution/SampleSet types**
- ✅ **Constructor methods for controlled Solution/SampleSet creation**
- ✅ **Comprehensive round-trip testing for Solution and SampleSet parsing**
- ✅ **Flattened Solution structure with optimality and relaxation as direct fields**
- ✅ **Improved error handling with UnknownEnumValue for better diagnostics**

**Solution and SampleSet Implementation (Completed ✅)**:

Following the same design principles as constraint types, strongly-typed Solution and SampleSet alternatives have been implemented:

```rust
// Single solution result with data integrity guarantees  
#[derive(Debug, Clone, PartialEq, Getters)]
pub struct Solution {
    #[getset(get = "pub")]
    state: v1::State,                        // Essential: variable assignments
    #[getset(get = "pub")]
    objective: f64,                          // Essential: objective value
    #[getset(get = "pub")]
    evaluated_constraints: Vec<EvaluatedConstraint>, // Protected: constraint evaluations
    #[getset(get = "pub")]
    decision_variables: Vec<v1::DecisionVariable>,   // Protected: decision variable values
    #[getset(get = "pub")]
    feasible: bool,                          // Protected: overall feasibility
    #[getset(get = "pub")]
    feasible_relaxed: bool,                  // Protected: relaxed feasibility
    #[getset(get = "pub")]
    optimality: v1::Optimality,              // Protected: optimality status
    #[getset(get = "pub")]
    relaxation: v1::Relaxation,              // Protected: relaxation status
}

// Multiple sample solution results with deduplication
#[derive(Debug, Clone, Getters)]
pub struct SampleSet {
    #[getset(get = "pub")]
    decision_variables: Vec<v1::SampledDecisionVariable>, // Protected: sampled variables
    #[getset(get = "pub")]
    objectives: Option<Sampled<f64>>,        // Protected: objective values
    #[getset(get = "pub")]
    constraints: Vec<SampledConstraint>,     // Protected: constraint evaluations
    #[getset(get = "pub")]
    feasible_relaxed: FnvHashMap<u64, bool>, // Protected: relaxed feasibility map
    #[getset(get = "pub")]
    feasible: FnvHashMap<u64, bool>,         // Protected: strict feasibility map
    #[getset(get = "pub")]
    sense: Sense,                            // Essential: optimization sense
}
```

**Key Design Decisions**:
- **Data Integrity**: Solution evaluation results are private with getters only
- **Metadata Separation**: `SolutionMetadata` contains only auxiliary information like optimality status
- **Feasibility Pre-computation**: Both `feasible` and `feasible_relaxed` stored to avoid repeated calculations
- **Type Safety**: Uses `getset` crate for clean getter methods while preventing external modification
- **Efficient Storage**: `Sampled<f64>` for objectives enables deduplication when multiple samples share results

**Benefits Achieved**:
- **Data Integrity**: Prevents external modification of critical solution evaluation data
- **Performance**: Pre-computed feasibility avoids repeated constraint checks
- **Type Safety**: Strong typing with private fields and controlled access via getters  
- **Memory Efficiency**: `Sampled<T>` enables efficient storage with deduplication
- **Clean API**: Separation of essential solution properties from auxiliary metadata

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

**⚠️ Important**: All `task` commands must be run from the project root directory. The task command searches for `Taskfile.yml` from the current directory, so available commands will vary depending on your current location. Always ensure you are in the repository root before running any task commands.

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
task rust:test          # Rust tests only
task python:test-ci     # CI mode (no pyright for main ommx package)
```

**Code Quality:**
```bash
# Format all code (Python and Rust)
task format

# Python formatting
task python:format

# Python linting
task python:lint         # Run ruff check on all Python packages

# Rust checks
task rust:check         # Run cargo check
task rust:clippy        # Run clippy linting
task rust:format        # Format Rust code

# Type checking (Python)
task python:ommx:pyright
```

**Building and Documentation:**
```bash
# Generate code from protobuf
task proto              # Generate all (Rust and Python)
task proto:python       # Generate Python code only
task proto:rust         # Generate Rust code only
task proto:doc          # Generate documentation from protobuf

# Build documentation
task api_reference      # Python API docs (build and open)
task rust:doc           # Rust docs
task book_en            # English Jupyter Book (build and open)
task book_ja            # Japanese Jupyter Book (build and open)

# Additional documentation commands
task api_reference:build       # Build Python API docs only
task api_reference:open        # Open Python API docs
task book_en:build            # Build English book only
task book_en:open             # Open English book
task book_en:watch            # Watch and rebuild English book
task book_ja:build            # Build Japanese book only
task book_ja:open             # Open Japanese book
task book_ja:watch            # Watch and rebuild Japanese book
```

**Other Useful Commands:**
```bash
# Python benchmarks
task python:bench              # Run benchmarks for OMMX Python SDK

# Generate stubs for Rust extension
task python:stubgen

# Version management
task python:set-version        # Set version for all Python packages
task rust:set-version          # Set version for Rust SDK

# Codspeed benchmarks
task codspeed:list            # List all Codspeed workflows
task codspeed:trigger         # Trigger GitHub Actions workflow

# Generate LLMs.txt
task python:generate-llms-txt
```

### Package-Specific Commands

**Core OMMX:**
- `task python:ommx:test` - Test main Python package
- `task python:ommx:test-ci` - Test without pyright (CI mode)
- `task python:ommx:pytest` - Run pytest only
- `task python:ommx:pyright` - Type check only
- `task python:ommx:lint` - Lint check only
- `task python:ommx:bench` - Run benchmarks

**Adapters:**
- `task python:ommx-openjij-adapter:test`
- `task python:ommx-python-mip-adapter:test`
- `task python:ommx-pyscipopt-adapter:test`
- `task python:ommx-highs-adapter:test`

Each adapter also has individual commands for:
- `:lint` - Run ruff check
- `:pyright` - Type checking
- `:pytest` - Run tests
- `:markdown-code-runner` - Run markdown code examples (Python-MIP, PySCIPOpt, HiGHS only)

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

### Current Development Status
- **Core Migration ✅**: Protocol Buffer to Rust migration completed across all components
- **Adapter Support ✅**: All major adapters (Python-MIP, PySCIPOpt, HiGHS) migrated to v2 API
- **Documentation ✅**: Comprehensive migration guide and adapter specifications available
- **API Stability ✅**: Unified `ommx.v1` API established with proper extension patterns
- **Performance ✅**: Rust backend providing optimal performance for mathematical operations
- **Constraint System ✅**: Complete constraint evaluation system with data integrity guarantees
- **Type Safety ✅**: Strongly-typed constraint implementations with private fields and getters
- **Parse Integration ✅**: Full Protocol Buffer to Rust type conversion via Parse trait

## Development Notes

### 🚫 Critical Prohibitions
- **NEVER use `cd` command** - Work from repository root directory only
- **NEVER create inline tests** - Add tests to python/ommx-tests/tests only
- **NEVER run `python -c` directly** - Use proper test framework
- **NEVER modify v1_ext directory** - Contains deprecated Protocol Buffer implementations
- **NEVER import from `_ommx_rust` in adapters** - Use `ommx.v1` unified API instead

### 📁 Directory Guidelines  
- **All tasks MUST be performed from the repository root directory**
- Task commands require being in the root directory to access the main `Taskfile.yml`
- Return to root after completing any subtasks
- Use absolute paths when referencing files across packages
- Never use `cd` to navigate to subdirectories when running task commands

### 🧪 Testing Guidelines
- Add new Python test code to python/ommx-tests/tests directory only
- Run tests using pytest through task commands
- Verify test compatibility across adapter packages before committing

### 🔧 Code Quality
- Always run `task format` before committing changes
- Ensure `task python:test` passes completely
- Follow incremental development: small changes → test → commit