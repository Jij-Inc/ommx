"""Shared Sphinx configuration for OMMX documentation (en/ja)."""

import os
import sys
from pathlib import Path

import tomlkit

# -- Path setup --------------------------------------------------------------

# When exec'd from docs/en/conf.py or docs/ja/conf.py, __file__ points to the caller
# Path(__file__).parent = docs/en/ or docs/ja/, so .parent gives docs/
_docs_root = Path(__file__).parent.parent
python_root = _docs_root.parent / "python"

# Add the API docs directory to Python path for pyo3_stub_gen_ext
sys.path.insert(0, str(_docs_root / "api"))

# -- Project information -----------------------------------------------------

project = "OMMX"
copyright = "2024, Jij Inc."
author = "Jij Inc."

pyproject_toml = tomlkit.loads((python_root / "ommx" / "pyproject.toml").read_text())
version = str(pyproject_toml["project"]["version"])  # type: ignore
release = version

# -- General configuration ---------------------------------------------------

extensions = [
    "myst_nb",
    "sphinx.ext.autodoc",
    "sphinx.ext.intersphinx",
    "sphinx_fontawesome",
    "sphinxcontrib.katex",
    "autoapi.extension",
    "pyo3_stub_gen_ext",
]

source_suffix = {
    ".rst": "restructuredtext",
}

templates_path = []
exclude_patterns = ["_build", "Thumbs.db", ".DS_Store"]

# -- MyST / myst-nb settings ------------------------------------------------

myst_enable_extensions = ["dollarmath"]
myst_update_mathjax = False

# Allow overriding execution mode via environment variable (e.g. OMMX_NB_EXECUTION=force)
nb_execution_mode = os.environ.get("OMMX_NB_EXECUTION", "off")
nb_execution_timeout = 300
nb_execution_excludepatterns = ["release_note/ommx-1.*.md"]

# -- Options for HTML output -------------------------------------------------

html_theme = "sphinx_book_theme"
html_show_sourcelink = False
html_static_path = []
html_favicon = "../logo.png"
html_logo = "../logo.png"

# Display class names only, without module prefix
add_module_names = False
python_use_unqualified_type_names = True

# -- AutoAPI settings --------------------------------------------------------

autoapi_dirs = [
    python_root / "ommx",
    python_root / "ommx-python-mip-adapter",
    python_root / "ommx-pyscipopt-adapter",
    python_root / "ommx-highs-adapter",
    python_root / "ommx-openjij-adapter",
]
autoapi_options = [
    "members",
    "inherited-members",
    "undoc-members",
    "show-module-summary",
]
autoapi_member_order = "groupwise"
autoapi_file_patterns = ["*.pyi", "*.py"]
autoapi_ignore = [
    "**/tests/**",
    "**/conftest.py",
    "**/ommx/v1/**",
    "**/ommx/artifact/**",
    "**/ommx/_ommx_rust/**",
    "**/pywasmcross/**",
]
autoapi_add_toctree_entry = False

# -- Intersphinx Configuration -----------------------------------------------

intersphinx_mapping = {
    "python": ("https://docs.python.org/3", None),
    "numpy": ("https://numpy.org/doc/stable", None),
    "pandas": ("https://pandas.pydata.org/docs", None),
}
