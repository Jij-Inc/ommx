"""Sphinx configuration for OMMX Japanese documentation."""

from pathlib import Path

here = Path(__file__).parent
exec(open(here.parent / "conf_base.py").read())

language = "ja"
