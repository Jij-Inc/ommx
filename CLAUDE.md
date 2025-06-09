# OMMX Project Information for Claude Code

## Project Overview
OMMX is a library for standardizing mathematical optimization data formats using Protocol Buffers. The core logic is implemented in Rust with Python wrappers, creating a mixed Rust/Python project using PyO3/Maturin.

## Core Technologies & Languages
- **Primary**: Rust and Python
- **Serialization**: Protocol Buffers (.proto files)
- **Python Integration**: PyO3/Maturin for Rust-Python bindings
- **Dependency Management**: `uv` for Python, `cargo` for Rust
- **Task Management**: Taskfile.yml

## Architecture Principles

### Rust SDK Design (v2.0.0+)
- **Idiomatic Rust structs** (e.g., `ommx::Instance`, `ommx::Function`) for core logic and public APIs
- **Protobuf-generated structs** (e.g., `ommx::v1::Instance`, `ommx::v1::Function`) only for serialization/deserialization

### Data Flow
**Reading OMMX data:**
1. Deserialize binary → Protobuf structs (`ommx::v1::Instance`)
2. Convert → Idiomatic Rust structs (`ommx::Instance`)
3. Use idiomatic structs for all operations

**Writing OMMX data:**
1. Convert idiomatic structs → Protobuf structs
2. Serialize Protobuf structs → Binary data

## Repository Structure
- **`proto/`**: Protocol Buffer definitions
- **`python/`**: Python SDK and adapter packages
  - `ommx/`: Core Python SDK (mixed Rust/Python with PyO3/Maturin)
  - `ommx-*-adapter/`: Solver adapter packages
- **`rust/`**: Rust crates
  - `ommx/`: Core Rust implementation
  - `protogen/`: Code generation utilities
  - `dataset/`: Dataset conversion tools
- **`docs/`**: Documentation (API reference, user guides)

## Testing & Build Commands

### Python
- `task python` - Sync dependencies and run all Python tests
- `task python:test` - Run tests for all Python projects
- `task python:sync` - Setup development environment and rebuild SDK
- `task python:format` - Format code with ruff
- `task python:ommx:test` - Test main SDK (pytest + pyright)
- `task python:ommx:pytest` - Run pytest only
- `task python:ommx:pyright` - Type checking only

### Rust
- `task rust:test` - Run Rust tests
- `task rust:check` - Run cargo check
- `task rust:clippy` - Run clippy linter
- `task rust:doc` - Generate documentation

### Other Useful Commands
- `task python:stubgen` - Generate stub files for Rust extension
- `task python:set-version` - Set version for Python projects
- `task rust:set-version` - Set version for Rust SDK

## Development Guidelines

### Coding Standards
- Write all comments in English
- Add comments only for complex logic or reasoning ("why", not "what")
- Follow idiomatic practices: Rust standard library patterns, Python PEP 8
- Use type hints in Python code

### Rust Development
- Always use `Coefficient::try_from(value).unwrap()` or `coeff!(value)` macro for coefficients
- Prefer convenience macros in tests: `linear!()`, `quadratic!()`, `monomial!()`, `assign!()`
- Use property-based testing with `proptest` and existing `arbitrary_xxx` functions
- Compare floating-point math structures with `approx` crate for tolerance-based equality

### Python Development
- Core `ommx` package serves as dependency for adapter projects
- Main module: `python/ommx/ommx/__init__.py`
- Rust components available as `ommx._ommx_rust` submodule
- Use `uv` for all dependency management

## Current Development Context
- Working on architectural transition from Protobuf-generated structs to idiomatic Rust structs
- Legacy code may still use `ommx::v1::*` directly - refactor when encountered
- Prioritize idiomatic Rust structs for new features and core logic

## Current Migration Status (2025/1/9)

### COMPLETED ✅
- `Linear.raw` → `_ommx_rust.Linear` (完了)
- Rust SDK側に新しいメソッドを実装:
  - `Quadratic::linear_terms()`, `constant_term()`, `quadratic_terms()`, `from_coo()` 実装
  - `Polynomial::terms()`, `from_terms()` 実装
  - `Function::terms()` 実装
- Python側 `message.rs` を新しいRust SDK APIを使用するように部分的に修正

### CURRENT ISSUES 🔧
1. **コンパイルエラー**: Python側の `message.rs` でFnvHashMap型不整合
   - `std::collections::HashMap` vs `fnv::FnvHashMap` の型ミスマッチ
   - fnvクレートがPython側で利用できない

2. **アーキテクチャ設計**: 
   - Rust SDK側に機能を実装し、Python側から呼び出す構造に変更中
   - まだ移行途中で一部に古い直接実装が残存

3. **型の一貫性**:
   - 常に非ゼロの値は `Coefficient` 型
   - ゼロになりうる値は `f64` 型（例: `constant_term()`）

### NEXT STEPS 📋
1. FnvHashMap問題の解決（型変換もしくは代替手法）
2. 残りのPython側message.rs修正完了
3. Python側v1/__init__.pyの修正（新しいRust API使用）
4. `task python:ommx:pyright` を通す
5. テスト修復

### ARCHITECTURE CHANGES 📐
**新方針**: 
- 主要な機能をRust SDK (`rust/ommx/src/`) に実装
- Python側 (`python/ommx/src/message.rs`) はRust SDKのラッパーとして機能
- 型安全性とパフォーマンスの向上