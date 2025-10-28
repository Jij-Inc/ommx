"""
Global pytest configuration for ommx-tests.

This module sets up fixtures that are shared across all test modules.
"""

import tempfile
import shutil
from pathlib import Path

import pytest

from ommx.artifact import set_local_registry_root


@pytest.fixture(scope="session", autouse=True)
def setup_local_registry():
    """
    Set up a temporary local registry for all tests.

    This fixture is automatically used for all tests (autouse=True) and runs once
    per test session (scope="session"). The local registry root can only be set
    once per process due to OnceLock constraints in the Rust implementation.
    """
    temp_dir = tempfile.mkdtemp(prefix="ommx-test-registry-")
    try:
        set_local_registry_root(temp_dir)
        yield Path(temp_dir)
    finally:
        shutil.rmtree(temp_dir, ignore_errors=True)
