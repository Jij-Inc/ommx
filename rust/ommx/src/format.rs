use crate::{Monomial, MonomialDyn};
use std::fmt;

fn write_f64_with_precision(f: &mut fmt::Formatter, coefficient: f64) -> fmt::Result {
    if let Some(precision) = f.precision() {
        write!(f, "{coefficient:.precision$}")?;
    } else {
        write!(f, "{coefficient}")?;
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
        write!(f, "x{id}")?;
    }
    for id in ids {
        write!(f, "*x{id}")?;
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

impl<M: Monomial> fmt::Display for crate::PolynomialBase<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.num_terms() == 0 {
            return write!(f, "0");
        }
        format_polynomial(
            f,
            self.iter()
                .map(|(monomial, coefficient)| (monomial.clone().into(), coefficient.into_inner())),
        )
    }
}

impl<M: Monomial> fmt::Debug for crate::PolynomialBase<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl fmt::Display for crate::Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            crate::Function::Zero => write!(f, "0"),
            crate::Function::Constant(c) => write!(f, "{}", c.into_inner()),
            crate::Function::Linear(linear) => write!(f, "{linear}"),
            crate::Function::Quadratic(quadratic) => write!(f, "{quadratic}"),
            crate::Function::Polynomial(polynomial) => write!(f, "{polynomial}"),
        }
    }
}

impl fmt::Debug for crate::Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            crate::Function::Zero => write!(f, "Zero"),
            crate::Function::Constant(c) => write!(f, "Constant({})", c.into_inner()),
            crate::Function::Linear(linear) => write!(f, "Linear({linear})"),
            crate::Function::Quadratic(quadratic) => write!(f, "Quadratic({quadratic})"),
            crate::Function::Polynomial(polynomial) => write!(f, "Polynomial({polynomial})"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{coeff, linear, quadratic, Linear};

    #[test]
    fn test_polynomial_base_display_empty() {
        let poly: Linear = Linear::default();
        assert_eq!(format!("{poly}"), "0");
    }

    #[test]
    fn test_polynomial_base_display_single_term() {
        let poly = coeff!(3.0) * linear!(1);
        assert_eq!(format!("{poly}"), "3*x1");
    }

    #[test]
    fn test_polynomial_base_display_constant() {
        let poly = Linear::from(coeff!(5.0));
        assert_eq!(format!("{poly}"), "5");
    }

    #[test]
    fn test_polynomial_base_display_multiple_terms() {
        let poly = coeff!(2.0) * linear!(1) - coeff!(3.0) * linear!(2) + coeff!(1.0);

        let result = format!("{poly}");
        // Terms should be sorted by degree (highest first), then lexicographically
        assert_eq!(result, "2*x1 - 3*x2 + 1");
    }

    #[test]
    fn test_polynomial_base_display_quadratic() {
        let poly = coeff!(4.0) * quadratic!(1, 2) - coeff!(2.0) * quadratic!(1) + coeff!(3.0);

        let result = format!("{poly}");
        // Quadratic term should come first (highest degree), then linear, then constant
        assert_eq!(result, "4*x1*x2 - 2*x1 + 3");
    }

    #[test]
    fn test_polynomial_base_display_coefficient_one() {
        let poly: Linear = linear!(1).into();
        assert_eq!(format!("{poly}"), "x1");
    }

    #[test]
    fn test_polynomial_base_display_coefficient_negative_one() {
        let poly = coeff!(-1.0) * linear!(1);
        assert_eq!(format!("{poly}"), "-x1");
    }
}
