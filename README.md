# OMMX

[![main](https://img.shields.io/badge/Python_SDK-API_Reference-blue)](https://jij-inc.github.io/ommx/python/ommx/autoapi/index.html)
[![main](https://img.shields.io/badge/Rust_SDK-API_Reference-blue)](https://jij-inc.github.io/ommx/rust/ommx/index.html)

Open Mathematical prograMming eXchange (OMMX) is an open ecosystem that empowers mathematical programming and optimization developers and reserchers.

# Design

OMMX introduces two specification to solve the problem of data exchange in optimization field:

- Protocol buffers based data schema called [OMMX Message](./MESSAGE.md). This helps to store the optimization models and their solutions in language and framework agnostic way.
- OCI Artifact based packaging and distribution specification called [OMMX Artifact](./ARTIFACT.md). This helps to store your data with metadata and to exchange them with others as a container image.

# Tutorial

| Notebook | Open in Binder | Open in Colab |
|:---------|:---------------|:--------------|
| [OMMX Message](./notebooks/message.ipynb) | [![Binder](https://mybinder.org/badge_logo.svg)](https://mybinder.org/v2/gh/Jij-Inc/ommx/main?labpath=notebooks%2Fmessage.ipynb) | [![Open In Colab](https://colab.research.google.com/assets/colab-badge.svg)](https://colab.research.google.com/github/Jij-Inc/ommx/blob/main/notebooks/message.ipynb) |
| [OMMX Artifact](./notebooks/artifact.ipynb) | [![Binder](https://mybinder.org/badge_logo.svg)](https://mybinder.org/v2/gh/Jij-Inc/ommx/main?labpath=notebooks%2Fartifact.ipynb) | [![Open In Colab](https://colab.research.google.com/assets/colab-badge.svg)](https://colab.research.google.com/github/Jij-Inc/ommx/blob/main/notebooks/artifact.ipynb) |
| [Cookbook](./notebooks/cookbook.ipynb) | [![Binder](https://mybinder.org/badge_logo.svg)](https://mybinder.org/v2/gh/Jij-Inc/ommx/main?labpath=notebooks%2Fcookbook.ipynb) | [![Open In Colab](https://colab.research.google.com/assets/colab-badge.svg)](https://colab.research.google.com/github/Jij-Inc/ommx/blob/main/notebooks/cookbook.ipynb) |
| [Create OMMX Adapters](./notebooks/create_adapter.ipynb) | [![Binder](https://mybinder.org/badge_logo.svg)](https://mybinder.org/v2/gh/Jij-Inc/ommx/main?labpath=notebooks%2Fcreate_adapter.ipynb) | [![Open In Colab](https://colab.research.google.com/assets/colab-badge.svg)](https://colab.research.google.com/github/Jij-Inc/ommx/blob/main/notebooks/create_adapter.ipynb) |


To run the notebooks locally, you need to install required packages listed in [requirements.txt](./requirements.txt)

```bash
# Optional: create a virtual environment
python -m venv .venv && source .venv/bin/activate

# Install required packages (including Jupyter)
pip install -r requirements.txt

# Start Jupyter
jupyter lab
```

# API Reference

See [DEVELOPMENT.md](./DEVELOPMENT.md) about developing this project.

## Rust SDK

| Crate name | crates.io | API Reference (stable) | API Reference (main) |
|:----|:----|:----|:----|
| [ommx](./rust/ommx/) | [![ommx](https://img.shields.io/crates/v/ommx)](https://crates.io/crates/ommx) | [![docs.rs](https://docs.rs/ommx/badge.svg)](https://docs.rs/ommx) | [![main](https://img.shields.io/badge/docs-main-blue)](https://jij-inc.github.io/ommx/rust/ommx/index.html) |

## Python SDK

| Package name | PyPI | API Reference (main) |
|:--- |:--- |:--- |
| [ommx](./python/ommx) | [![ommx](https://img.shields.io/pypi/v/ommx)](https://pypi.org/project/ommx/) | [![main](https://img.shields.io/badge/API_Reference-main-blue)](https://jij-inc.github.io/ommx/python/ommx/autoapi/index.html) |
| [ommx-python-mip-adapter](./python/ommx-python-mip-adapter/) | [![ommx-python-mip-adapter](https://img.shields.io/pypi/v/ommx-python-mip-adapter)](https://pypi.org/project/ommx-python-mip-adapter/) | [![main](https://img.shields.io/badge/API_Reference-main-blue)](https://jij-inc.github.io/ommx/python/ommx-python-mip-adapter/autoapi/index.html)|

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
