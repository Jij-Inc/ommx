//! Open Mathematics prograMming eXchange (OMMX)

pub use prost::Message;

/// Module created from `ommx.v1` proto files
pub mod v1 {
    include!("ommx.v1.rs");
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    DecodeError(#[from] prost::DecodeError),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Decode a [`v1::Instance`] from a byte slice
pub fn decode_instance(buf: &[u8]) -> Result<v1::Instance> {
    Ok(v1::Instance::decode(buf)?)
}
