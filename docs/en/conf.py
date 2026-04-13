"""Sphinx configuration for OMMX English documentation."""

from pathlib import Path

here = Path(__file__).parent
exec(open(here.parent / "conf_base.py").read())

language = "en"
