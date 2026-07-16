# `ommx-pyo3-bridge`

`ommx-pyo3-bridge` converts values from the Rust `ommx` crate into the
canonical classes provided by the installed Python `ommx` package. It is for
independently built PyO3 extension modules that cannot safely pass Rust values
or private PyO3 wrapper types across a shared-library boundary.

Use the bridge wrapper as the direct return type of a PyO3 function:

```rust,no_run
use ommx_pyo3_bridge::PyFunction;
use pyo3::prelude::*;

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn objective() -> PyFunction {
    ommx::Function::default().into()
}
```

At runtime this return value is an `ommx.Function`. `pyo3-stub-gen` also emits
`ommx.Function` in the generated stub. The crate provides the same pattern for
detached constraints, detached decision variables, and instances through
`PyConstraint`, `PyDecisionVariable`, and `PyInstance`.

The consuming extension and the bridge must resolve a compatible PyO3 version.
This release accepts PyO3 0.27.2 through the 0.29 release line and uses
`pyo3-stub-gen` 0.23.

## Transfer boundary

The bridge serializes each value to protobuf bytes, passes those bytes through
the Python/PyO3 ABI, and asks the installed Python OMMX package to reconstruct
its canonical class. These payloads are an ephemeral implementation detail of
the bridge, not new persistence roots for component values.

The installed Python SDK owns the binding-private reconstruction endpoints
because it owns the canonical Python classes. These endpoints are production
bridge capabilities, not user-facing component serialization APIs.

The initial bridge has no payload version, version negotiation, or cross-version
compatibility guarantee. Rust and Python OMMX releases remain independently
versioned; callers must install a Python OMMX release that provides the bridge
endpoints expected by this crate. A missing endpoint produces an `ImportError`
with the required capability name.
