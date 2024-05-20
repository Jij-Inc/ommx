# Configuration file for the Sphinx documentation builder.
#
# For the full list of built-in configuration values, see the documentation:
# https://www.sphinx-doc.org/en/master/usage/configuration.html

# -- Path setup --------------------------------------------------------------
import sphinx_rtd_theme
import sphinx_fontawesome

# -- Project information -----------------------------------------------------
# https://www.sphinx-doc.org/en/master/usage/configuration.html#project-information

project = "ommx"
copyright = "2024, Jij Inc."
author = "Jij Inc."

version = "0.1.0"
release = "0.1.0"

# -- General configuration ---------------------------------------------------
# https://www.sphinx-doc.org/en/master/usage/configuration.html#general-configuration

extensions = [
    "sphinx.ext.autodoc",
    "sphinx.ext.napoleon",
    "sphinx_rtd_theme",
    "sphinx_fontawesome",
    "autoapi.extension",
]
source_suffix = {
    ".rst": "restructuredtext",
    ".md": "markdown",
}

templates_path = ["_templates"]
language = "en"
exclude_patterns = ["_build", "Thumbs.db", ".DS_Store"]

# -- Options for HTML output -------------------------------------------------
# https://www.sphinx-doc.org/en/master/usage/configuration.html#options-for-html-output

html_theme = "sphinx_rtd_theme"
html_theme_path = [sphinx_rtd_theme.get_html_theme_path()]
html_show_sourcelink = False
html_static_path = ["_static"]


# -- AutoAPI settings --------------------------------------------------------
# https://sphinx-autoapi.readthedocs.io/en/latest/reference/config.html#event-autoapi-skip-member

autoapi_dirs = ["../../ommx"]
autoapi_options = [
    "members",
    "undoc-members",
    "show-module-summary",
]
autoapi_member_order = "groupwise"
autoapi_file_patterns = ["*.pyi", "*.py"]


def skip_member(app, what, name, obj, skip, options):
    # Ignore the members for generating protobuf.
    if "global__" in name:
        return True
    if "DESCRIPTOR" in name:
        return True

    # Not display the string that generated by mypy-protobuf.
    if what == "module":
        obj.docstring = None

    return skip


def setup(sphinx):
    sphinx.connect("autoapi-skip-member", skip_member)
