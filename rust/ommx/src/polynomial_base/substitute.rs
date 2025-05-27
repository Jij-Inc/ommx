use crate::{
    substitute::{ClassifiedAssignments, Substitute},
    Monomial, Polynomial, PolynomialBase,
};

impl<M> Substitute for PolynomialBase<M>
where
    M: Monomial,
{
    type Output = Polynomial;

    fn substitute_classified(
        &self,
        classified_assignments: &ClassifiedAssignments,
    ) -> Self::Output {
        todo!()
    }
}
