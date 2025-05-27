use super::*;

impl<M: Monomial> PolynomialBase<M> {
    pub fn new(terms: FnvHashMap<M, Coefficient>) -> Self {
        Self { terms }
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
        let mut terms = FnvHashMap::default();
        terms.insert(M::default(), c);
        Self { terms }
    }
}
