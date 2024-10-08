//! Parse MPS format
//!
//! ```no_run
//!
//! # fn main() -> Result<(), ommx::mps::MpsParseError> {
//! let mps = ommx::mps::load_mps_file("data/directory/data.mps.gz")?;
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

use std::path::Path;

mod convert;
mod parser;

use parser::*;

pub fn load_mps_file(path: impl AsRef<Path>) -> Result<crate::v1::Instance, MpsParseError> {
    let mps_data = Mps::from_file(path)?;
    convert::convert(mps_data)
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
