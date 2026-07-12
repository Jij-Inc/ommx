//! Private proof-verification infrastructure.
//!
//! This module is intentionally not part of the Rust SDK surface. It owns
//! exact, immutable representations against which OMMX core can verify
//! untrusted presolve and inverse-lowering evidence. Root objects such as
//! [`crate::Instance`] remain the only mutation authority.

// The private proof vocabulary is defined as a complete representation before
// every verifier needs every atom. It is intentionally unreachable from the
// SDK surface.
#![allow(dead_code)]

mod exact;
mod linear;
