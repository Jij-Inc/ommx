use super::*;
use crate::{CoefficientError, Function};

impl<M: Monomial> PolynomialBase<M> {
    pub fn new(terms: FnvHashMap<M, Coefficient>) -> Self {
        Self { terms }
    }

    pub fn try_from_terms(
        iter: impl IntoIterator<Item = (M, Coefficient)>,
    ) -> Result<Self, CoefficientError> {
        let mut polynomial = Self::default();
        for (term, coefficient) in iter {
            polynomial.add_term(term, coefficient)?;
        }
        Ok(polynomial)
    }

    pub fn single_term(term: M, coefficient: Coefficient) -> Self {
        let mut terms = FnvHashMap::default();
        terms.insert(term, coefficient);
        Self { terms }
    }
}

impl<M1: Monomial, M2: Monomial> TryFrom<&PolynomialBase<M1>> for PolynomialBase<M2>
where
    M2: for<'a> TryFrom<&'a M1, Error = InvalidDegreeError>,
{
    type Error = InvalidDegreeError;
    fn try_from(q: &PolynomialBase<M1>) -> std::result::Result<Self, InvalidDegreeError> {
        Ok(Self {
            terms: q
                .terms
                .iter()
                .map(|(k, v)| Ok((k.try_into()?, *v)))
                .collect::<Result<_, InvalidDegreeError>>()?,
        })
    }
}

impl<M: Monomial> IntoIterator for PolynomialBase<M> {
    type Item = (M, Coefficient);
    type IntoIter = std::collections::hash_map::IntoIter<M, Coefficient>;
    fn into_iter(self) -> Self::IntoIter {
        self.terms.into_iter()
    }
}

impl<'a, M: Monomial> IntoIterator for &'a PolynomialBase<M> {
    type Item = (&'a M, &'a Coefficient);
    type IntoIter = std::collections::hash_map::Iter<'a, M, Coefficient>;
    fn into_iter(self) -> Self::IntoIter {
        self.terms.iter()
    }
}

impl<M: Monomial> From<Coefficient> for PolynomialBase<M> {
    fn from(c: Coefficient) -> Self {
        let mut terms = FnvHashMap::default();
        terms.insert(M::default(), c);
        Self { terms }
    }
}

impl From<LinearMonomial> for Function {
    fn from(value: LinearMonomial) -> Self {
        let p = PolynomialBase::<LinearMonomial>::from(value);
        Function::from(p)
    }
}

impl From<QuadraticMonomial> for Function {
    fn from(value: QuadraticMonomial) -> Self {
        let p = PolynomialBase::<QuadraticMonomial>::from(value);
        Function::from(p)
    }
}

impl From<MonomialDyn> for Function {
    fn from(value: MonomialDyn) -> Self {
        let p = PolynomialBase::<MonomialDyn>::from(value);
        Function::from(p)
    }
}

impl<M: Monomial> TryFrom<f64> for PolynomialBase<M> {
    type Error = CoefficientError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        match Coefficient::try_from(value) {
            Ok(coefficient) => Ok(Self::from(coefficient)),
            Err(CoefficientError::Zero) => Ok(Self::default()),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, linear, Linear};

    #[test]
    fn try_from_terms_combines_duplicate_terms() {
        let polynomial =
            Linear::try_from_terms([(linear!(1), coeff!(2.0)), (linear!(1), coeff!(3.0))]).unwrap();

        assert_eq!(polynomial.terms[&linear!(1)], coeff!(5.0));
    }

    #[test]
    fn try_from_terms_removes_cancelled_terms() {
        let polynomial =
            Linear::try_from_terms([(linear!(1), coeff!(1.0)), (linear!(1), coeff!(-1.0))])
                .unwrap();

        assert!(polynomial.is_zero());
    }

    #[test]
    fn try_from_terms_returns_error_on_overflow() {
        let huge = Coefficient::try_from(f64::MAX).unwrap();
        let err = Linear::try_from_terms([(linear!(1), huge), (linear!(1), huge)]).unwrap_err();

        assert_eq!(err, CoefficientError::Infinite);
    }
}
