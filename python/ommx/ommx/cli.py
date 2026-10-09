"""Console entry point for the shared Rust CLI."""

import signal
import sys


def main() -> int:
    from ommx._ommx_rust import _run_cli

    return _run_cli(sys.argv)


def _console_main() -> int:
    # Rust execution can block without returning to Python's signal checks.
    # A console invocation should terminate immediately on Ctrl+C.
    signal.signal(signal.SIGINT, signal.SIG_DFL)
    return main()


if __name__ == "__main__":
    sys.exit(_console_main())
