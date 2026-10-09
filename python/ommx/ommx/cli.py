"""Console entry point for the shared Rust CLI."""

import sys


def main() -> int:
    from ommx._ommx_rust import _run_cli

    return _run_cli(sys.argv)


if __name__ == "__main__":
    sys.exit(main())
