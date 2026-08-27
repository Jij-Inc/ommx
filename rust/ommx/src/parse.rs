use crate::{DecisionVariable, VariableID};
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
/// let parse_error = error.downcast_ref::<ommx::ParseError>().unwrap();
/// let cause = std::error::Error::source(parse_error).unwrap();
/// assert!(cause.is::<ommx::RawParseError>());
/// ```
#[derive(Debug)]
pub struct ParseError {
    pub context: Vec<ParseContext>,
    /// The wire-format, domain-signal, or ordinary semantic error that caused
    /// parsing to fail.
    ///
    /// Public callers access this cause as `&dyn std::error::Error` through
    /// [`std::error::Error::source`].
    error: crate::Error,
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
        Some(self.error.as_ref())
    }
}

impl From<RawParseError> for ParseError {
    fn from(error: RawParseError) -> Self {
        Self::new(error)
    }
}

impl ParseError {
    /// Crate parsing modules use this constructor to attach their owning
    /// domain cause while this module retains the protobuf breadcrumb
    /// envelope.
    pub(crate) fn new(error: impl Into<crate::Error>) -> Self {
        ParseError {
            context: vec![],
            error: error.into(),
        }
    }

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

/// Generic failures owned by the protobuf parsing boundary.
///
/// Dedicated message-specific parse signals, domain validation signals, and
/// ordinary semantic failures are exposed directly through
/// [`ParseError`]'s [`std::error::Error::source`], rather than being wrapped in
/// this enum.
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

    /// Extension annotation maps must not carry OMMX-owned metadata.
    #[error(
        "Annotation key `{key}` is reserved for OMMX metadata and cannot be stored in extension annotations."
    )]
    ReservedAnnotationKey { key: String },

    /// The wire format is invalid.
    #[error("Cannot decode as a Protobuf Message: {0}")]
    DecodeError(#[from] DecodeError),
}

impl RawParseError {
    pub fn context(self, message: &'static str, field: &'static str) -> ParseError {
        ParseError::new(self).context(message, field)
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
        return Err(ParseError::new(crate::error!(
            "Undefined variable ID is used: {id:?}"
        )));
    }
    Ok(id)
}
