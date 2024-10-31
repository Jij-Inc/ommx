//! Parse MPS format
//!
//! ```no_run
//!
//! # fn main() -> Result<(), ommx::mps::MpsParseError> {
//! let mps = ommx::mps::load_file("data/directory/data.mps.gz")?;
//! # Ok(()) }
//! ```
//!
//! Differences from the original format
//! -------------------------------------
//! MPS format is very old format, and there are some differences between the original format and the actual data.
//! Some modification has been made to load the benchmark dataset in MIPLIB:
//!
//! - The original format is fixed format, but we parse it as space-separated format.
//! - `LI` as lower (negative) integer and `UI` as upper (positive) integer in `BOUNDS` section
//! - `PL` is treated as `FR` in the `BOUNDS` section.
//!
//! Original fixed format
//! ----------------------
//! ```text
//! │1 │2(5─12) ││3(15─22)││4(25─36)    │││5(40─47)││6(50─61)    │
//! ├──┼────────┼┼────────┼┼────────────┼┼┼────────┼┼────────────┤
//! NAME          TESTPROB                                         < MPS file starts
//! ROWS────────┬┬────────┬┬────────────┬┬┬────────┬┬────────────┤
//! │N │COST    ││        ││            │││        ││            │
//! │L │LIM1    ││        ││            │││        ││            │
//! │G │LIM2    ││        ││            │││        ││            │
//! │E │MYEQN   ││        ││            │││        ││            │
//! COLUMNS─────┼┼────────┼┼────────────┼┼┼────────┼┼────────────┤
//! │  │XONE    ││COST    ││           1│││LIM1    ││           1│
//! │  │XONE    ││LIM2    ││           1│││        ││            │
//! │  │YTWO    ││COST    ││           4│││LIM1    ││           1│
//! │  │YTWO    ││MYEQN   ││          ─1│││        ││            │
//! │  │ZTHREE  ││COST    ││           9│││LIM2    ││           1│
//! │  │ZTHREE  ││MYEQN   ││           1│││        ││            │
//! RHS─────────┼┼────────┼┼────────────┼┼┼────────┼┼────────────┤
//! │  │RHS1    ││LIM1    ││           5│││LIM2    ││          10│
//! │  │RHS1    ││MYEQN   ││           7│││        ││            │
//! BOUNDS──────┼┼────────┼┼────────────┼┼┼────────┼┼────────────┤
//! │UP│BND1    ││XONE    ││           4│││        ││            │
//! │LO│BND1    ││YTWO    ││          ─1│││        ││            │
//! │UP│BND1    ││YTWO    ││           1│││        ││            │
//! ENDATA──────┴┴────────┴┴────────────┴┴┴────────┴┴────────────┘
//! ```
//!
//! Links
//! ------
//! - <https://plato.asu.edu/ftp/mps_format.txt>
//! - [MPS (format) -- Wikipedia](https://en.wikipedia.org/wiki/MPS_(format))
//!

use prost::Message;
use std::path::Path;

mod convert;
mod parser;
mod to_mps;

use parser::*;

pub fn load_file(path: impl AsRef<Path>) -> Result<crate::v1::Instance, MpsParseError> {
    let mps_data = Mps::from_file(path)?;
    convert::convert(mps_data)
}

pub fn load_file_bytes(path: impl AsRef<Path>) -> Result<Vec<u8>, MpsParseError> {
    let instance = load_file(path)?;
    Ok(instance.encode_to_vec())
}

pub fn write_file(
    instance: &crate::v1::Instance,
    out_path: impl AsRef<Path>,
) -> Result<(), MpsWriteError> {
    let path = std::path::absolute(out_path.as_ref())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::options()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;

    let mut writer = std::io::BufWriter::new(file);
    to_mps::write_mps(instance, &mut writer)?;
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum MpsParseError {
    #[error("Unknown row name: {0}")]
    UnknownRowName(String),

    #[error("Invalid row type: {0}")]
    InvalidRowType(String),

    #[error("Invalid bound type: {0}")]
    InvalidBoundType(String),

    #[error("Invalid header line: {0}")]
    InvalidHeader(String),

    #[error("Invalid marker in COLUMN section: {0}")]
    InvalidMarker(String),

    #[error("Invalid OBJSENSE: {0}")]
    InvalidObjSense(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    ParseFloat(#[from] std::num::ParseFloatError),
}

#[derive(Debug, thiserror::Error)]
pub enum MpsWriteError {
    #[error(
        "Unsupported equation: Constraint {0} was {1}, but only linear functions are supported"
    )]
    InvalidConstraintType(String, String),
    #[error(
        "Unsupported equation: Objective function was {1}, but only linear functions are supported"
    )]
    InvalidObjectiveType(String, String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
