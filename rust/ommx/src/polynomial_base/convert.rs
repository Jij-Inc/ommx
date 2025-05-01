use super::*;

impl<M1: Monomial, M2: Monomial> TryFrom<&PolynomialBase<M1>> for PolynomialBase<M2>
where
    M2: for<'a> TryFrom<&'a M1, Error = MonomialDowngradeError>,
{
    type Error = MonomialDowngradeError;
    fn try_from(q: &PolynomialBase<M1>) -> std::result::Result<Self, MonomialDowngradeError> {
        Ok(Self {
            terms: q
                .terms
                .iter()
                .map(|(k, v)| Ok((k.try_into()?, *v)))
                .collect::<Result<_, MonomialDowngradeError>>()?,
        })
    }
}

impl<M: Monomial> FromIterator<(M, Coefficient)> for PolynomialBase<M> {
    fn from_iter<I: IntoIterator<Item = (M, Coefficient)>>(iter: I) -> Self {
        let mut polynomial = Self::default();
        for (term, coefficient) in iter {
            polynomial.add_term(term, coefficient);
        }
        polynomial
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
        Self {
            terms: HashMap::from([(M::default(), c)]),
        }
    }
}
