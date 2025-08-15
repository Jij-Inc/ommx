use crate::{
    polynomial_base::QuadraticParseError, BoundError, CoefficientError, Constraint, ConstraintID,
    DecisionVariable, DecisionVariableError, InstanceError, RemovedConstraint, SampleID,
    SampleSetError, SolutionError, SubstitutionError, VariableID,
};
use prost::DecodeError;
use std::{collections::BTreeMap, fmt};

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
#[non_exhaustive]
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

    /// When an integer value doesn't correspond to a known enum variant during deserialization.
    /// This includes cases where the value is unspecified (0) or a new variant added in a newer version.
    #[error("Unknown or unsupported enum value {value} for {enum_name}. This may be due to an unspecified value or a newer version of the protocol.")]
    UnknownEnumValue { enum_name: &'static str, value: i32 },

    #[error(transparent)]
    InstanceError(#[from] InstanceError),

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

pub(crate) fn as_constraint_id(
    constraints: &BTreeMap<ConstraintID, Constraint>,
    removed_constraints: &BTreeMap<ConstraintID, RemovedConstraint>,
    id: u64,
) -> Result<ConstraintID, ParseError> {
    let id = ConstraintID::from(id);
    if !constraints.contains_key(&id) && !removed_constraints.contains_key(&id) {
        return Err(
            RawParseError::InstanceError(InstanceError::UndefinedConstraintID { id }).into(),
        );
    }
    Ok(id)
}

pub(crate) fn as_variable_id(
    decision_variables: &BTreeMap<VariableID, DecisionVariable>,
    id: u64,
) -> Result<VariableID, ParseError> {
    let id = VariableID::from(id);
    if !decision_variables.contains_key(&id) {
        return Err(RawParseError::InstanceError(InstanceError::UndefinedVariableID { id }).into());
    }
    Ok(id)
}
