"""Run Rust commands with the current Python environment available to embedded PyO3.

Use through uv, e.g. uv run --no-sync python ci-scripts/run-rust-with-python.py
cargo test -p _ommx_rust --lib pandas::schema::tests
"""

import os
import subprocess
import sys


env = os.environ.copy()
env["PYO3_PYTHON"] = sys.executable
# Embedded CPython does not automatically activate the invoking virtualenv.
env["PYTHONPATH"] = os.pathsep.join(path for path in sys.path if path)
subprocess.run(sys.argv[1:], env=env, check=True)
