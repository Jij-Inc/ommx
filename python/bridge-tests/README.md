# Rust SDK v3 sender against Python SDK 2.9

This suite needs two independently built wheels: this branch's Python SDK 2.9
and `rust/ommx-pyo3-bridge/fixture` from Rust SDK v3. It lives outside
`python/ommx-tests` because the normal SDK test, type-check, and benchmark
environments do not install the sender fixture.

The Python workflow's `bridge-v1` job takes the fixture source from
`rust-3.0.0-beta.6` (`32e90cfa53d3c589316d8acb71a368a402c94bf1`). Before building,
it replaces the fixture's `ommx` and `ommx-pyo3-bridge` path dependencies with
exact `=3.0.0-beta.6` dependencies from crates.io. `cargo add` updates the
checkout's lockfile, and the wheel build uses `--locked`. This exercises the
published sender crates as a downstream consumer would use them.

The job installs the sender alongside the SDK wheel produced by `build-wheel`
and runs both Pyright and pytest on this directory.
The fixture import is mandatory: a missing or broken sender fails this job.
The Python SDK receiver uses `ommx-pyo3-bridge = "=3.0.0-beta.6"` from crates.io
with default features disabled, preserving its existing Rust SDK dependencies.

To run the same checks locally, put the SDK wheel in `wheels/` and check out the
pinned release in `.bridge-sender`, then run:

```sh
uv venv --python 3.12 .venv-bridge
cargo add --manifest-path .bridge-sender/rust/ommx-pyo3-bridge/fixture/Cargo.toml 'ommx@=3.0.0-beta.6' --no-default-features
cargo add --manifest-path .bridge-sender/rust/ommx-pyo3-bridge/fixture/Cargo.toml 'ommx-pyo3-bridge@=3.0.0-beta.6'
uvx --from 'maturin>=1.12.2,<2' maturin build --locked --out bridge-wheels --interpreter .venv-bridge/bin/python -m .bridge-sender/rust/ommx-pyo3-bridge/fixture/Cargo.toml
uv pip install --python .venv-bridge/bin/python wheels/*.whl bridge-wheels/*.whl pytest pyright
uv run --no-project --python .venv-bridge/bin/python pyright --pythonpath .venv-bridge/bin/python python/bridge-tests
uv run --no-project --python .venv-bridge/bin/python pytest -v python/bridge-tests
```

The receiver advertises its actual V1-only contract; these tests do not mock
negotiation. The same sender wheel can be installed in a Python SDK v3 environment.
