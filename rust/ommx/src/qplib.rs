//! Parser for the QPLIB format

mod convert;
mod parser;

use crate::Instance;
pub use parser::QplibFile;
use std::{io::Read, path::Path};

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
