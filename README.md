# OMMX

Open Mathematical prograMming eXchange (OMMX) is an open ecosystem that empowers mathematical programming and optimization developers and reserchers.

# Design

OMMX introduces two specification to solve the problem of data exchange in optimization field:

- Protocol buffers based data schema called [OMMX Message](./MESSAGE.md). This helps to store the optimization models and their solutions in language and framework agnostic way.
- OCI Artifact based packaging and distribution specification called [OMMX Artifact](./ARTIFACT.md). This helps to store your data with metadata and to exchange them with others as a container image.

# API Reference

See [DEVELOPMENT.md](./DEVELOPMENT.md) about developing this project.

## Rust SDK

| Crate name | crates.io | Document (stable) | Document (main) |
|:----|:----|:----|:----|
| [ommx](./rust/ommx/) | [![ommx](https://img.shields.io/crates/v/ommx)](https://crates.io/crates/ommx) | [![docs.rs](https://docs.rs/ommx/badge.svg)](https://docs.rs/ommx) | [![main](https://img.shields.io/badge/docs-main-blue)](https://jij-inc.github.io/ommx/rust/ommx/index.html) |

## Python SDK

| Package name | PyPI | Document (main) |
|:--- |:--- |:--- |
| [ommx](./python/ommx) | [![ommx](https://img.shields.io/pypi/v/ommx)](https://pypi.org/project/ommx/) | [![main](https://img.shields.io/badge/docs-main-blue)](https://jij-inc.github.io/ommx/python/ommx/index.html) |
| [ommx-python-mip-adapter](./python/ommx-python-mip-adapter/) | [![ommx-python-mip-adapter](https://img.shields.io/pypi/v/ommx-python-mip-adapter)](https://pypi.org/project/ommx-python-mip-adapter/) | [![main](https://img.shields.io/badge/docs-main-blue)](https://jij-inc.github.io/ommx/python/ommx-python-mip-adapter/index.html)|

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
