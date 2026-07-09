use crate::Bound;

pub(super) const MAX_EXACT_ENCODING_INTEGER: f64 = (1_u64 << 53) as f64;

pub(super) fn ensure_unit_spaced_integer_bound(
    integer_bound: Bound,
    encoding_name: &str,
) -> crate::Result<()> {
    if integer_bound.width() == 0.0 {
        return Ok(());
    }

    let lower = integer_bound.lower();
    let upper = integer_bound.upper();
    if lower < -MAX_EXACT_ENCODING_INTEGER || upper > MAX_EXACT_ENCODING_INTEGER {
        crate::bail!(
            { ?integer_bound, lower, upper, max_exact_integer = MAX_EXACT_ENCODING_INTEGER },
            "integer bound is too far from zero for {encoding_name}: non-point range [{lower}, {upper}] cannot be represented as unit-spaced f64 integers",
        );
    }
    Ok(())
}
