# Rust SDK v3 model producer against Python SDK 2.9

`modeling/` is an independent PyO3 package that plays the role of a model producer
such as JijModeling. It is a member of the root Cargo workspace and shares its
committed lockfile. Its Rust SDK and bridge dependencies are explicitly the
published `=3.0.0-beta.6` crates from crates.io. Its Python dependency is
`ommx==2.9.0`.

The package takes costs and bounds, constructs an `ommx::Instance` in Rust v3,
and transfers an explicit V1 protobuf message. OneHot and SOS1 models retain
ordinary constraints and selectors, with hints generated from SDK-validated
promotion plans. Exact zero-bound SOS1 members stay in the original variable
set but are excluded before constructing selectors and link rows. With fewer
than two remaining members, the SOS1 formulation needs no rows or hints.

The Python workflow's `build-wheel` job builds this producer and Python SDK 2.9
once each as separate wheels, installs both into an isolated Python 3.12
environment, and runs the integration suite once. It checks
the generated producer stub, concrete Python return types, IDs, labels, hints,
objective values, and projected feasibility over small enumerated domains.
An absolute-value objective also exercises error propagation when the v2
parser rejects a function supported by Rust v3. These checks do not require
native and Big-M constraint violations or finite-ATol feasibility to agree.
The SDK wheel is then uploaded for the existing Python-version test matrix;
the producer suite is not repeated in that matrix.

The producer sends only ProtobufV1. This branch tests only Python SDK 2.9.0.
No source checkout of Rust SDK v3 or CI-time manifest rewriting is needed.
The seven receiver factories continue to have their own SDK unit tests in
`python/ommx-tests/tests/test_bridge_receiver.py`.

To run locally from the repository root, put the Python SDK 2.9 wheel in
`wheels/`, then run:

```sh
uv venv --python 3.12 .venv-bridge
PYO3_PYTHON="$PWD/.venv-bridge/bin/python" cargo run --locked -p bridge-test-modeling --bin stub_gen
uvx --from 'ruff==0.9.10' ruff format python/bridge-tests/modeling/bridge_test_modeling.pyi
uvx --from 'maturin>=1.12.2,<2' maturin build --locked --out bridge-wheels --interpreter .venv-bridge/bin/python -m python/bridge-tests/modeling/Cargo.toml
uv pip install --python .venv-bridge/bin/python wheels/*.whl bridge-wheels/*.whl pytest pyright
uv run --no-project --python .venv-bridge/bin/python pyright --pythonpath .venv-bridge/bin/python python/bridge-tests
uv run --no-project --python .venv-bridge/bin/python pytest -v python/bridge-tests
```

The tests live outside normal SDK and benchmark discovery. The wheel-building
job requires the producer package; a missing or broken installation fails the job.
