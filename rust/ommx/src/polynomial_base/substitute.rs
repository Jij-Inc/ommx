use crate::{
    substitute::{ClassifiedAssignments, LinearAssignments, Substitute, SubstituteWithLinears},
    Linear, LinearMonomial, Monomial, Polynomial, PolynomialBase, Quadratic, QuadraticMonomial,
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

impl SubstituteWithLinears for LinearMonomial {
    type Output = Linear;
    fn substitute_with_linears(&self, linear_assignments: &LinearAssignments) -> Self::Output {
        match self {
            LinearMonomial::Variable(id) => {
                if let Some(linear_func) = linear_assignments.get(id) {
                    linear_func.clone()
                } else {
                    Linear::from(*self)
                }
            }
            LinearMonomial::Constant => Linear::one(),
        }
    }
}

impl SubstituteWithLinears for QuadraticMonomial {
    type Output = Quadratic;

    fn substitute_with_linears(&self, linear_assignments: &LinearAssignments) -> Self::Output {
        match self {
            QuadraticMonomial::Pair(pair) => {
                let l_sub = LinearMonomial::Variable(pair.lower())
                    .substitute_with_linears(linear_assignments);
                let u_sub = LinearMonomial::Variable(pair.upper())
                    .substitute_with_linears(linear_assignments);
                (&l_sub * &u_sub).into()
            }
            QuadraticMonomial::Linear(id) => LinearMonomial::Variable(*id)
                .substitute_with_linears(linear_assignments)
                .into(),
            QuadraticMonomial::Constant => Quadratic::one(),
        }
    }
}
