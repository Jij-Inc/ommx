"""Shared Sphinx configuration for OMMX documentation (en/ja)."""

import os

# -- General configuration ---------------------------------------------------

extensions = [
    "myst_nb",
    "sphinx_rtd_theme",
    "sphinx.ext.intersphinx",
]

source_suffix = {
    ".rst": "restructuredtext",
}

templates_path = []
exclude_patterns = ["_build", "Thumbs.db", ".DS_Store"]

# -- MyST / myst-nb settings ------------------------------------------------

myst_enable_extensions = ["dollarmath"]

# Allow overriding execution mode via environment variable (e.g. OMMX_NB_EXECUTION=force)
nb_execution_mode = os.environ.get("OMMX_NB_EXECUTION", "off")
nb_execution_timeout = 300
nb_execution_excludepatterns = ["release_note/ommx-1.*.md"]

# -- Options for HTML output -------------------------------------------------

html_theme = "sphinx_rtd_theme"
html_show_sourcelink = False
html_static_path = []
html_favicon = "../logo.png"
html_logo = "../logo.png"

# -- Intersphinx Configuration -----------------------------------------------

intersphinx_mapping = {
    "python": ("https://docs.python.org/3", None),
    "numpy": ("https://numpy.org/doc/stable", None),
    "pandas": ("https://pandas.pydata.org/docs", None),
}
