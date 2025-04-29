use crate::MonomialDyn;
use std::fmt;

fn write_f64_with_precision(f: &mut fmt::Formatter, coefficient: f64) -> fmt::Result {
    if let Some(precision) = f.precision() {
        write!(f, "{1:.0$}", precision, coefficient)?;
    } else {
        write!(f, "{}", coefficient)?;
    }
    Ok(())
}

fn write_term(f: &mut fmt::Formatter, ids: MonomialDyn, coefficient: f64) -> fmt::Result {
    if ids.is_empty() {
        write_f64_with_precision(f, coefficient)?;
        return Ok(());
    }
    if coefficient == -1.0 {
        write!(f, "-")?;
    } else if coefficient != 1.0 {
        write_f64_with_precision(f, coefficient)?;
    }
    if coefficient.abs() != 1.0 {
        write!(f, "*")?;
    }
    let mut ids = ids.iter().peekable();
    if let Some(id) = ids.next() {
        write!(f, "x{}", id)?;
    }
    for id in ids {
        write!(f, "*x{}", id)?;
    }
    Ok(())
}

pub fn format_polynomial(
    f: &mut fmt::Formatter,
    iter: impl Iterator<Item = (MonomialDyn, f64)>,
) -> fmt::Result {
    let mut terms: Vec<_> = iter
        .filter(|(_, coefficient)| coefficient.abs() > f64::EPSILON)
        .collect();
    if terms.is_empty() {
        write!(f, "0")?;
        return Ok(());
    }
    terms.sort_unstable_by(|(a, _), (b, _)| {
        if a.len() != b.len() {
            b.len().cmp(&a.len())
        } else {
            a.cmp(b)
        }
    });

    let mut iter = terms.into_iter();
    let (ids, coefficient) = iter.next().unwrap();
    write_term(f, ids, coefficient)?;

    for (ids, coefficient) in iter {
        if coefficient < 0.0 {
            write!(f, " - ")?;
            write_term(f, ids, -coefficient)?;
        } else {
            write!(f, " + ")?;
            write_term(f, ids, coefficient)?;
        }
    }
    Ok(())
}
