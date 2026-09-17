# Rust SDK v3 sender against Python SDK 2.9

Build `rust/ommx-pyo3-bridge/fixture` from the Rust SDK v3 branch with maturin,
install that wheel in this branch's Python environment without replacing OMMX,
and run `uv run --no-sync pytest python/ommx-tests/bridge`.
The receiver advertises its actual V1-only contract; these tests do not mock
negotiation. The same sender wheel can be installed in a Python SDK v3 environment.
