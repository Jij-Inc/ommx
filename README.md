# OMMX

[![Book/en](https://img.shields.io/badge/Book-English-blue)](https://jij-inc.github.io/ommx/en)
[![Book/ja](https://img.shields.io/badge/Book-日本語-blue)](https://jij-inc.github.io/ommx/ja)
[![Discord-EN](https://img.shields.io/badge/Discord-English-default?logo=Discord)](https://discord.gg/aGfCmnJp)
[![Discord-JP](https://img.shields.io/badge/Discord-日本語-default?logo=Discord)](https://discord.gg/ckAgHqE3)
[![LLMs.md](https://img.shields.io/badge/LLMs.md-AI_Documentation-green)](https://raw.githubusercontent.com/Jij-Inc/ommx/refs/heads/main/LLMs.md)

Open Mathematical prograMming eXchange (OMMX) is an open ecosystem that empowers mathematical programming and optimization developers and reserchers.

# Tutorial

## English

- [LLMs.md](https://raw.githubusercontent.com/Jij-Inc/ommx/refs/heads/main/LLMs.md) - Documentation formatted for AI assistants like ChatGPT or NotebookLM.

| Title | Jupyter Book |
|:------|:-------------|
| Solving Optimization Problems with OMMX Adapter | [![Book](https://img.shields.io/badge/Book-English-blue)](https://jij-inc.github.io/ommx/en/tutorial/solve_with_ommx_adapter.html) |
| Sampling from QUBO with OMMX Adapter | [![Book](https://img.shields.io/badge/Book-English-blue)](https://jij-inc.github.io/ommx/en/tutorial/tsp_sampling_with_openjij_adapter.html) |
| Comparing Results from Multiple Adapters | [![Book](https://img.shields.io/badge/Book-English-blue)](https://jij-inc.github.io/ommx/en/tutorial/switching_adapters.html) |
| Sharing Various Types of Data in an OMMX Artifact | [![Book](https://img.shields.io/badge/Book-English-blue)](https://jij-inc.github.io/ommx/en/tutorial/share_in_ommx_artifact.html) |
| Downloading a MIPLIB Instance | [![Book](https://img.shields.io/badge/Book-English-blue)](https://jij-inc.github.io/ommx/en/tutorial/download_miplib_instance.html) |
| Implementing an OMMX Adapter | [![Book](https://img.shields.io/badge/Book-English-blue)](https://jij-inc.github.io/ommx/en/tutorial/implement_adapter.html) |

## 日本語

| タイトル | Jupyter Book |
|:------|:-------------|
| OMMX Adapterで最適化問題を解く | [![Book](https://img.shields.io/badge/Book-日本語-blue)](https://jij-inc.github.io/ommx/ja/tutorial/solve_with_ommx_adapter.html) |
| OMMX AdapterでQUBOからサンプリングする | [![Book](https://img.shields.io/badge/Book-日本語-blue)](https://jij-inc.github.io/ommx/ja/tutorial/tsp_sampling_with_openjij_adapter.html) |
| 複数のAdapterで最適化問題を解いて結果を比較する | [![Book](https://img.shields.io/badge/Book-日本語-blue)](https://jij-inc.github.io/ommx/ja/tutorial/switching_adapters.html) |
| 様々なデータをOMMX Artifact形式で共有する | [![Book](https://img.shields.io/badge/Book-日本語-blue)](https://jij-inc.github.io/ommx/ja/tutorial/share_in_ommx_artifact.html) |
| MIPLIBインスタンスをダウンロードする | [![Book](https://img.shields.io/badge/Book-日本語-blue)](https://jij-inc.github.io/ommx/ja/tutorial/download_miplib_instance.html) |
| OMMX Adapterを実装する | [![Book](https://img.shields.io/badge/Book-日本語-blue)](https://jij-inc.github.io/ommx/ja/tutorial/implement_adapter.html) |

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

See [Supported Adapters](https://jij-inc.github.io/ommx/en/user_guide/supported_ommx_adapters.ipynb) about a complete list of supported adapters for each solver/sampler.

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
