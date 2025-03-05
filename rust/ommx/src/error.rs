use prost::DecodeError;

/// Error occurred during parsing OMMX Message
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// See [`crate::Function`] for more information.
    #[error(
        "Unsupported ommx.v1.Function is found. It is created by a newer version of OMMX SDK."
    )]
    UnsupportedV1Function,

    #[error("Enum ({enum_name}) value is unspecified.")]
    UnspecifiedEnum { enum_name: &'static str },

    #[error("Cannot decode as a Protobuf Message: {0}")]
    DecodeError(#[from] DecodeError),
}
