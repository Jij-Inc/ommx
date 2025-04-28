use super::*;

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
