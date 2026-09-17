# Rust SDK v3 sender against Python SDK 2.9

This suite needs two independently built wheels: this branch's Python SDK 2.9
and `rust/ommx-pyo3-bridge/fixture` from Rust SDK v3. It lives outside
`python/ommx-tests` because the normal SDK test, type-check, and benchmark
environments do not install the sender fixture.

The Python workflow's `bridge-v1` job builds the sender from merged #1225
(`ee9c8d5cd9f8c9a1e97fd9298fc5c8515d50192e`), installs it alongside the SDK wheel
produced by `build-wheel`, and runs both Pyright and pytest on this directory.
The fixture import is mandatory: a missing or broken sender fails this job.

To run the same checks locally, put the SDK wheel in `wheels/` and the sender
wheel in `bridge-wheels/`, built for the selected interpreter, then run:

```sh
uv venv --python 3.12 .venv-bridge
uv pip install --python .venv-bridge/bin/python wheels/*.whl bridge-wheels/*.whl pytest pyright
uv run --no-project --python .venv-bridge/bin/python pyright --pythonpath .venv-bridge/bin/python python/bridge-tests
uv run --no-project --python .venv-bridge/bin/python pytest -v python/bridge-tests
```

The receiver advertises its actual V1-only contract; these tests do not mock
negotiation. The same sender wheel can be installed in a Python SDK v3 environment.
