---
applyTo: "rust/ommx/**/*.rs"
---

# Development Note on Rust SDK 2.0.0

- The functionality of `ommx::v1::Instance` is passed to `ommx::Instance`.
  `ommx::v1::*` are only working for protobuf serialization and deserialization.
  - Same for `ommx::v1::Function` and `ommx::Function` and other types.

