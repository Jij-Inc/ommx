---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: .venv
  language: python
  name: python3
---

# OMMX Python SDK 1.6.0

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_1.6.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-1.6.0)

Summary
--------

- OMMX starts to support QUBO.
  - New adapter package [ommx-openjij-adapter](https://pypi.org/project/ommx-openjij-adapter/) has been added.
  - Please see new [tutorial page](https://jij-inc.github.io/ommx/en/tutorial/tsp_sampling_with_openjij_adapter.html)
  - Several APIs are added for converting `ommx.v1.Instance` into QUBO format. Please see the above tutorial.
- Python 3.8 support has been dropped due to its EOL
