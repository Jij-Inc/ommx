# OMMX
[![PyPI - Version](https://img.shields.io/pypi/v/ommx)](https://pypi.org/project/ommx/)
[![Crates.io Version](https://img.shields.io/crates/v/ommx)](https://crates.io/crates/ommx)

Open Mathematical prograMming eXchange (OMMX) is an open ecosystem that empowers mathematical programming and optimization developers and reserchers.

# API Reference

- [Protobuf](https://jij-inc.github.io/ommx/protobuf.html)
- [Rust](https://jij-inc.github.io/ommx/rust/ommx/index.html)
- [Python](https://jij-inc.github.io/ommx/python/index.html)

See [DEVELOPMENT.md](./DEVELOPMENT.md) about developing this project.

# Compatibility

This project manages several versions as follows:

## Protocol buffers schema

- OMMX defines a protocol buffers schema with version like `v1`, `v2`, etc. `v1` schema has a namesapce `ommx.v1`.
- Schemas in `ommx.v1` will be compatible after [ommx.v1 schema release](https://github.com/Jij-Inc/ommx/milestone/3). Note that the schema can be changed incompatible way until this release.
- `v2` schema with namespace `ommx.v2` will start developing if we need to change the schema in incompatible way after `ommx.v1` release. Compatible changes will be made in `v1` schema also after its release. We never create namespaces like `ommx.v1_1`.

## Language specific bindings
This project consists of main protobuf schema and language specific bindings.
Their versions are managed independently, and adopts semantic versioning.

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
