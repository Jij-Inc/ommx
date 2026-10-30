use crate::error::OmmxPyResult;
use ommx::Message;

/// Decode one protobuf message through the binding-owned error boundary.
pub fn decode<M>(bytes: &[u8], root: &'static str) -> OmmxPyResult<M>
where
    M: Message + Default,
{
    let message = M::decode(bytes)
        .map_err(|error| ommx::RawParseError::from(error).context(root, "bytes"))?;
    Ok(message)
}
