use crate::{
    polynomial_base::QuadraticParseError, BoundError, CoefficientError, DecisionVariableError,
    InstanceError,
};
use prost::DecodeError;
use std::fmt;

/// Parse [`crate::v1`] messages into validated Rust types.
pub trait Parse: Sized {
    type Output;
    type Context;

    fn parse(self, context: &Self::Context) -> Result<Self::Output, ParseError>;

    fn parse_as(
        self,
        context: &Self::Context,
        message: &'static str,
        field: &'static str,
    ) -> Result<Self::Output, ParseError> {
        self.parse(context).map_err(|e| e.context(message, field))
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

impl ParseError {
    pub fn context(mut self, message: &'static str, field: &'static str) -> Self {
        self.context.push(ParseContext { message, field });
        self
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

    #[error(transparent)]
    InstanceError(#[from] InstanceError),

    #[error(transparent)]
    InvalidCoefficient(#[from] CoefficientError),

    #[error(transparent)]
    QuadraticParseError(#[from] QuadraticParseError),

    #[error(transparent)]
    InvalidBound(#[from] BoundError),

    #[error(transparent)]
    InvalidDecisionVariable(#[from] DecisionVariableError),

    /// The wire format is invalid.
    #[error("Cannot decode as a Protobuf Message: {0}")]
    DecodeError(#[from] DecodeError),
}

impl RawParseError {
    pub fn context(self, message: &'static str, field: &'static str) -> ParseError {
        ParseError {
            context: vec![ParseContext { message, field }],
            error: self,
        }
    }
}
