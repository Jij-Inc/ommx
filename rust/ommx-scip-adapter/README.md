# OMMX SCIP Adapter

Rust adapter for solving OMMX optimization problems using the [SCIP](https://www.scipopt.org/) solver through C FFI.

## Overview

This crate provides a direct integration between OMMX (Rust) and SCIP (C) without going through Python. It uses:
- **bindgen** to automatically generate Rust bindings from SCIP's C API
- **Direct C FFI** for efficient communication with SCIP
- **Native Rust types** from the OMMX core library

## Prerequisites

### SCIP Installation

You must have SCIP installed on your system before building this crate.

#### Ubuntu/Debian

```bash
sudo apt-get install libscip-dev
```

#### macOS

```bash
brew install scip
```

#### From Source

```bash
git clone https://github.com/scipopt/scip.git
cd scip
mkdir build && cd build
cmake ..
make
sudo make install
```

#### Verify Installation

```bash
# Check if SCIP is available via pkg-config
pkg-config --modversion scip

# Check if headers are available
ls /usr/local/include/scip/scip.h  # or /usr/include/scip/scip.h
```

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
ommx = { version = "2.3" }
ommx-scip-adapter = { version = "2.3" }
```

Basic example:

```rust
use ommx::{Instance, DecisionVariable, Sense, linear, coeff};
use ommx_scip_adapter::ScipAdapter;
use maplit::btreemap;

// Create an OMMX instance
// Minimize: x1 + 2*x2
// Subject to: x1 + x2 <= 1
//             x1, x2 >= 0
let instance = Instance::new(
    Sense::Minimize,
    (coeff!(1.0) * linear!(1) + coeff!(2.0) * linear!(2)).into(),
    btreemap! {
        1.into() => DecisionVariable::continuous(1.into()),
        2.into() => DecisionVariable::continuous(2.into()),
    },
    btreemap! {
        1.into() => Constraint::less_than_or_equal_to_zero(
            1.into(),
            (linear!(1) + linear!(2) + coeff!(-1.0)).into()
        ),
    },
)?;

// Solve with SCIP
let solution = ScipAdapter::solve(&instance)?;

println!("Objective value: {}", solution.objective());
println!("Feasible: {}", solution.feasible());
```

## Features

### Currently Supported

- ✅ Linear objective functions
- ✅ Linear constraints (equality and inequality)
- ✅ Decision variable types: Binary, Integer, Continuous
- ✅ Optimization direction: Minimize, Maximize
- ✅ Solution decoding to OMMX format
- ✅ Optimality status detection

### Planned Support

- ⏳ Quadratic objective functions
- ⏳ Quadratic constraints
- ⏳ SOS1 constraints
- ⏳ SOS2 constraints
- ⏳ Indicator constraints
- ⏳ Custom SCIP parameters
- ⏳ Warm start solutions
- ⏳ Dual values extraction

## Architecture

```
┌─────────────────┐
│  OMMX Instance  │  (Rust)
│   (Rust SDK)    │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  ScipAdapter    │  (Rust)
│  - Variables    │
│  - Objective    │
│  - Constraints  │
└────────┬────────┘
         │ FFI (bindgen)
         ▼
┌─────────────────┐
│   SCIP C API    │  (C)
│   scip.h        │
│   (libscip.so)  │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   SCIP Solver   │  (C)
└─────────────────┘
```

## Comparison with Python Adapter

| Aspect | Python Adapter | Rust Adapter (this) |
|--------|---------------|---------------------|
| Language | Python (pyscipopt) | Rust (direct C FFI) |
| Performance | Python overhead | Native speed |
| Memory | Python GC + SCIP | Rust + SCIP only |
| Type Safety | Runtime | Compile-time |
| Distribution | Python package | Rust crate |
| Use Case | Rapid prototyping | Production systems |

## Development

### Building

```bash
# Build the crate
cargo build -p ommx-scip-adapter

# Run tests (requires SCIP)
cargo test -p ommx-scip-adapter

# Check without building
cargo check -p ommx-scip-adapter
```

### Testing

Tests require SCIP to be installed:

```bash
cargo test -p ommx-scip-adapter -- --nocapture
```

### Debugging

Enable logging to see SCIP adapter internals:

```rust
env_logger::init();
```

## Troubleshooting

### Build Errors

**Error: `'scip/scip.h' file not found`**

Solution: Install SCIP development headers (see Prerequisites above).

**Error: `library 'scip' not found`**

Solution: Ensure SCIP library is in your library path:
```bash
export LD_LIBRARY_PATH=/usr/local/lib:$LD_LIBRARY_PATH
```

**Error: `pkg-config for SCIP not found`**

This is a warning. The build will continue trying common paths. To fix:
```bash
# Make sure pkg-config is installed
sudo apt-get install pkg-config

# Or add SCIP's .pc file to PKG_CONFIG_PATH
export PKG_CONFIG_PATH=/usr/local/lib/pkgconfig:$PKG_CONFIG_PATH
```

## License

MIT OR Apache-2.0

## Contributing

Contributions are welcome! Please ensure:
1. All tests pass
2. Code is formatted with `cargo fmt`
3. No clippy warnings: `cargo clippy`
4. Add tests for new features

## References

- [SCIP Optimization Suite](https://www.scipopt.org/)
- [SCIP C API Documentation](https://scipopt.org/doc/html/modules.php)
- [OMMX Project](https://github.com/Jij-Inc/ommx)
