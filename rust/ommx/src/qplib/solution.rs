use super::QplibParseError;
use crate::{v1::State, Result};
use anyhow::Context;
use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Read},
    path::Path,
};

/// Load a published QPLIB `.sol` file into a [`State`].
///
/// `num_variables` is the variable count of the original QPLIB problem.
/// See [`parse_solution`] for the supported naming convention and zero filling.
/// This reads a state only; use [`Instance::evaluate`](crate::Instance::evaluate)
/// to compute the objective value and feasibility.
#[tracing::instrument(skip_all)]
pub fn load_solution(path: impl AsRef<Path>, num_variables: usize) -> Result<State> {
    let path = path.as_ref();
    let reader = File::open(path)
        .with_context(|| format!("Failed to read QPLIB solution {}", path.display()))?;
    parse_solution(reader, num_variables)
}

/// Parse a published QPLIB `.sol` file into a [`State`].
///
/// Each nonblank, noncomment line must contain a variable name and a finite
/// numeric value. The supported names are the standard GAMS names used in
/// QPLIB's published solutions: `xN`, `bN`, or `iN`, with `N >= 2`. Their
/// suffix includes the GAMS objective variable, so `N` maps to the zero-based
/// OMMX variable ID `N - 2`. Names are case insensitive. Custom variable names
/// and other solver `.sol` formats are not supported.
///
/// The returned state contains every ID in `0..num_variables`, with omitted
/// values filled with zero. The optional `objvar` entry is validated and
/// discarded: it is a reported objective value, not a decision variable.
/// Bounds, integrality, and feasibility are checked by evaluating the state
/// against its original QPLIB instance, not by this parser.
///
/// Blank lines and full-line comments starting with `#`, `!`, or `%` are
/// ignored. Decimal and `e`/`E`/`d`/`D` exponent notation are accepted.
/// Malformed lines, duplicate variable IDs (including aliases with different
/// prefixes), duplicate `objvar`, nonfinite values, and out-of-range IDs return
/// [`QplibParseError`]. Read failures preserve the underlying I/O error.
///
/// ```
/// use ommx::qplib::parse_solution;
/// let state = parse_solution(b"objvar 12\nx2 0.5\nb4 1\n".as_slice(), 3)?;
/// assert_eq!(state.entries[&0], 0.5);
/// assert_eq!(state.entries[&1], 0.0);
/// assert_eq!(state.entries[&2], 1.0);
/// # Ok::<(), ommx::Error>(())
/// ```
#[tracing::instrument(skip_all)]
pub fn parse_solution(reader: impl Read, num_variables: usize) -> Result<State> {
    let mut entries = HashMap::new();
    let mut seen_objective = false;
    for (index, line) in BufReader::new(reader).lines().enumerate() {
        let line_num = index + 1;
        let line =
            line.with_context(|| format!("Failed to read QPLIB solution line {line_num}"))?;
        let line = line.trim();
        if line.is_empty() || line.starts_with(['#', '!', '%']) {
            continue;
        }
        let error = |message: &str| QplibParseError::new(line_num, message);
        let mut fields = line.split_whitespace();
        let name = fields.next().expect("nonempty line").to_ascii_lowercase();
        let value = fields
            .next()
            .ok_or_else(|| error("expected a variable name and a numeric value"))?;
        if fields.next().is_some() {
            return Err(error("expected exactly a variable name and a numeric value").into());
        }
        let value: f64 = value.replace(['d', 'D'], "e").parse().map_err(|_| {
            QplibParseError::new(line_num, format!("invalid value {value:?} for {name:?}"))
        })?;
        if !value.is_finite() {
            return Err(error("solution values must be finite").into());
        }
        if name == "objvar" {
            if seen_objective {
                return Err(error("duplicate objvar entry").into());
            }
            seen_objective = true;
            continue;
        }
        let suffix = name
            .strip_prefix(['x', 'b', 'i'])
            .filter(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()));
        let id = suffix
            .and_then(|s| s.parse::<u64>().ok())
            .and_then(|n| n.checked_sub(2))
            .ok_or_else(|| {
                QplibParseError::new(
                    line_num,
                    format!("invalid variable name {name:?}: expected xN, bN, or iN with N >= 2"),
                )
            })?;
        if id >= num_variables as u64 {
            return Err(QplibParseError::new(
                line_num,
                format!("variable {name:?} refers to ID {id}, outside 0..{num_variables}"),
            )
            .into());
        }
        let value = if value == 0.0 { 0.0 } else { value };
        if entries.insert(id, value).is_some() {
            return Err(
                QplibParseError::new(line_num, format!("duplicate variable ID {id}")).into(),
            );
        }
    }
    for id in 0..num_variables as u64 {
        entries.entry(id).or_insert(0.0);
    }
    Ok(State { entries })
}
