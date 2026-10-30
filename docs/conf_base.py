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

# Add the API docs directory to Python path for the generated renderer and the
# OMMX-specific wrapper loaded immediately after it.
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
    "sphinx.ext.doctest",
    "sphinx.ext.intersphinx",
    "sphinx_fontawesome",
    "sphinxcontrib.katex",
    "autoapi.extension",
    "pyo3_stub_gen_ext",
    "ommx_pyo3_stub_gen_ext",
]

source_suffix = {
    ".rst": "restructuredtext",
}

templates_path = []
exclude_patterns = [
    "_build",
    "Thumbs.db",
    ".DS_Store",
    # autoapi generates an index page for the top-level `ommx` package,
    # but we list each submodule explicitly in api/index.rst instead.
    "autoapi/ommx/index.rst",
]

# The `ommx`, `ommx._ommx_rust`, `ommx.artifact`, and `ommx.experiment`
# modules are documented via pyo3_stub_gen_ext (not autoapi), so autoapi cannot
# resolve them when it processes the autoapi-covered submodules. Silence those
# spurious warnings.
suppress_warnings = ["autoapi.python_import_resolution"]

# Both language builds use -W, so unresolved cross-references must be warnings.
nitpicky = True

# Exact external types without published Python-domain inventory entries.
# Keep OMMX references checked; do not add package-wide ignore patterns.
nitpick_ignore = [
    # HiGHS uses non-Sphinx documentation for its Python interface.
    ("py:class", "highspy.Highs"),
    # Python-MIP's inventory covers Model/LinExpr but not its Solver base class.
    ("py:class", "mip.Solver"),
    # OpenJij's published Book inventory does not contain the Response API.
    ("py:class", "openjij.Response"),
    # OpenTelemetry documents its SDK, but not these generated protobuf classes.
    (
        "py:class",
        "opentelemetry.proto.collector.trace.v1.trace_service_pb2.ExportTraceServiceRequest",
    ),
    ("py:class", "opentelemetry.proto.trace.v1.trace_pb2.Span"),
]

# -- MyST / myst-nb settings ------------------------------------------------

myst_enable_extensions = ["dollarmath"]
myst_update_mathjax = False

# Allow overriding execution mode via environment variable (e.g. OMMX_NB_EXECUTION=force)
nb_execution_mode = os.environ.get("OMMX_NB_EXECUTION", "off")
nb_execution_timeout = 300
nb_execution_excludepatterns = [
    "release_note/ommx-1.*.md",
    "release_note/ommx-2.*.md",
]
nb_execution_raise_on_error = True

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
    "imported-members",
    "show-module-summary",
]
autoapi_member_order = "groupwise"
autoapi_file_patterns = ["*.pyi", "*.py"]
autoapi_ignore = [
    "**/tests/**",
    "**/conftest.py",
    "**/ommx/v1/**",
    "**/ommx/artifact/**",
    "**/ommx/experiment/__init__.py",
    "**/ommx/_ommx_rust/**",
    "**/.pyodide_build/**",
    "**/node_modules/pyodide/**",
    "**/pywasmcross/**",
]
autoapi_add_toctree_entry = False

# -- Intersphinx Configuration -----------------------------------------------

intersphinx_mapping = {
    "python": ("https://docs.python.org/3", None),
    "numpy": ("https://numpy.org/doc/stable", None),
    "pandas": ("https://pandas.pydata.org/docs", None),
    "opentelemetry": ("https://opentelemetry-python.readthedocs.io/en/latest", None),
    "ipython": ("https://ipython.readthedocs.io/en/stable", None),
    "pyscipopt": ("https://pyscipopt.readthedocs.io/en/stable", None),
    "mip": ("https://docs.python-mip.com/en/latest", None),
}
