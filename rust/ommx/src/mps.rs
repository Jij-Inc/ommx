//! Parse MPS format
//!
//! ```no_run
//! # fn main() -> anyhow::Result<()> {
//! let instance: ommx::Instance = ommx::mps::load("data/directory/data.mps.gz")?;
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
//! - [CPLEX](https://www.ibm.com/docs/en/icos/22.1.1?topic=extensions-integer-variables-in-mps-files)
//! - [GUROBI](https://docs.gurobi.com/projects/optimizer/en/current/reference/fileformats/modelformats.html#formatmps)
//!

mod compressed;
mod convert;
mod format;
mod parser;
#[cfg(test)]
mod tests;

pub use compressed::is_gzipped;
pub use format::{format, to_string};

use parser::*;
use std::{io::Read, path::Path};

/// Reads and parses the MPS file from the given [`Read`] source with automatic gzipped detection.
#[tracing::instrument(skip_all)]
pub fn parse(reader: impl Read) -> crate::Result<crate::Instance> {
    let mps_data = Mps::parse(reader)?;
    convert::convert(mps_data)
}

/// Reads and parses the file at the given path. Gzipped files are automatically detected and decompressed.
//
// Note: the caller's path is intentionally not recorded as a span field to
// avoid leaking local directory structure through exported telemetry.
#[tracing::instrument(skip_all)]
pub fn load(path: impl AsRef<Path>) -> crate::Result<crate::Instance> {
    let mps_data = Mps::load(path)?;
    convert::convert(mps_data)
}

/// Writes out the instance as an MPS file to the specified path with compression control.
///
/// If `compress` is true, the output will be gzipped. If false, it will be written as plain text.
///
/// Limitation
/// ----------
/// Only linear problems are supported. See [`format()`] for detailed information about information loss,
/// removed constraints handling, and variable filtering behavior.
// Note: the caller's output path is intentionally not recorded as a span
// field to avoid leaking local directory structure through exported telemetry.
#[tracing::instrument(skip_all, fields(compress))]
pub fn save(
    instance: &crate::Instance,
    out_path: impl AsRef<Path>,
    compress: bool,
) -> crate::Result<()> {
    let path = std::path::absolute(out_path.as_ref())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::File::options()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;

    if compress {
        let mut writer = flate2::write::GzEncoder::new(file, flate2::Compression::new(5));
        format::format(instance, &mut writer)?;
    } else {
        format::format(instance, &mut file)?;
    }
    Ok(())
}
