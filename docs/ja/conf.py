"""Sphinx configuration for OMMX Japanese documentation."""

import sys
from pathlib import Path

import tomlkit

# -- Load shared configuration -----------------------------------------------

here = Path(__file__).parent
docs_root = here.parent
exec(open(docs_root / "conf_base.py").read())

# -- Path setup --------------------------------------------------------------

python_root = docs_root.parent / "python"

# Add the API docs directory to Python path for pyo3_stub_gen_ext
sys.path.insert(0, str(docs_root / "api"))

# -- Project information -----------------------------------------------------

project = "OMMX"
copyright = "2024, Jij Inc."
author = "Jij Inc."
language = "ja"

pyproject_toml = tomlkit.loads((python_root / "ommx" / "pyproject.toml").read_text())
version = str(pyproject_toml["project"]["version"])  # type: ignore
release = version

# -- Additional extensions for API Reference ----------------------------------

extensions += [  # noqa: F821
    "sphinx.ext.autodoc",
    "sphinx_fontawesome",
    "autoapi.extension",
    "pyo3_stub_gen_ext",
]

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
