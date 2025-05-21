---
applyTo: "rust/**/*"
---

# Rust SDK (ommx crate) Development Guidelines

This document provides specific instructions for developing the Rust SDK, primarily within the `ommx` crate located in the `rust/ommx/` directory.

## Core Design Principle: Idiomatic Rust Structures vs. Protobuf-Generated Structures

A fundamental architectural decision for the Rust SDK (version 2.0.0 and onwards) is the separation of concerns between idiomatic Rust structures and structures auto-generated from Protocol Buffer (`.proto`) definitions.

**1. Idiomatic Rust Structures (e.g., `ommx::Instance`, `ommx::Function`):**
    *   These are the primary structures to be used for all **core logic, internal operations, and public-facing APIs** of the SDK.
    *   They should be designed following Rust best practices and idiomatic patterns.
    *   **Goal:** Provide a developer-friendly, type-safe, and efficient interface for working with OMMX data within Rust.

**2. Protobuf-Generated Structures (e.g., `ommx::v1::Instance`, `ommx::v1::Function`):**
    *   These structures are automatically generated from the `.proto` files (e.g., those in `proto/ommx/v1/`).
    *   Their **sole purpose** is for **serialization and deserialization** of data to and from the OMMX binary format.
    *   They should **not** be used directly for implementing core SDK logic or in public APIs beyond the raw serialization/deserialization layer.

**Data Flow Expectation:**

*   **Reading OMMX data:**
    1.  Deserialize binary data into Protobuf-generated structs (e.g., `ommx::v1::Instance`).
    2.  Convert/map these Protobuf-generated structs into their corresponding idiomatic Rust structs (e.g., `ommx::Instance`).
    3.  Use the idiomatic Rust structs for all subsequent operations.
*   **Writing OMMX data:**
    1.  Convert/map idiomatic Rust structs (e.g., `ommx::Instance`) into their corresponding Protobuf-generated structs (e.g., `ommx::v1::Instance`).
    2.  Serialize the Protobuf-generated structs into binary data.

## Transition and Current State

*   This architectural shift is an **ongoing process**.
*   You may find parts of the existing codebase where Protobuf-generated structs (`ommx::v1::*`) are still used directly in logic that should ideally use idiomatic Rust structs.
*   **When developing new features or refactoring existing code:**
    *   Prioritize the use of idiomatic Rust structs for all logic.
    *   Implement the necessary conversions to/from the `ommx::v1::*` Protobuf structs at the serialization boundaries.
    *   If you encounter legacy code directly using `ommx::v1::*` for logic, and it's within the scope of your task, consider refactoring it to align with this new principle.

## Summary of Key Types:

*   `ommx::Instance`: Preferred idiomatic Rust struct for representing an optimization instance.
*   `ommx::v1::Instance`: Protobuf-generated struct, use only for serializing/deserializing `ommx::Instance`.
*   This pattern applies to other core OMMX entities like `Function`, `Constraint`, `Variable`, etc. (e.g., `ommx::Function` vs. `ommx::v1::Function`).

By adhering to these guidelines, we aim to create a more maintainable, robust, and idiomatic Rust SDK.

