# OMMX

[![Docs/en](https://img.shields.io/badge/Docs-English-blue)](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/)
[![Docs/ja](https://img.shields.io/badge/Docs-日本語-blue)](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/)
[![Discord-EN](https://img.shields.io/badge/Discord-English-default?logo=Discord)](https://discord.gg/QPMbdJCbAB)
[![Discord-JP](https://img.shields.io/badge/Discord-日本語-default?logo=Discord)](https://discord.gg/CPd9ZsxtR5)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/Jij-Inc/ommx)

Open Mathematical prograMming eXchange (OMMX) is an open ecosystem that empowers mathematical programming and optimization developers and reserchers.

# Tutorial

## English

| Title | Docs |
|:------|:-----|
| Solve a 0-1 Knapsack Problem with the PySCIPOpt Adapter | [![Docs](https://img.shields.io/badge/Docs-English-blue)](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/tutorial/solve_with_ommx_adapter.html) |
| Solve Special Constraints Directly with the PySCIPOpt Adapter | [![Docs](https://img.shields.io/badge/Docs-English-blue)](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/tutorial/solve_special_constraints_with_pyscipopt_adapter.html) |
| Prepare an Instance for an Adapter | [![Docs](https://img.shields.io/badge/Docs-English-blue)](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/tutorial/prepare_instance_for_adapter.html) |
| Sample with the OpenJij Adapter | [![Docs](https://img.shields.io/badge/Docs-English-blue)](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/tutorial/tsp_sampling_with_openjij_adapter.html) |
| Solve with Multiple Adapters and Compare the Results | [![Docs](https://img.shields.io/badge/Docs-English-blue)](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/tutorial/switching_adapters.html) |
| Recording and Sharing Experiments | [![Docs](https://img.shields.io/badge/Docs-English-blue)](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/tutorial/experiment_management.html) |
| Sharing Various Types of Data in an OMMX Artifact | [![Docs](https://img.shields.io/badge/Docs-English-blue)](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/tutorial/share_in_ommx_artifact.html) |
| Download Benchmark Instances | [![Docs](https://img.shields.io/badge/Docs-English-blue)](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/tutorial/download_benchmark_instance.html) |

## 日本語

| タイトル | Docs |
|:------|:-----|
| PySCIPOpt Adapterで0-1ナップサック問題を解く | [![Docs](https://img.shields.io/badge/Docs-日本語-blue)](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/tutorial/solve_with_ommx_adapter.html) |
| PySCIPOpt Adapterで特殊制約をそのまま解く | [![Docs](https://img.shields.io/badge/Docs-日本語-blue)](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/tutorial/solve_special_constraints_with_pyscipopt_adapter.html) |
| Adapter向けにInstanceを準備する | [![Docs](https://img.shields.io/badge/Docs-日本語-blue)](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/tutorial/prepare_instance_for_adapter.html) |
| OpenJij Adapterでサンプリングする | [![Docs](https://img.shields.io/badge/Docs-日本語-blue)](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/tutorial/tsp_sampling_with_openjij_adapter.html) |
| 複数のAdapterで最適化問題を解いて結果を比較する | [![Docs](https://img.shields.io/badge/Docs-日本語-blue)](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/tutorial/switching_adapters.html) |
| 実験を記録し共有する | [![Docs](https://img.shields.io/badge/Docs-日本語-blue)](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/tutorial/experiment_management.html) |
| 様々なデータをOMMX Artifact形式で共有する | [![Docs](https://img.shields.io/badge/Docs-日本語-blue)](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/tutorial/share_in_ommx_artifact.html) |
| ベンチマークInstanceをダウンロードする | [![Docs](https://img.shields.io/badge/Docs-日本語-blue)](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/tutorial/download_benchmark_instance.html) |

# Developer Guide

| Audience | Docs |
|:---------|:-----|
| Implement an OMMX Adapter | [![Docs](https://img.shields.io/badge/Docs-English-blue)](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/developer_guide/adapter.html) |
| OMMX Adapterを実装する | [![Docs](https://img.shields.io/badge/Docs-日本語-blue)](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/developer_guide/adapter.html) |

# SDK

[![CodSpeed Badge](https://img.shields.io/endpoint?url=https://codspeed.io/badge.json)](https://codspeed.io/Jij-Inc/ommx?utm_source=badge)

See [DEVELOPMENT.md](./DEVELOPMENT.md) about developing this project.

| SDK | Package | API Reference |
|:----|:--------|:--------------|
| Rust | [![crates.io](https://img.shields.io/crates/v/ommx)](https://crates.io/crates/ommx) | [![docs.rs](https://docs.rs/ommx/badge.svg)](https://docs.rs/ommx) |
| Rust-to-Python PyO3 bridge | [![crates.io](https://img.shields.io/crates/v/ommx-pyo3-bridge)](https://crates.io/crates/ommx-pyo3-bridge) | [![docs.rs](https://docs.rs/ommx-pyo3-bridge/badge.svg)](https://docs.rs/ommx-pyo3-bridge) |
| Python | [![PyPI](https://img.shields.io/pypi/v/ommx)](https://pypi.org/project/ommx/) | [![Docs](https://img.shields.io/badge/API_Reference-main-blue)](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/api/index.html) |

See [Supported Adapters](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/user_guide/supported_ommx_adapters.html) about a complete list of supported adapters for each solver/sampler.

## CLI

Prebuilt `ommx` CLI archives are attached to Rust SDK releases and can be installed with [`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall):

```bash
cargo binstall ommx
```

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
This work was performed for Council for Science, Technology and Innovation (CSTI), Cross-ministerial Strategic Innovation Promotion Program (SIP), "Promoting the application of advanced quantum technology platforms to social issues"(Funding agency : QST).
