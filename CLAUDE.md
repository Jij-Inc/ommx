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

## Current Development: API Consistency Improvements

### Goal
Align Solution and SampleSet APIs with Instance's design pattern for better consistency and user experience.

### Implementation Plan

#### Phase 1: Rust SDK Updates ✅ COMPLETED
Add the following APIs to `rust/ommx/src/`:

**Solution class:**
1. ✅ `decision_variables` property - Returns `Vec<EvaluatedDecisionVariable>` sorted by ID
2. ✅ `constraints` property - Returns `Vec<EvaluatedConstraint>` sorted by ID
3. ✅ `get_decision_variable_by_id(id)` method - Returns specific `EvaluatedDecisionVariable`
4. ✅ `get_constraint_by_id(id)` method - Returns specific `EvaluatedConstraint`

**SampleSet class:**
1. ✅ `get_sample_by_id(id)` method - Alias for existing `get(id)` method
2. ✅ `samples` property - Returns `Vec<Solution>` sorted by sample ID
3. ✅ `decision_variables` property - Returns `Vec<SampledDecisionVariable>` sorted by ID (already existed)
4. ✅ `constraints` property - Returns `Vec<SampledConstraint>` sorted by ID (already existed)
5. ✅ `get_decision_variable_by_id(id)` method - Returns specific `SampledDecisionVariable`
6. ✅ `get_constraint_by_id(id)` method - Returns specific `SampledConstraint`

#### Phase 2: Python SDK Updates
Update `python/ommx/ommx/v1/__init__.py` to expose new Rust APIs:

**Solution class:**
- Add `decision_variables` property (wraps Rust `decision_variables`, returns list of `EvaluatedDecisionVariable`)
- Add `constraints` property (wraps Rust `constraints`, returns list of `EvaluatedConstraint`)
- Expose `get_decision_variable_by_id()` and `get_constraint_by_id()`
- Note: These methods return evaluated types while maintaining consistent naming with Instance

**SampleSet class:**
- Add `get_sample_by_id()` alias
- Add `samples` property
- Add `decision_variables` property (returns list of `SampledDecisionVariable`)
- Add `constraints` property (returns list of `SampledConstraint`)

#### Phase 3: Testing & Documentation
1. Add comprehensive tests in `python/ommx-tests/tests/`
2. Update API documentation
3. Update PYTHON_SDK_MIGRATION_GUIDE.md with new APIs

### Implementation Status

✅ **Phase 1 Complete**: All Rust SDK APIs implemented and stub files regenerated. Removed duplicate methods (`get_constraint_value`, `get_dual_variable`) in favor of accessing via `get_constraint_by_id()`
⏳ **Phase 2 Pending**: Python SDK wrapper implementation
⏳ **Phase 3 Pending**: Testing and documentation updates

### API Consistency Achievement

All three core classes now follow consistent patterns:

- **Instance**: `decision_variables` → `list[DecisionVariable]`, `get_decision_variable_by_id()` → `DecisionVariable`
- **Solution**: `decision_variables` → `list[EvaluatedDecisionVariable]`, `get_decision_variable_by_id()` → `EvaluatedDecisionVariable`  
- **SampleSet**: `decision_variables` → `list[SampledDecisionVariable]`, `get_decision_variable_by_id()` → `SampledDecisionVariable`

Same patterns apply to constraints and sample access methods.

### Design Principles
- **Backward Compatibility**: All existing APIs remain functional
- **Consistency**: Follow Instance's patterns (list properties, `get_*_by_id` methods)
- **Performance**: Use lazy evaluation and caching where appropriate
- **Type Safety**: Maintain strong typing throughout the stack