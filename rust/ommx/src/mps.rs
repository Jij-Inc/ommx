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

use crate::{Equality, InstanceClass, InstanceClassClause, Kind, PolynomialRequirement, Sense};
use parser::*;
use std::{collections::BTreeSet, io::Read, path::Path, sync::LazyLock};

/// Validate that `instance` belongs to the exact structural class supported by
/// the MPS writer before an output side effect starts.
fn preflight(instance: &crate::Instance) -> crate::Result<()> {
    static INPUT_CLASS: LazyLock<InstanceClass> = LazyLock::new(|| {
        let quadratic = PolynomialRequirement::at_most(2);
        InstanceClassClause::new(
            "MPS",
            BTreeSet::from([Kind::Binary, Kind::Integer, Kind::Continuous]),
            quadratic,
            BTreeSet::from([Sense::Minimize, Sense::Maximize]),
        )
        .with_regular_constraint(Equality::EqualToZero, quadratic)
        .with_regular_constraint(Equality::LessThanOrEqualToZero, quadratic)
        .into()
    });

    let report = INPUT_CLASS.check_membership(instance);
    crate::ensure!(
        report.is_member(),
        { report = %report },
        "Instance is outside the MPS input class:\n{report}",
    );
    Ok(())
}

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
/// Only the model family described by [`format()`] is supported. See it for
/// detailed information about required lowering, information loss, removed
/// constraints handling, and variable filtering behavior.
// Note: the caller's output path is intentionally not recorded as a span
// field to avoid leaking local directory structure through exported telemetry.
#[tracing::instrument(skip_all, fields(compress))]
pub fn save(
    instance: &crate::Instance,
    out_path: impl AsRef<Path>,
    compress: bool,
) -> crate::Result<()> {
    // Reject unsupported model content before creating directories or
    // truncating an existing destination. `format` repeats this guard for
    // callers that provide their own writer.
    preflight(instance)?;
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

#[cfg(test)]
mod save_tests {
    use super::*;
    use crate::{linear, DecisionVariable, Function, Instance, Sense, VariableID};
    use std::collections::BTreeMap;

    fn unsupported_instance() -> Instance {
        let id = VariableID::from(1);
        Instance::new(
            Sense::Minimize,
            Function::from(linear!(id)).abs(),
            BTreeMap::from([(id, DecisionVariable::continuous())]),
            BTreeMap::new(),
        )
        .unwrap()
    }

    #[test]
    fn unsupported_model_does_not_truncate_existing_destination() {
        let instance = unsupported_instance();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("existing.mps");
        std::fs::write(&path, b"existing content").unwrap();

        let error = save(&instance, &path, false).unwrap_err();

        assert!(error
            .to_string()
            .contains("objective function is not polynomial"));
        assert_eq!(std::fs::read(path).unwrap(), b"existing content");
    }

    #[test]
    fn unsupported_model_does_not_create_destination() {
        let instance = unsupported_instance();
        let directory = tempfile::tempdir().unwrap();

        for compress in [false, true] {
            let parent = directory.path().join(format!("missing-{compress}"));
            let path = parent.join("output.mps");

            save(&instance, &path, compress).unwrap_err();

            assert!(!parent.exists());
            assert!(!path.exists());
        }
    }
}
