"""Sphinx configuration for OMMX Japanese documentation."""

from pathlib import Path

# -- Load shared configuration -----------------------------------------------

here = Path(__file__).parent
docs_root = here.parent
exec(open(docs_root / "conf_base.py").read())

# -- Project information -----------------------------------------------------

project = "OMMX"
copyright = "2024, Jij Inc."
author = "Jij Inc."
language = "ja"
