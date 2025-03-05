# OMMX

[![Book/en](https://img.shields.io/badge/Book-English-blue)](https://jij-inc.github.io/ommx/en)
[![Book/ja](https://img.shields.io/badge/Book-日本語-blue)](https://jij-inc.github.io/ommx/ja)
[![Discord-EN](https://img.shields.io/badge/Discord-English-default?logo=Discord)](https://discord.gg/aGfCmnJp)
[![Discord-JP](https://img.shields.io/badge/Discord-日本語-default?logo=Discord)](https://discord.gg/ckAgHqE3)
[![LLMs.txt](https://img.shields.io/badge/LLMs.txt-AI_Documentation-green)](https://raw.githubusercontent.com/Jij-Inc/ommx/refs/heads/main/LLMs.txt)

Open Mathematical prograMming eXchange (OMMX) is an open ecosystem that empowers mathematical programming and optimization developers and reserchers.

# Tutorial

## English

- [LLMs.txt](https://raw.githubusercontent.com/Jij-Inc/ommx/refs/heads/main/LLMs.txt) - Documentation formatted for AI assistants like ChatGPT or NotebookLM.

| Title | Jupyter Book |
|:------|:-------------|
| Solving Optimization Problems with OMMX Adapter | [![Book](https://img.shields.io/badge/Book-English-blue)](https://jij-inc.github.io/ommx/en/tutorial/solve_with_ommx_adapter.html) |
| Sampling from QUBO with OMMX Adapter | [![Book](https://img.shields.io/badge/Book-English-blue)](https://jij-inc.github.io/ommx/en/tutorial/tsp_sampling_with_openjij_adapter.html) |
| Downloading a MIPLIB Instance | [![Book](https://img.shields.io/badge/Book-English-blue)](https://jij-inc.github.io/ommx/en/tutorial/download_miplib_instance.html) |

## 日本語

| タイトル | Jupyter Book |
|:------|:-------------|
| OMMX Adapterで最適化問題を解く | [![Book](https://img.shields.io/badge/Book-日本語-blue)](https://jij-inc.github.io/ommx/ja/tutorial/solve_with_ommx_adapter.html) |
| OMMX AdapterでQUBOからサンプリングする | [![Book](https://img.shields.io/badge/Book-日本語-blue)](https://jij-inc.github.io/ommx/ja/tutorial/tsp_sampling_with_openjij_adapter.html) |
| MIPLIBインスタンスをダウンロードする | [![Book](https://img.shields.io/badge/Book-日本語-blue)](https://jij-inc.github.io/ommx/ja/tutorial/download_miplib_instance.html) |

# API Reference

See [DEVELOPMENT.md](./DEVELOPMENT.md) about developing this project.

## Rust SDK

| Crate name | crates.io | API Reference (stable) | API Reference (main) | Benchmark |
|:----|:----|:----|:----|:----|
| [ommx](./rust/ommx/) | [![ommx](https://img.shields.io/crates/v/ommx)](https://crates.io/crates/ommx) | [![docs.rs](https://docs.rs/ommx/badge.svg)](https://docs.rs/ommx) | [![main](https://img.shields.io/badge/docs-main-blue)](https://jij-inc.github.io/ommx/rust/ommx/index.html) | [![CodSpeed Badge](https://img.shields.io/endpoint?url=https://codspeed.io/badge.json)](https://codspeed.io/Jij-Inc/ommx) |

## Python SDK

OMMX Python SDK consists of the main package `ommx` and several adapters for OSS solvers.

| Package name | PyPI | API Reference | Description |
|:--- |:--- |:--- |:--- |
| [ommx](./python/ommx) | [![ommx](https://img.shields.io/pypi/v/ommx)](https://pypi.org/project/ommx/) | [![main](https://img.shields.io/badge/API_Reference-main-blue)](https://jij-inc.github.io/ommx/python/ommx/autoapi/index.html) | Core package of OMMX Python SDK |
| [ommx-python-mip-adapter](./python/ommx-python-mip-adapter/) | [![ommx-python-mip-adapter](https://img.shields.io/pypi/v/ommx-python-mip-adapter)](https://pypi.org/project/ommx-python-mip-adapter/) | [![main](https://img.shields.io/badge/API_Reference-main-blue)](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx_python_mip_adapter/index.html)| Adapter for [Python-MIP](https://www.python-mip.com/) |
| [ommx-pyscipopt-adapter](./python/ommx-pyscipopt-adapter/) | [![ommx-pyscipopt-adapter](https://img.shields.io/pypi/v/ommx-pyscipopt-adapter)](https://pypi.org/project/ommx-pyscipopt-adapter/) | [![main](https://img.shields.io/badge/API_Reference-main-blue)](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx_pyscipopt_adapter/index.html) | Adapter for [PySCIPOpt](https://github.com/scipopt/PySCIPOpt)
| [ommx-highs-adapter](./python/ommx-highs-adapter/) | [![ommx-highs-adapter](https://img.shields.io/pypi/v/ommx-highs-adapter)](https://pypi.org/project/ommx-highs-adapter/) | [![main](https://img.shields.io/badge/API_Reference-main-blue)](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx_highs_adapter/index.html) | Adapter for [HiGHS](https://github.com/ERGO-Code/HiGHS)
| [ommx-openjij-adapter](./python/ommx-openjij-adapter/) | [![ommx-openjij-adapter](https://img.shields.io/pypi/v/ommx-openjij-adapter)](https://pypi.org/project/ommx-openjij-adapter/) | [![main](https://img.shields.io/badge/API_Reference-main-blue)](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx_openjij_adapter/index.html) | Adapter for [OpenJij](https://github.com/OpenJij/OpenJij)

Non-OSS solvers are also supported in other repositories.

| Package name | PyPI | Description |
|:--- |:--- |:--- |
| [ommx-gurobipy-adapter](https://github.com/Jij-Inc/ommx-gurobipy-adapter) | [![ommx-gurobipy-adapter](https://img.shields.io/pypi/v/ommx-gurobipy-adapter)](https://pypi.org/project/ommx-gurobipy-adapter/) | Adapter for [Gurobi](https://www.gurobi.com/) |
| [ommx-fixstars-amplify-adapter](https://github.com/Jij-Inc/ommx-fixstars-amplify-adapter) | [![ommx-fixstars-amplify-adapter](https://img.shields.io/pypi/v/ommx-fixstars-amplify-adapter)](https://pypi.org/project/ommx-fixstars-amplify-adapter/) | Adapter for [Fixstars Amplify](https://amplify.fixstars.com/ja/docs/amplify/v1/index.html#) |

# License
© 2024 Jij Inc.

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

# Contribution
TBW

# Acknowledgement
![BRIDGE](./BRIDGE.png)
This work was performed for Council for Science, Technology and Innovation (CSTI), Cross-ministerial Strategic Innovation Promotion Program (SIP), “Promoting the application of advanced quantum technology platforms to social issues”(Funding agency : QST).
