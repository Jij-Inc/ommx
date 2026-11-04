"""Shared pytest options for the SDK, solver adapters, and bridge fixture."""

import sys

import pytest


def pytest_addoption(parser):
    parser.addoption(
        "--require-gil-disabled",
        action="store_true",
        help="Fail if the GIL is enabled before collection or by the end of this session.",
    )


def check_gil(session, phase):
    if (
        session.config.getoption("--require-gil-disabled")
        and getattr(sys, "_is_gil_enabled", lambda: True)()
    ):
        pytest.exit(f"GIL enabled {phase} (--require-gil-disabled)", returncode=1)


def pytest_sessionstart(session):
    check_gil(session, "before test collection")


def pytest_sessionfinish(session):
    check_gil(session, "at the end of the pytest session")
