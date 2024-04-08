//! Open Mathematics prograMming eXchange (OMMX)

pub mod archive;
pub mod error;
pub use prost::Message;

/// Module created from `ommx.v1` proto files
pub mod v1 {
    include!("ommx.v1.rs");
}
