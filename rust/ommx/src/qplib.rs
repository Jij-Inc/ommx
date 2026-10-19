//! Parser for the QPLIB format

mod convert;
mod parser;
mod solution;

use crate::Instance;
pub use parser::QplibFile;
pub use solution::{load_solution, parse_solution};
use std::{fmt::Display, io::Read, path::Path};

/// Failure to parse a QPLIB problem or solution file at a known line number.
///
/// Carries the 1-based `line_num` and a message describing the invalid input.
/// Callers can recover the position programmatically by downcasting the error
/// returned by [`load`], [`parse`], [`load_solution`], or [`parse_solution`].
/// Every parser-created error also emits a structured `tracing::error!` event
/// with `line_num` and `message` fields.
#[derive(Debug, thiserror::Error)]
#[error("QPLIB parse error at line {line_num}: {message}")]
pub struct QplibParseError {
    pub line_num: usize,
    pub message: String,
}

impl QplibParseError {
    fn new(line_num: usize, cause: impl Display) -> Self {
        let message = cause.to_string();
        tracing::error!(line_num, %message, "QPLIB parse error");
        Self { line_num, message }
    }
}

/// Reads and parses the file into a [`Instance`].
//
// Note: the caller's path is intentionally not recorded as a span field to
// avoid leaking local directory structure through exported telemetry.
#[tracing::instrument(skip_all)]
pub fn load(path: impl AsRef<Path>) -> crate::Result<Instance> {
    let data = QplibFile::load(path)?;
    let converted = convert::convert(data);
    Ok(converted.try_into()?)
}

/// Parses QPLIB data from a reader into a [`Instance`].
#[tracing::instrument(skip_all)]
pub fn parse(reader: impl Read) -> crate::Result<Instance> {
    let data = QplibFile::parse(reader)?;
    let converted = convert::convert(data);
    Ok(converted.try_into()?)
}
