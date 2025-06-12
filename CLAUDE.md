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

5. **Phase 5: ✅ Completed** - Final cleanup and validation
   - ✅ Fixed removed_constraints issue in QUBO conversion by implementing proper Rust SDK methods
   - ✅ Complete pyright type checking success (0 errors, 0 warnings)
   - ✅ All doctests passing including QUBO conversion examples
   - ✅ PyO3 deepcopy support implemented for all major classes

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
- ✅ Python Instance class migration (Phase 4 complete)
- ✅ Final cleanup and validation (Phase 5 complete)
- ✅ PyO3 deepcopy support for all major classes
- ✅ Deprecated global evaluation functions removed

**🎉 MIGRATION COMPLETED**: The Protocol Buffer to Rust migration is now complete. All Python SDK functionality now uses high-performance Rust implementations with maintained API compatibility.

## Recent Migration Work (December 2024)

### Python-MIP Adapter Migration to v2 API

**Completion Status**: ✅ Fully completed with API improvements and best practices established

**Work Summary**:
The Python-MIP Adapter was successfully migrated from Protocol Buffer-based v1 API to the new Rust-PyO3 based v2 API. This work involved comprehensive updates across all adapter components and established important best practices for Python SDK usage.

**Key Accomplishments**:

1. **Complete Adapter Migration**:
   - ✅ `adapter.py`: Full API migration with enum updates and Function method changes
   - ✅ `python_mip_to_ommx.py`: Constraint creation pattern updates
   - ✅ All test files: Updated to new API patterns
   - ✅ Doctests: Updated with new import patterns and API usage

2. **Technical Improvements**:
   - ✅ Fixed `State.encode()` → `State.SerializeToString()` usage
   - ✅ Resolved enum constant issues (`Sense.Maximize` vs `Instance.MAXIMIZE`)
   - ✅ Updated Function API usage (`.HasField()` → `.as_linear()`)
   - ✅ Fixed iteration patterns for dictionaries vs keys

3. **API Enhancement at Core Level**:
   - ✅ Extended `Instance.from_components()` to accept `ommx.v1.Function` directly
   - ✅ Added automatic type conversion in core SDK
   - ✅ Eliminated need for manual `V1Function.from_raw()` conversions

4. **Solution Class Constants**:
   - ✅ Added `OPTIMAL`, `NOT_OPTIMAL`, `LP_RELAXED` constants to Solution class
   - ✅ Eliminated need for `solution_pb2` imports in adapters

5. **Best Practices Established**:
   - ✅ Removed all `_ommx_rust` direct imports from Python-MIP Adapter
   - ✅ Added necessary APIs to Python SDK (`Function.degree()`, `Function.num_terms()`, `Function.as_linear()`)
   - ✅ Established pattern of extending Python SDK rather than using raw APIs

**Impact on Other Adapters**:
- The `Instance.from_components()` enhancement benefits all adapters
- Established best practice: avoid `_ommx_rust` imports, extend `ommx.v1` instead
- Migration guide updated with comprehensive examples and patterns

**Test Results**:
- ✅ 12/12 functional tests passing
- ✅ 0 Pyright type errors
- ✅ All doctest examples working
- ✅ README examples functional
- ✅ No `_ommx_rust` imports remaining

**Time Investment**: ~8 hours total
- Analysis and planning: 1 hour
- Core migration work: 3 hours
- Pyright error resolution: 1 hour
- API improvement implementation: 1 hour
- Best practices implementation: 2 hours

**Knowledge Captured**: 
- 12+ detailed technical insights documented in migration guide
- Established clear pattern for avoiding raw API usage
- Created comprehensive examples of Python SDK extension

### DecisionVariable Kind PyO3 Enum Implementation (December 2024)

**Completion Status**: ✅ Fully completed with PyO3 enum integration

**Work Summary**:
Implemented DecisionVariable's Kind as a PyO3 Enum similar to existing Equality and Sense enums, following the Protocol Buffer to Rust migration pattern.

**Key Accomplishments**:

1. **Kind PyO3 Enum Implementation**:
   - ✅ Added Kind enum in `python/ommx/src/enums.rs` with Binary, Integer, Continuous, SemiInteger, SemiContinuous variants
   - ✅ Implemented from_pb()/to_pb() conversion methods for Protocol Buffer compatibility
   - ✅ Added proper __repr__ and __str__ methods using Debug trait
   - ✅ Exported Kind enum in PyO3 module (`python/ommx/src/lib.rs`)

2. **Python Wrapper Integration**:
   - ✅ Updated DecisionVariable wrapper in `python/ommx/ommx/v1/__init__.py` to use Kind enum
   - ✅ Modified kind property to return Kind.from_pb(self.raw.kind)
   - ✅ Updated of_type method to use kind.to_pb() for Rust conversion
   - ✅ Fixed doctests to use capitalized Kind names (Binary, Integer, etc.)

3. **Type Safety and Compatibility**:
   - ✅ Maintains compatibility with integer constants (DecisionVariable.BINARY still works)
   - ✅ Supports PyO3 enum comparison (Kind.Binary == Kind.Binary)
   - ✅ Generated proper type stubs via task python:stubgen

**Technical Details**:
- Used Debug trait instead of Display for string representation consistency
- Removed unnecessary _kind helper function as per user feedback
- Fixed constraint equality usage to use Constraint.EQUAL_TO_ZERO constants
- Updated all doctests from lowercase to capitalized Kind names

**Test Results**:
- ✅ 57/57 tests passing
- ✅ 0 Pyright type errors
- ✅ All doctests updated and passing
- ✅ Type safety maintained across all use cases

### Python-MIP Adapter Property Access Fix (December 2024)

**Issue**: Tests failing with `TypeError: 'float' object is not callable` due to incorrect method calls

**Root Cause**: Linear class properties `constant_term` and `linear_terms` were being called as methods with parentheses

**Solution**: Updated test files to use property access pattern:
- ✅ `linear_func.constant_term()` → `linear_func.constant_term` (property)
- ✅ `linear_func.linear_terms()` → `linear_func.linear_terms` (property)

**Files Updated**:
- `python/ommx-python-mip-adapter/tests/test_model_to_instance.py` - 6 instances fixed

**Test Results**:
- ✅ 12/12 tests passing, 2 skipped
- ✅ 0 Pyright type errors
- ✅ All doctests and README examples working

**Migration Guide Impact**:
- Added Problem 5 to PYTHON_SDK_MIGRATION_GUIDE.md documenting this common error
- Updated Linear class property documentation
- Emphasized property vs method access patterns

### Migration Guide Updates

**PYTHON_SDK_MIGRATION_GUIDE.md** has been significantly enhanced with:
- ✅ Clear guidance on avoiding `_ommx_rust` imports
- ✅ Examples of extending Python SDK with needed APIs
- ✅ Complete migration patterns from v1 to v2
- ✅ Best practices section emphasizing unified API usage

## Next Steps and Future Work

### Immediate Tasks

1. **Other Adapter Migrations**:
   - [ ] HiGHS Adapter - Apply same patterns as Python-MIP
   - [ ] PySCIPOpt Adapter - May need careful handling due to complexity  
   - [ ] OpenJij Adapter - Should be straightforward

2. **Python SDK Enhancements**:
   - [ ] Add `Function.as_quadratic()` method
   - [ ] Add `Function.as_polynomial()` method
   - [ ] Consider adding more convenience methods based on adapter needs

3. **Documentation**:
   - [ ] Update all adapter README files with v2 API examples
   - [ ] Create adapter development guide
   - [ ] Add migration examples to main documentation

### Completed Work Summary (December 2024)

**Major Achievements**:
1. ✅ **DecisionVariable Kind PyO3 Enum**: Complete implementation with type safety
2. ✅ **Python-MIP Adapter Property Fix**: Resolved property vs method access issues
3. ✅ **Migration Guide Enhancement**: Added Linear/Quadratic property access patterns
4. ✅ **Type Safety Improvements**: PyO3 enums with proper Debug trait usage

**Key Learnings Documented**:
- Property access patterns for Linear/Quadratic classes
- PyO3 enum implementation following established patterns
- Importance of maintaining API compatibility during migrations
- Debug trait usage for consistent string representation

### Long-term Goals

1. **API Consistency**:
   - Ensure all mathematical objects have consistent APIs
   - Add missing convenience methods based on usage patterns
   - Consider deprecating `.raw` attribute access in future versions

2. **Performance Optimization**:
   - Profile common operations and optimize hot paths
   - Consider caching for frequently accessed properties
   - Optimize Protocol Buffer conversions where still needed

3. **Developer Experience**:
   - Improve error messages for common mistakes
   - Add more type hints and documentation
   - Create adapter template/generator tool

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
2. **Protocol Buffers Compatibility**: During the migration period, ensure proper use of `ParseFromString()` method when converting from Protocol Buffers messages to Rust implementations
3. **Test Coverage**: The test suite includes comprehensive tests covering core functionality, QUBO conversion, MPS format handling, decision variable analysis, constraint wrappers (221 test cases for metadata management), and doctests
4. **Performance**: Core mathematical operations are implemented in Rust for optimal performance while maintaining Python usability
5. **Error Handling**: Rust implementations provide detailed error messages for debugging mathematical programming issues

## Development Guidance

### General Development Principles
- When modifying Rust code, always make small incremental changes and ensure `cargo check` passes at each step
- Always run `task python:test` after making changes to ensure all tests pass
- Use incremental approach: implement one component at a time, test, then commit
- Maintain backward compatibility during migration phases
- **New**: Prefer extending Python SDK over using raw APIs - this ensures API stability and better user experience

### Instance Migration Guidelines
When working on the Protocol Buffer to Rust Instance migration:

1. **Small Incremental Changes**: Add one PyO3 wrapper at a time (DecisionVariable ✅, Constraint ✅, RemovedConstraint ✅)
2. **Test-Driven Development**: Ensure `cargo check` passes and all tests pass before each commit
3. **Use Native Rust Types**: Prefer `ommx::DecisionVariable`, `ommx::Constraint` over Protocol Buffer types (ommx::v1::*)
4. **Proper Error Handling**: Use `anyhow::Result` for proper error propagation in PyO3 wrappers
5. **API Consistency**: Follow established patterns from completed wrapper implementations
6. **Type Safety**: Always regenerate stub files and run pyright after adding new wrappers

### Adapter Migration Guidelines

When migrating solver adapters to v2 API:

1. **Import Updates**: Replace all Protocol Buffer and `_ommx_rust` imports with `ommx.v1` imports
2. **API Usage**: Use Python SDK methods instead of raw API calls
3. **Type Conversions**: Let Python SDK handle conversions between Rust and Python types
4. **Extension Pattern**: If needed functionality is missing, add it to Python SDK classes
5. **Testing**: Ensure all tests pass including doctests and pyright checks

**Example Pattern**:
```python
# Bad: Using raw API
from ommx._ommx_rust import Function
function.as_linear()  # Direct Rust method

# Good: Using Python SDK
from ommx.v1 import Function
function.as_linear()  # Python SDK method that wraps Rust
```

### Current Development Status (December 2024)
- **Core Migration ✅**: All phases of Protocol Buffer to Rust migration complete
- **Python-MIP Adapter ✅**: Fully migrated with best practices established
- **Migration Guide ✅**: Comprehensive guide with examples and patterns
- **API Extensions ✅**: Function class extended with needed methods
- **DecisionVariable Kind PyO3 Enum ✅**: Implemented Kind enum with Debug trait support
- **Linear/Quadratic Property Access ✅**: Fixed constant_term()/linear_terms() method calls to property access
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

