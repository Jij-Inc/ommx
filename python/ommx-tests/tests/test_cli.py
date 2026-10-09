"""Exercise the installed console script and the shared in-process CLI."""

import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys

import pytest

from ommx.artifact import gc, list_artifacts
from ommx.cli import main


@pytest.fixture
def cli(tmp_path):
    registry = tmp_path / "registry"
    executable = Path(sys.executable).with_name(
        "ommx.exe" if sys.platform == "win32" else "ommx"
    )

    def run(*args):
        return subprocess.run(
            [str(executable), *map(str, args)],
            capture_output=True,
            text=True,
            check=False,
            env={
                **os.environ,
                "OMMX_LOCAL_REGISTRY_ROOT": str(registry),
                "NO_COLOR": "1",
            },
        )

    return run, registry


@pytest.mark.parametrize("args", [("--help",), ("inspect", "--help")])
def test_console_help_does_not_open_registry(cli, args):
    run, registry = cli
    result = run(*args)
    assert result.returncode == 0
    assert "Usage:" in result.stdout
    assert result.stderr == ""
    assert not registry.exists()


def test_console_argument_error_is_stderr_and_exit_two(cli):
    run, registry = cli
    result = run("inspect")
    assert result.returncode == 2
    assert result.stdout == ""
    assert "required arguments" in result.stderr
    assert not registry.exists()


def test_console_execution_error_is_stderr_and_exit_one(cli, tmp_path):
    run, _ = cli
    result = run("import", tmp_path / "missing.ommx")
    assert result.returncode == 1
    assert result.stdout == ""
    assert "Failed to stat" in result.stderr
    assert "Traceback" not in result.stderr


@pytest.mark.parametrize(
    ("args", "stream"),
    [
        (("version",), "stdout"),
        (("--help",), "stdout"),
        (("inspect",), "stderr"),
        (("import", "missing.ommx"), "stderr"),
        (("load", "missing.ommx"), "stderr"),
    ],
)
def test_main_returns_one_when_terminal_write_fails(args, stream, tmp_path):
    read_fd, write_fd = os.pipe()
    os.close(read_fd)
    try:
        result = subprocess.run(
            [
                sys.executable,
                "-c",
                "from ommx.cli import main; import sys; "
                "sys.argv[0] = 'ommx'; "
                "code = main(); assert code == 1, code",
                *args,
            ],
            stdout=write_fd if stream == "stdout" else subprocess.PIPE,
            stderr=write_fd if stream == "stderr" else subprocess.PIPE,
            text=True,
            check=False,
            timeout=30,
            env={
                **os.environ,
                "OMMX_LOCAL_REGISTRY_ROOT": str(tmp_path / "registry"),
                "NO_COLOR": "1",
            },
        )
    finally:
        os.close(write_fd)
    # The caller reaches its assertion and exits normally after the CLI returns.
    assert result.returncode == 0, result.stdout or result.stderr
    output = result.stdout or result.stderr or ""
    assert "Traceback" not in output
    assert "panicked" not in output


def test_console_archive_registry_round_trip(cli, tmp_path):
    run, registry = cli
    archive = Path(__file__).resolve().parents[3] / "data/random_lp_instance.ommx"
    inspected = run("inspect", archive)
    assert inspected.returncode == 0, inspected.stderr
    manifest = json.loads(inspected.stdout)
    assert not registry.exists()

    imported = run("import", archive)
    assert imported.returncode == 0, imported.stderr
    listed = run("list")
    assert listed.returncode == 0, listed.stderr
    [image_name] = listed.stdout.splitlines()
    assert image_name.startswith("ghcr.io/jij-inc/ommx/random_lp_instance:")

    exported_path = tmp_path / "exported-例.ommx"
    exported = run("export", image_name, exported_path)
    assert exported.returncode == 0, exported.stderr
    exported_manifest = run("inspect", exported_path)
    assert exported_manifest.returncode == 0, exported_manifest.stderr
    assert json.loads(exported_manifest.stdout) == manifest
    assert exported_manifest.stdout == inspected.stdout

    removed = run("rm", image_name)
    assert removed.returncode == 0, removed.stderr
    digest = re.search(r"sha256:[0-9a-f]{64}", removed.stdout)
    assert digest is not None
    assert run("list").stdout == ""
    restored = run("restore-ref", image_name, digest.group())
    assert restored.returncode == 0, restored.stderr
    assert run("list").stdout.splitlines() == [image_name]


def test_console_sigint_terminates_without_a_python_traceback():
    result = subprocess.run(
        [
            sys.executable,
            "-c",
            "import signal; import ommx.cli as cli; "
            "cli.main = lambda: signal.raise_signal(signal.SIGINT); "
            "cli._console_main(); print('continued after SIGINT')",
        ],
        capture_output=True,
        text=True,
        check=False,
        timeout=30,
    )
    assert result.returncode != 0
    assert result.stdout == ""
    assert "Traceback" not in result.stderr
    assert "KeyboardInterrupt" not in result.stderr


def test_main_returns_without_exiting_or_replacing_sdk_tracing(
    monkeypatch, tmp_path, capfd
):
    # Initialize the SDK's tracing bridge before the CLI is called.
    sigint_handler = signal.getsignal(signal.SIGINT)
    registry = tmp_path / "caller-registry"
    gc(root=registry)
    for _ in range(2):
        monkeypatch.setattr(sys, "argv", ["ommx", "version"])
        assert main() == 0
    assert "Version" in capfd.readouterr().out

    monkeypatch.setattr(sys, "argv", ["ommx", "inspect"])
    assert main() == 2
    assert "required arguments" in capfd.readouterr().err
    assert list_artifacts(root=registry) == []
    assert signal.getsignal(signal.SIGINT) == sigint_handler
