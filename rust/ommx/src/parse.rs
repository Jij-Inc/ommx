use crate::{
    polynomial_base::QuadraticParseError, BoundError, CoefficientError, DecisionVariable,
    DecisionVariableError, SampleID, SampleSetError, SolutionError, SubstitutionError, VariableID,
};
use prost::DecodeError;
use std::{
    collections::{BTreeMap, HashMap},
    fmt,
};

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

/// OMMX-owned signal for protobuf wire decoding and semantic message parsing.
///
/// Public SDK byte decoders keep returning [`crate::Result`], but preserve
/// this type at the top of the error chain so callers can downcast without
/// depending on the protobuf implementation:
///
/// ```rust
/// let error = ommx::Instance::from_v1_bytes(&[0x80]).unwrap_err();
/// assert!(error.downcast_ref::<ommx::ParseError>().is_some());
/// ```
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

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

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
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum RawParseError {
    /// Incompatibility due to `oneof` in protobuf. See [`crate::Function`] for more information.
    #[error(
        "Unsupported ommx.v1.Function is found. It is created by a newer version of OMMX SDK."
    )]
    UnsupportedV1Function,

    /// The message's `format_version` exceeds what this SDK supports.
    /// The data was produced by a newer SDK whose format is not backward compatible with this one.
    #[error(
        "Unsupported ommx format version: data has format_version={data_version}, but this SDK supports up to {current_version}. Please upgrade the OMMX SDK."
    )]
    UnsupportedFormatVersion {
        data_version: u32,
        current_version: u32,
    },

    /// In proto3, all fields of message types are implicitly optional even if explicit `optional` flag is absent.
    /// When the SDK requires a field to be present, it will return this error.
    #[error("Field {field} in {message} is missing.")]
    MissingField {
        message: &'static str,
        field: &'static str,
    },

    /// When an integer value doesn't correspond to a known enum variant during deserialization.
    /// This includes cases where the value is unspecified (0) or a new variant added in a newer version.
    #[error("Unknown or unsupported enum value {value} for {enum_name}. This may be due to an unspecified value or a newer version of the protocol.")]
    UnknownEnumValue { enum_name: &'static str, value: i32 },

    /// Catch-all for [`crate::Instance`] invariant violations discovered during
    /// parsing (duplicated / undefined / non-unique IDs, etc.). Holds the
    /// rendered message; we no longer expose a typed enum for this case since
    /// downstream code never matched on discriminants.
    #[error("{0}")]
    InvalidInstance(String),

    /// Extension annotation maps must not carry OMMX-owned metadata.
    #[error(
        "Annotation key `{key}` is reserved for OMMX metadata and cannot be stored in extension annotations."
    )]
    ReservedAnnotationKey { key: String },

    #[error(transparent)]
    SolutionError(#[from] SolutionError),

    #[error(transparent)]
    SampleSetError(#[from] SampleSetError),

    #[error(transparent)]
    SubstitutionError(#[from] SubstitutionError),

    #[error(transparent)]
    InvalidCoefficient(#[from] CoefficientError),

    #[error(transparent)]
    QuadraticParseError(#[from] QuadraticParseError),

    #[error(transparent)]
    InvalidBound(#[from] BoundError),

    #[error(transparent)]
    InvalidDecisionVariable(#[from] DecisionVariableError),

    #[error("Duplicated sample ID: {id:?}")]
    DuplicatedSampleID { id: SampleID },

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

/// Validate that a message's `format_version` does not exceed what this SDK accepts.
pub(crate) fn check_format_version(
    format_version: u32,
    message: &'static str,
) -> Result<(), ParseError> {
    if format_version > crate::CURRENT_FORMAT_VERSION {
        return Err(RawParseError::UnsupportedFormatVersion {
            data_version: format_version,
            current_version: crate::CURRENT_FORMAT_VERSION,
        }
        .context(message, "format_version"));
    }
    Ok(())
}

/// Crate-internal parse paths use this to preserve the domain invariant that
/// extension annotations never contain OMMX-reserved metadata keys.
pub(crate) fn validate_extension_annotations(
    annotations: &HashMap<String, String>,
    message: &'static str,
) -> Result<(), ParseError> {
    for key in annotations.keys() {
        if crate::is_reserved_annotation_key(key) {
            return Err(RawParseError::ReservedAnnotationKey { key: key.clone() }
                .context(message, "annotations"));
        }
    }
    Ok(())
}

pub(crate) fn as_variable_id(
    decision_variables: &BTreeMap<VariableID, DecisionVariable>,
    id: u64,
) -> Result<VariableID, ParseError> {
    let id = VariableID::from(id);
    if !decision_variables.contains_key(&id) {
        return Err(RawParseError::InvalidInstance(format!(
            "Undefined variable ID is used: {id:?}"
        ))
        .into());
    }
    Ok(id)
}
