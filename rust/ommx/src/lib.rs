//! Open Mathematics prograMming eXchange (OMMX)
//!
//! Message as OCI Artifact
//! ------------------------
//! OMMX defines a protobuf schema. Then the [`v1::Instance`] and [`v1::Solution`] are serialized as protobuf messages,
//! i.e. a byte stream satisfying the schema.
//! This is enough for in process communication, but not enough for out of process communication.
//! For storing message on local disk or sharing with other applications via cloud storage,
//! OMMX also defines a metadata for each message, such as who and when this message was created by which application, and so on,
//! and treats the pair of metadata and message as OCI Artifact.
//!

pub mod artifact;
pub mod random;
pub use prost::Message;
mod arbitrary;
mod convert;
mod evaluate;

pub use evaluate::Evaluate;

/// Module created from `ommx.v1` proto files
pub mod v1 {
    include!("ommx.v1.rs");
}
