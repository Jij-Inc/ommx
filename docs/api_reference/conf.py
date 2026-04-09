import sys
from pathlib import Path

import sphinx_rtd_theme
import tomlkit

# -- Path setup --------------------------------------------------------------

here = Path(__file__).parent
python_root = here.parent.parent / "python"  # ${REPO_ROOT}/python

# Add the API docs directory to Python path for pyo3_stub_gen_ext
sys.path.insert(0, str(here / "api"))

# -- Project information -----------------------------------------------------

project = "OMMX Python SDK"
copyright = "2024, Jij Inc."
author = "Jij Inc."

pyproject_toml = tomlkit.loads((python_root / "ommx" / "pyproject.toml").read_text())
version = str(pyproject_toml["project"]["version"])  # type: ignore
release = version

# -- General configuration ---------------------------------------------------

extensions = [
    "sphinx.ext.napoleon",
    "sphinx.ext.intersphinx",
    "sphinx_rtd_theme",
    "sphinx_fontawesome",
    "myst_parser",
    "pyo3_stub_gen_ext",
]
source_suffix = {
    ".rst": "restructuredtext",
    ".md": "markdown",
}

templates_path = ["_templates"]
language = "en"
exclude_patterns = ["_build", "Thumbs.db", ".DS_Store"]

# -- Options for HTML output -------------------------------------------------

html_theme = "sphinx_rtd_theme"
html_theme_path = [sphinx_rtd_theme.get_html_theme_path()]
html_show_sourcelink = False
html_static_path = []

# Display class names only, without module prefix
add_module_names = False
python_use_unqualified_type_names = True

# -- Napoleon Configuration --------------------------------------------------

napoleon_google_docstring = True
napoleon_numpy_docstring = True
napoleon_include_init_with_doc = True

# -- Intersphinx Configuration -----------------------------------------------

intersphinx_mapping = {
    "python": ("https://docs.python.org/3", None),
    "numpy": ("https://numpy.org/doc/stable", None),
    "pandas": ("https://pandas.pydata.org/docs", None),
}
