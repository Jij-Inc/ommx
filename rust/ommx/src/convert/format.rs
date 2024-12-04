use std::fmt;

fn write_f64_with_precision(f: &mut fmt::Formatter, coefficient: f64) -> fmt::Result {
    if let Some(precision) = f.precision() {
        write!(f, "{1:.0$}", precision, coefficient)?;
    } else {
        write!(f, "{}", coefficient)?;
    }
    Ok(())
}

fn write_term(f: &mut fmt::Formatter, mut ids: Vec<u64>, coefficient: f64) -> fmt::Result {
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
    ids.sort_unstable();
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
    iter: impl Iterator<Item = (Vec<u64>, f64)>,
) -> fmt::Result {
    let mut terms = iter.peekable();
    for (ids, coefficient) in terms.by_ref() {
        if coefficient == 0.0 {
            continue;
        }
        write_term(f, ids, coefficient)?;
        break;
    }

    for (ids, coefficient) in terms {
        if coefficient == 0.0 {
            continue;
        }
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
