//! Parser for the QPLIB format

mod convert;
mod parser;

use crate::Instance;
pub use parser::QplibFile;
use std::path::Path;

/// Reads and parses the file into a [`Instance`].
pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Instance> {
    let data = QplibFile::load(path)?;
    let converted = convert::convert(data)?;
    Ok(converted.try_into()?)
}

#[derive(Debug, thiserror::Error)]
#[error("{reason} (at line {line_num})")]
pub struct QplibParseError {
    pub line_num: usize,
    pub reason: ParseErrorReason,
}

impl QplibParseError {
    // generic "invalid line" error
    fn invalid_line(line_num: usize) -> Self {
        Self {
            line_num,
            reason: ParseErrorReason::InvalidLine(line_num),
        }
    }

    fn unexpected_eof(line_num: usize) -> Self {
        Self {
            line_num,
            reason: ParseErrorReason::UnexpectedEndOfFile(line_num),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseErrorReason {
    #[error("Invalid problem type: {0}")]
    InvalidProblemType(String),
    #[error("Invalid OBJSENSE: {0}")]
    InvalidObjSense(String),
    #[error("Invalid variable type: {0}")]
    InvalidVarType(String),
    #[error("Unexpected end of file at line {0}")]
    UnexpectedEndOfFile(usize),
    #[error("Line {0} did not match expected formatting")]
    InvalidLine(usize),
    #[error(transparent)]
    ParseInt(#[from] std::num::ParseIntError),
    #[error(transparent)]
    ParseFloat(#[from] std::num::ParseFloatError),
}

impl ParseErrorReason {
    // This is a method to make it easier to add the line number at which an
    // error occurred in the qplib parser.
    pub(crate) fn with_line(self, line_num: usize) -> QplibParseError {
        QplibParseError {
            line_num,
            reason: self,
        }
    }
}

// Workaround to the fact that `String`'s `FromStr` impl has error
// type `Infallible`. As the conversion can't fail, by definition,
// this will never be called and no panic will ever happen
impl From<std::convert::Infallible> for ParseErrorReason {
    fn from(_: std::convert::Infallible) -> Self {
        unreachable!()
    }
}
