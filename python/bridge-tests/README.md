# Rust SDK v3 model producer against Python SDK 2.9

`modeling/` is an independent PyO3 package that plays the role of a model producer
such as JijModeling. It belongs to the root Cargo and uv workspaces and shares
their committed lockfiles. Its Rust SDK and bridge dependencies are explicitly the
published `=3.0.0-beta.6` crates from crates.io. Its Python dependency is
`ommx==2.9.0`.

The fixture sets `tool.uv.package = false`, so ordinary workspace syncs do not
build or install it. The wheel test explicitly adds the built fixture wheel,
with a Python-version marker matching the interpreter used to build it.

The package takes costs and bounds, constructs an `ommx::Instance` in Rust v3,
and transfers an explicit V1 protobuf message. OneHot and SOS1 models retain
ordinary constraints and selectors, with hints generated from SDK-validated
promotion plans. Exact zero-bound SOS1 members stay in the original variable
set but are excluded before constructing selectors and link rows. With fewer
than two remaining members, the SOS1 formulation needs no rows or hints.

The Python workflow's `build-wheel` job builds this producer and Python SDK 2.9
once each as separate wheels, selects them with the existing Python test setup
script, and runs `uv sync` in the shared Python 3.12 `.venv`. It runs the
integration suite once and checks
the generated producer stub, concrete Python return types, IDs, labels, hints,
objective values, and projected feasibility over small enumerated domains.
An absolute-value objective also exercises error propagation when the v2
parser rejects a function supported by Rust v3. These checks do not require
native and Big-M constraint violations or finite-ATol feasibility to agree.
The SDK wheel is then uploaded for the existing Python-version test matrix;
the producer suite is not repeated in that matrix.

The producer sends only ProtobufV1. This branch tests only Python SDK 2.9.0.
No source checkout of Rust SDK v3 or CI-time Cargo manifest rewriting is needed.
The seven receiver factories continue to have their own SDK unit tests in
`python/ommx-tests/tests/test_bridge_receiver.py`.

To reproduce the wheel test locally, put the Python SDK 2.9 wheel in `wheels/`
and run from the repository root. As in CI, the setup script updates the root
project's sources to use the built wheels in the shared `.venv`:

```sh
export UV_PYTHON=3.12
uv venv
uvx --from 'maturin>=1.12.2,<2' maturin build --locked --out wheels --interpreter .venv/bin/python -m python/bridge-tests/modeling/Cargo.toml
uv run ci-scripts/setup-python-test-project.py 3.12 --bridge-wheel wheels/bridge_test_modeling-*.whl
task python:sync
uv run pyright python/bridge-tests
uv run pytest -v python/bridge-tests
```

The tests live outside normal SDK and benchmark discovery. The wheel-building
job requires the producer package; a missing or broken installation fails the job.
