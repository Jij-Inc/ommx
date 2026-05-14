//! # Rust SDK for OMMX (Open Mathematics prograMming eXchange)
//!
//! This crate provides native Rust types and operations for mathematical
//! optimization problems. It offers type-safe, high-performance implementations
//! with convenient macros for expression building.
//!
//! See [`doc::tutorial`] for a guided walkthrough of the public API, and
//! [`doc::migration_guide`] / [`doc::release_note`] for version-specific notes.

// Allow the `ommx-derive` proc-macro to refer to this crate as `::ommx` when
// its generated code is compiled inside this crate itself.
extern crate self as ommx;

// Re-export the dependencies
pub use prost::Message;

// Public modules
pub mod artifact;
#[cfg(feature = "remote-artifact")]
pub mod dataset;
pub mod experiment;
pub(crate) mod logical_memory;
pub(crate) use logical_memory::impl_logical_memory_profile;
pub use logical_memory::MemoryProfile;
pub mod mps;
pub mod parse;
pub mod qplib;
pub mod random;

// Internal modules
mod atol;
mod bound;
mod coefficient;
mod constraint;
mod constraint_hints;
mod constraint_type;
mod decision_variable;
pub mod error;
mod evaluate;
mod format;
mod function;
mod indicator_constraint;
mod infeasible_detected;
mod instance;
mod macros;
mod named_function;
mod one_hot_constraint;
mod polynomial_base;
mod sample_set;
mod sampled;
mod solution;
mod sos1_constraint;
mod substitute;

pub use atol::*;
pub use bound::*;
pub use coefficient::*;
pub use constraint::*;
pub use constraint_type::*;
pub use decision_variable::*;
pub use error::*;
pub use evaluate::{Evaluate, Propagate, PropagateOutcome};
pub use function::*;
pub use indicator_constraint::*;
pub use infeasible_detected::*;
pub use instance::*;
pub use named_function::*;
pub use one_hot_constraint::*;
pub use parse::*;
pub use polynomial_base::*;
pub use sample_set::*;
pub use sampled::*;
pub use solution::*;
pub use sos1_constraint::*;
pub use substitute::*;

/// The `format_version` this SDK produces and the maximum it can read.
///
/// This SDK writes `format_version = CURRENT_FORMAT_VERSION` on all top-level
/// messages and accepts any value `<= CURRENT_FORMAT_VERSION` on parse. Data
/// whose `format_version` exceeds this value was produced by a newer SDK with
/// semantic-breaking format changes and cannot be read correctly.
/// See `proto/ommx/v1/instance.proto` for the full policy.
pub const CURRENT_FORMAT_VERSION: u32 = 0;

/// Module created from `ommx.v1` proto files
#[allow(clippy::doc_overindented_list_items)] // prost breaks markdown indents
pub mod v1 {
    include!("ommx.v1.rs");
}

mod v1_io;

/// Supplementary documentation bundled with the crate.
///
/// Each submodule renders a Markdown file from `rust/ommx/doc/` as rustdoc
/// so it is browsable on docs.rs alongside the API reference. Gated behind
/// `#[cfg(doc)]` — present only when rustdoc runs, absent from normal
/// `cargo build` / `cargo check` output.
#[cfg(doc)]
pub mod doc {
    #[doc = include_str!("../doc/tutorial.md")]
    pub mod tutorial {
        #[doc = include_str!("../doc/tutorial/expressions.md")]
        pub mod expressions {}

        #[doc = include_str!("../doc/tutorial/decision_variables.md")]
        pub mod decision_variables {}

        #[doc = include_str!("../doc/tutorial/constraints.md")]
        pub mod constraints {}

        #[doc = include_str!("../doc/tutorial/instance.md")]
        pub mod instance {}

        #[doc = include_str!("../doc/tutorial/evaluate.md")]
        pub mod evaluate {}

        #[doc = include_str!("../doc/tutorial/solution.md")]
        pub mod solution {}

        #[doc = include_str!("../doc/tutorial/substitute.md")]
        pub mod substitute {}

        #[doc = include_str!("../doc/tutorial/error_handling.md")]
        pub mod error_handling {}
    }

    #[doc = include_str!("../doc/migration_guide.md")]
    pub mod migration_guide {}

    #[doc = include_str!("../doc/release_note.md")]
    pub mod release_note {}

    #[doc = include_str!("../doc/artifact_design.md")]
    pub mod artifact_design {}
}
