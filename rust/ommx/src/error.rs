use crate::{ConstraintID, VariableID};
use prost::DecodeError;

/// Error occurred during parsing OMMX Message
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// Incompatibility due to `oneof` in protobuf. See [`crate::Function`] for more information.
    #[error(
        "Unsupported ommx.v1.Function is found. It is created by a newer version of OMMX SDK."
    )]
    UnsupportedV1Function,

    /// In proto3, all fields of message types are implicitly optional even if explicit `optional` flag is absent.
    /// When the SDK requires a field to be present, it will return this error.
    #[error("Field {field} in {message} is missing.")]
    MissingField {
        message: &'static str,
        field: &'static str,
    },

    /// Since protobuf requires all enum has `UNSPECIFIED` value as default value,
    /// this error is returned when the enum value is not set.
    #[error("Enum ({enum_name}) value is unspecified.")]
    UnspecifiedEnum { enum_name: &'static str },

    #[error("Duplicated constraint ID is found: {id:?}")]
    DuplicatedConstraintID { id: ConstraintID },

    #[error("Duplicated variable ID is found: {id:?}")]
    DuplicatedVariableID { id: VariableID },

    /// The wire format is invalid.
    #[error("Cannot decode as a Protobuf Message: {0}")]
    DecodeError(#[from] DecodeError),
}
