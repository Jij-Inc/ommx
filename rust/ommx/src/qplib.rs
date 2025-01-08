use prost::Message;
use std::path::Path;

mod convert;
mod parser;

use parser::QplibFile;

/// Reads and parses the file at the given path as a gzipped MPS file.
pub fn load_file(path: impl AsRef<Path>) -> Result<crate::v1::Instance, QplibParseError> {
    let data = QplibFile::from_file(path)?;
    convert::convert(data)
}

pub fn load_file_bytes(path: impl AsRef<Path>) -> Result<Vec<u8>, QplibParseError> {
    let instance = load_file(path)?;
    Ok(instance.encode_to_vec())
}

#[derive(Debug, thiserror::Error)]
pub enum QplibParseError {
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
    #[error(transparent)]
    Io(#[from] std::io::Error),

    // a little hack to allow us to add the line numbers
    // as context to errors generated in `FromStr` impls
    #[error("{inner} (at line {line_num})")]
    WithLine {
        line_num: usize,
        inner: Box<QplibParseError>,
    },
}

impl QplibParseError {
    // This is a method to make it easier to add the line number at which an
    // error occurred in the qplib parser.
    pub(crate) fn with_line(self, line_num: usize) -> QplibParseError {
        use QplibParseError::*;
        match self {
            e @ UnexpectedEndOfFile(_) => e,
            e @ InvalidLine(_) => e,
            WithLine { inner, .. } => WithLine { line_num, inner },
            e => WithLine {
                line_num,
                inner: Box::new(e),
            },
        }
    }
}

// Workaround to the fact that `String`'s `FromStr` impl has error
// type `Infallible`. As the conversion can't fail, by definition,
// this will never be called and no panic will ever happen
impl From<std::convert::Infallible> for QplibParseError {
    fn from(_: std::convert::Infallible) -> Self {
        unreachable!()
    }
}
