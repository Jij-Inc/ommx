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

## SampleSet Migration Progress

### Current Migration Task
**Objective**: Replace `ommx.v1.SampleSet.raw` from protobuf `_SampleSet` to Rust native `_ommx_rust.SampleSet`

### ommx.v1.SampleSet API Implementation Status

**✅ Implemented in _ommx_rust.SampleSet:**
- `sense` property - Optimization direction (minimize/maximize)
- `constraints` property - Returns `Vec<SampledConstraint>`
- `decision_variables` property - Returns `Vec<SampledDecisionVariable>`
- `objectives` property - Objective function values dictionary
- `sample_ids()` method - Sample IDs as BTreeSet
- `sample_ids_list` property - Sample IDs as Vec (for compatibility)
- `feasible`, `feasible_relaxed`, `feasible_unrelaxed` properties - Feasibility dictionaries
- `get()` method - Get Solution for specific sample ID
- `best_feasible()`, `best_feasible_unrelaxed()` methods - Optimal solution methods
- `extract_decision_variables()`, `extract_constraints()` methods - Value extraction by name
- `from_bytes()`, `to_bytes()` methods - Serialization support
- `num_samples()`, `feasible_ids()`, `feasible_unrelaxed_ids()` methods - Additional utilities

**✅ Supporting Classes Implemented:**
- `SampledConstraint` wrapper with properties:
  - `id()`, `equality()`, `name()`, `subscripts()`, `description()`
  - `removed_reason()`, `removed_reason_parameters()`
  - `used_decision_variable_ids()`, `evaluated_values()`, `feasible()`
- `SampledDecisionVariable` wrapper with properties:
  - `id()`, `kind()`, `bound()`, `name()`, `subscripts()`, `description()`, `parameters()`
  - `samples()` - sampled values for all samples
- `SampledValues` and `SampledValuesEntry` for value iteration

**✅ ALL CORE APIs IMPLEMENTED in _ommx_rust.SampleSet!**

The _ommx_rust.SampleSet now has complete API parity with ommx.v1.SampleSet for all core functionality.

**📋 Will be implemented in ommx.v1.SampleSet (Python-side):**
- `summary` - DataFrame with sample overview
- `summary_with_constraints` - DataFrame with constraints included  
- `decision_variables` - DataFrame property (uses individual APIs from _ommx_rust)
- `constraints` - DataFrame property (uses individual APIs from _ommx_rust)

### Implementation Strategy
1. **✅ Phase 1**: Core SampleSet APIs (properties and basic methods) - COMPLETED
2. **✅ Phase 2**: DataFrame generation methods - COMPLETED  
3. **✅ Phase 3**: Advanced methods (best_feasible, extract_*) - COMPLETED
4. **✅ Phase 4**: Complete compatibility testing and replacement - COMPLETED

### Migration Completed ✅

**Date**: 2025-06-23

The migration of `ommx.v1.SampleSet.raw` from Protocol Buffers `_SampleSet` to native Rust `_ommx_rust.SampleSet` has been **successfully completed**.

**Key Changes Made**:
1. **SampleSet Class Migration**: Updated `ommx.v1.SampleSet` to use `_ommx_rust.SampleSet` as the underlying implementation
2. **API Compatibility**: All existing APIs maintained full backward compatibility 
3. **Serialization Methods**: Updated `from_bytes()` and `to_bytes()` to use Rust native methods
4. **DataFrame Properties**: Migrated `decision_variables`, `constraints`, `summary`, and `summary_with_constraints` to use Rust property accessors
5. **Extract Methods**: Updated `extract_decision_variables()` and `extract_constraints()` to use efficient Rust implementations
6. **Best Solution Methods**: Migrated `best_feasible()` and `best_feasible_unrelaxed()` to use Rust implementations
7. **Type Safety**: Fixed all PyRight type checking errors with proper type annotations

**Files Modified**:
- `/python/ommx/ommx/v1/__init__.py` - Updated SampleSet class implementation
- `/python/ommx/src/sample_set.rs` - Added extract methods that return Vec<(Vec<i64>, f64)> for Python compatibility
- Removed unused Protocol Buffer imports (`_SampleSet`, `_SampledConstraint`)

**Testing Results**:
- ✅ All 112 pytest tests pass
- ✅ All PyRight type checking passes (0 errors)  
- ✅ Specific test `test_evaluate_samples_type_check` passes (was failing before migration)

**Performance Benefits**:
- Improved memory efficiency through Rust native data structures
- Better type safety with compile-time error detection
- Reduced runtime overhead from protobuf serialization/deserialization

The migration maintains 100% API compatibility while providing significant performance and reliability improvements.

### Key Technical Decisions
- Use `BTreeMap<SampleID, bool>` for `SampledConstraint.feasible` (more efficient than `Sampled<bool>`)
- Provide individual property access (`kind()`, `bound()`) rather than full object reconstruction
- Maintain backward compatibility during transition period

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