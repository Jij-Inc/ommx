---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
---

# Supported OMMX Adapters
To solve mathematical optimization problems described in OMMX using solvers, it is necessary to convert them into data structures that conform to the solver's specifications. OMMX Adapters play this conversion role. Since specifications differ for each solver, there exists an adapter for each solver.

## Adapters for OSS solvers/samplers
Several adapters for OSS solvers/samplers are supported in OMMX repository.

| Package name | PyPI | API Reference | Description |
|:--- |:--- |:--- |:--- |
| [ommx-highs-adapter](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-highs-adapter) | [![ommx-highs-adapter](https://img.shields.io/pypi/v/ommx-highs-adapter)](https://pypi.org/project/ommx-highs-adapter/) | [![main](https://img.shields.io/badge/API_Reference-main-blue)](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx_highs_adapter/index.html) | Adapter for [HiGHS](https://github.com/ERGO-Code/HiGHS)
| [ommx-openjij-adapter](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-openjij-adapter) | [![ommx-openjij-adapter](https://img.shields.io/pypi/v/ommx-openjij-adapter)](https://pypi.org/project/ommx-openjij-adapter/) | [![main](https://img.shields.io/badge/API_Reference-main-blue)](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx_openjij_adapter/index.html) | Adapter for [OpenJij](https://github.com/OpenJij/OpenJij)
| [ommx-python-mip-adapter](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-python-mip-adapter) | [![ommx-python-mip-adapter](https://img.shields.io/pypi/v/ommx-python-mip-adapter)](https://pypi.org/project/ommx-python-mip-adapter/) | [![main](https://img.shields.io/badge/API_Reference-main-blue)](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx_python_mip_adapter/index.html)| Adapter for [Python-MIP](https://www.python-mip.com/) |
| [ommx-pyscipopt-adapter](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-pyscipopt-adapter) | [![ommx-pyscipopt-adapter](https://img.shields.io/pypi/v/ommx-pyscipopt-adapter)](https://pypi.org/project/ommx-pyscipopt-adapter/) | [![main](https://img.shields.io/badge/API_Reference-main-blue)](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx_pyscipopt_adapter/index.html) | Adapter for [PySCIPOpt](https://github.com/scipopt/PySCIPOpt)

## Adapters for Non-OSS solvers/samplers
Non-OSS solvers/samplers are also supported in other repositories.

| Package name | PyPI | Description |
|:--- |:--- |:--- |
| [ommx-da4-adapter](https://github.com/Jij-Inc/ommx-da4-adapter) | [![ommx-da4-adapter](https://img.shields.io/pypi/v/ommx-da4-adapter)](https://pypi.org/project/ommx-da4-adapter/) | Adapter for [Fujitsu Digital Annealer(DA4)](https://www.fujitsu.com/jp/digitalannealer/) |
|  [ommx-dwave-adapter](https://github.com/Jij-Inc/ommx-dwave-adapter) | [![ommx-dwave-adapter](https://img.shields.io/pypi/v/ommx-dwave-adapter)](https://pypi.org/project/ommx-dwave-adapter) | Adapter for [D-Wave](https://docs.dwavequantum.com/en/latest/index.html) |
| [ommx-fixstars-amplify-adapter](https://github.com/Jij-Inc/ommx-fixstars-amplify-adapter) | [![ommx-fixstars-amplify-adapter](https://img.shields.io/pypi/v/ommx-fixstars-amplify-adapter)](https://pypi.org/project/ommx-fixstars-amplify-adapter/) | Adapter for [Fixstars Amplify](https://amplify.fixstars.com/ja/docs/amplify/v1/index.html#) |
| [ommx-gurobipy-adapter](https://github.com/Jij-Inc/ommx-gurobipy-adapter) | [![ommx-gurobipy-adapter](https://img.shields.io/pypi/v/ommx-gurobipy-adapter)](https://pypi.org/project/ommx-gurobipy-adapter/) | Adapter for [Gurobi](https://www.gurobi.com/) |
| [ommx-kipu-iskay-adapter](https://github.com/Jij-Inc/ommx-kipu-iskay-adapter) | [![ommx-kipu-iskay-adapter](https://img.shields.io/pypi/v/ommx-kipu-iskay-adapter)](https://pypi.org/project/ommx-kipu-iskay-adapter/) | Adapter for [Kipu Iskay through Qiskit Functions Catalog](https://quantum.cloud.ibm.com/docs/en/guides/kipu-optimization) |
| [ommx-qctrl-qaoa-adapter](https://github.com/Jij-Inc/ommx-qctrl-qaoa-adapter) | [![ommx-qctrl-qaoa-adapter](https://img.shields.io/pypi/v/ommx-qctrl-qaoa-adapter)](https://pypi.org/project/ommx-qctrl-qaoa-adapter/) | Adapter for [Fire Opal QAOA Solver](https://docs.q-ctrl.com/fire-opal/execute/run-algorithms/solve-optimization-problems/fire-opals-qaoa-solver) |

```{code-cell}
---
vscode:
  languageId: plaintext
---

```
