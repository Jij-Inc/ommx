use crate::{ConstraintID, VariableID};
use prost::DecodeError;
use std::fmt;

/// A wrapper of [`TryFrom`] trait to provide a backtrace of parsing error.
pub trait Parse<Output> {
    fn parse(self, message: &'static str, field: &'static str) -> Result<Output, ParseError>;
}

impl<Input, Output> Parse<Output> for Input
where
    Output: TryFrom<Input, Error = ParseError>,
{
    fn parse(self, message: &'static str, field: &'static str) -> Result<Output, ParseError> {
        self.try_into().map_err(|mut err: ParseError| {
            err.context.push(ParseContext { message, field });
            err
        })
    }
}

#[derive(Debug)]
pub struct ParseError {
    pub context: Vec<ParseContext>,
    pub error: RawParseError,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Traceback for OMMX Message parse error:")?;
        let mut width = 0;
        for ctx in self.context.iter().rev() {
            writeln!(f, "{:width$}└─{}[{}]", "", ctx.message, ctx.field,)?;
            width += 2;
        }
        writeln!(f, "{}", self.error)
    }
}

impl std::error::Error for ParseError {}

impl From<RawParseError> for ParseError {
    fn from(error: RawParseError) -> Self {
        ParseError {
            context: vec![],
            error,
        }
    }
}

#[derive(Debug)]
pub struct ParseContext {
    pub message: &'static str,
    pub field: &'static str,
}

/// Error occurred during parsing OMMX Message
#[derive(Debug, thiserror::Error)]
pub enum RawParseError {
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

    #[error("Undefined variable ID is used: {id:?}")]
    UndefinedVariableID { id: VariableID },

    #[error("Undefined constraint ID is used: {id:?}")]
    UndefinedConstraintID { id: ConstraintID },

    /// The wire format is invalid.
    #[error("Cannot decode as a Protobuf Message: {0}")]
    DecodeError(#[from] DecodeError),
}
