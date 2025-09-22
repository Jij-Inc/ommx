# OMMX GitHub Copilot Instructions

**FOLLOW THESE INSTRUCTIONS FIRST.** Only fallback to additional search and context gathering if the information in these instructions is incomplete or found to be in error.

OMMX (Open Mathematical prograMming eXchange) is a multi-language optimization ecosystem with Rust core, Python bindings, and solver adapters. It uses Protocol Buffers for data interchange and maturin for Python-Rust integration.

## Working Effectively

### Prerequisites Installation
Install these tools in order:
```bash
# Install Taskfile (required for all operations)
sudo sh -c "$(curl --location https://taskfile.dev/install.sh)" -- -d -b /usr/local/bin

# Install Protocol Buffers tools
sudo apt-get update && sudo apt-get install -y protobuf-compiler
curl -sSL https://github.com/bufbuild/buf/releases/latest/download/buf-Linux-x86_64 -o /usr/local/bin/buf
sudo chmod +x /usr/local/bin/buf

# Install Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

# Install uv for Python management
pip install uv
```

### Bootstrap Development Environment
**NEVER CANCEL THESE COMMANDS** - Initial setup can take 15-45 minutes:
```bash
# Set Python version (required)
echo "cpython@3.9" > .python-version

# Generate Protocol Buffer code - takes 5-10 minutes
task proto

# Setup Python environment - takes 10-30 minutes, NEVER CANCEL
task python:sync

# Verify setup works
task -l
```

### Build and Test Commands
**TIMING EXPECTATIONS - NEVER CANCEL:**
- `task proto` - 30 seconds: Generate all protobuf code (Rust=12s, Python may fail offline)
- `task python:sync` - 3-10 minutes: Install dependencies and build Rust extensions (measured: 2m41s)
- `task python:test` - 15-45 minutes: Run all Python tests including adapters
- `task rust:test` - 2-5 minutes: Run all Rust tests (measured: 1m7s, 294 tests)
- `task rust:check` - 1-2 minutes: Check Rust compilation (measured: 52s)
- `task format` - 2-5 minutes: Format all code

**Essential Commands:**
```bash
# Build everything from scratch - 5-15 minutes total, NEVER CANCEL
task proto && task python:sync

# Run all tests - 20-50 minutes, NEVER CANCEL
task python:test
task rust:test

# Quick validation (after making changes) - 3-7 minutes
task format && task python:lint && task rust:clippy
```

## Project Structure

### Key Directories
- `proto/` - Protocol Buffer message definitions (core data structures)
- `rust/ommx/` - Core Rust SDK implementation
- `python/ommx/` - Main Python package with Rust bindings
- `python/ommx-*-adapter/` - Solver-specific adapters
- `python/ommx-tests/` - All Python tests
- `docs/` - Jupyter Books documentation (English/Japanese)

### Critical Files
- `Taskfile.yml` - Main task runner configuration
- `CLAUDE.md` - Detailed development guidelines (READ THIS)
- `DEVELOPMENT.md` - Developer documentation
- `.github/workflows/` - CI/CD pipelines
- `python/ommx/pyproject.toml` - Python package configuration with maturin

## Validation Scenarios

**ALWAYS test these scenarios after making changes:**

### 1. Basic SDK Functionality
```bash
# Test core Python SDK - must work after python:sync
cd python/ommx-tests
uv run python -c "
from ommx.v1 import DecisionVariable, Instance, Solution
x = DecisionVariable.binary(0)
constraint = x == 1
print('Basic SDK import and usage: OK')
"
```

### 2. Rust Core Validation
```bash
# Test Rust SDK (fast validation) - 1-2 minutes
task rust:test
# Expected: 294+ tests pass, compilation succeeds in ~1-2 minutes
```

### 3. Adapter Functionality
```bash
# Test a solver adapter works
task python:ommx-highs-adapter:test
# Expected: All tests pass, including integration tests with actual solving
```

### 3. Documentation Build
```bash
# Test documentation builds - takes 10-20 minutes, NEVER CANCEL
task book_en
# Expected: Jupyter Book builds successfully, opens in browser
```

### 4. Full Integration Test
```bash
# Run comprehensive test suite - 45-90 minutes, NEVER CANCEL
task python:test
# Expected: All packages pass linting, type checking, and unit tests
```

## Development Rules

### Critical Prohibitions
- **NEVER use `cd` command** - Always work from repository root
- **NEVER import from `_ommx_rust`** - Use `ommx.v1` unified API only
- **NEVER modify v1_ext directory** - Contains deprecated implementations
- **NEVER claim performance improvements without benchmarks**

### Quality Requirements
- **ALWAYS run `task format` before committing**
- **ALWAYS ensure `task python:test` passes completely**
- Add tests only to `python/ommx-tests/tests/`
- Use `ommx.v1` imports in all new code

### Timing and Cancellation Rules
- **NEVER CANCEL build or test commands**
- Set timeouts to at least 60 minutes for builds, 90 minutes for full test suites
- Initial setup can take 45+ minutes - this is normal
- If a command appears stuck, wait at least 60 minutes before investigating

## Common Tasks

### Working with Adapters
```bash
# Create/modify solver adapters
# Good pattern:
from ommx.v1 import Instance, DecisionVariable, Function, Solution

# Bad pattern (DO NOT USE):
from ommx._ommx_rust import Function
from ommx.v1.solution_pb2 import Optimality
```

### Testing Changes
```bash
# Test specific components
task python:ommx:test              # Core Python SDK only
task python:ommx-highs-adapter:test # Specific adapter
task rust:test                     # Rust SDK only

# Pre-commit validation
task format && task python:lint && task rust:clippy
```

### Protocol Buffer Changes
```bash
# After modifying .proto files
task proto                         # Regenerate all bindings
task python:sync                   # Rebuild Python extensions
```

### Documentation
```bash
# Build API reference - 10-15 minutes
task api_reference

# Build books - 15-25 minutes each
task book_en    # English
task book_ja    # Japanese
```

## Troubleshooting

### Build Failures
1. **Network connectivity issues**: Most common cause of build failures
   - Check internet access: `ping crates.io`
   - Corporate firewalls may block crates.io, pypi.org, buf.build
   - Retry commands - network issues often resolve themselves
2. Ensure all prerequisites are installed in correct order
3. Clean and retry: `rm -rf target/ python/ommx/.venv/` then restart setup

### Test Failures
1. Run `task python:sync` to ensure latest build
2. Check specific test with `task python:ommx:pytest` for detailed output
3. Verify solver adapters have required external solvers installed

### Performance Issues
- Large compile times are normal for Rust-Python integration
- Initial Python sync rebuilds Rust extensions completely
- Subsequent builds use cargo caching (much faster)

### Network Environment Issues
- Sandboxed environments may have limited network access
- `task proto` may fail on `buf generate` but `cargo run --bin protogen` often works
- Download failures from crates.io or PyPI indicate network restrictions

## Expected Timings Summary
- Prerequisites installation: 5-10 minutes
- Initial setup (proto + python:sync): 3-12 minutes (measured: ~3.5 min total)
- Rust tests only: 2-5 minutes (measured: 1m7s, 294 tests)
- Python tests (all adapters): 15-45 minutes
- Documentation builds: 15-25 minutes per book
- Format/lint checks: 2-5 minutes
- Incremental builds after changes: 1-5 minutes

**REMEMBER: NEVER CANCEL long-running operations. Even quick operations can take longer in CI environments.**