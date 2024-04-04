# OMMX

Open Mathematical prograMming eXchange (OMMX) is an open ecosystem that empowers mathematical programming and optimization developers and reserchers.

# API Reference

- [Protobuf](https://jij-inc.github.io/ommx/protobuf.html)
- [Rust](https://jij-inc.github.io/ommx/rust/ommx/index.html)

# Compatibility

This project manages several versions as follows:

## Protocol buffers schema

- OMMX defines a protocol buffers schema with version like `v1`, `v2`, etc. `v1` schema has a namesapce `ommx.v1`.
- Schemas in `ommx.v1` will be compatible after [`1.0.0` release](https://github.com/Jij-Inc/ommx/milestone/3). Note that the schema can be changed incompatible way until this release.
- `v2` schema with namespace `ommx.v2` will start developing if we need to change the schema in incompatible way after `1.0.0` release. Compatible changes will be made in `v1` schema also after `1.0.0` release. We never create namespaces like `ommx.v1_1`.

## `ommx` Rust crate
TBW

## `ommx` Python package
TBW

# License
© 2024 Jij Inc.

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

# Contribution
TBW
