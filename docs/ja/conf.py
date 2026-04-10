"""Sphinx configuration for OMMX Japanese documentation."""

import os
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

# -- Intersphinx: link to English API Reference -------------------------------

# On RTD, use the canonical URL to link to the same version (PR build, latest, etc.)
# Locally, fall back to the production URL
_rtd_canonical = os.environ.get("READTHEDOCS_CANONICAL_URL", "")
if _rtd_canonical:
    _en_url = _rtd_canonical.replace("/ja/", "/en/")
else:
    _en_url = "https://jij-inc-ommx.readthedocs-hosted.com/en/latest/"

intersphinx_mapping["ommx-en"] = (_en_url, None)  # noqa: F821
