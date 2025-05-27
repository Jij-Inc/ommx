use crate::{
    substitute::{ClassifiedAssignments, Substitute},
    Linear, Monomial, Polynomial, PolynomialBase, VariableID,
};
use fnv::FnvHashMap;

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

    fn substitute_with_linears(&self, linear_assignments: &FnvHashMap<VariableID, Linear>) -> Self {
        todo!()
    }
}
