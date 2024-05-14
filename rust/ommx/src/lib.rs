//! Open Mathematics prograMming eXchange (OMMX)

pub use prost::Message;

/// Module created from `ommx.v1` proto files
pub mod v1 {
    include!("ommx.v1.rs");
}

mod arbitrary;
mod convert;
pub mod random;
