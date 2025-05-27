use crate::{
    substitute::{LinearAssignments, SubstituteWithLinears},
    Linear, LinearMonomial, Monomial, MonomialDyn, Polynomial, PolynomialBase, Quadratic,
    QuadraticMonomial,
};

impl<M> SubstituteWithLinears for PolynomialBase<M>
where
    M: Monomial + SubstituteWithLinears<Output = Self>,
{
    type Output = Self;
    fn substitute_with_linears(&self, linear_assignments: &LinearAssignments) -> Self::Output {
        let mut substituted = Self::one();
        for (monomial, coefficient) in self.terms.iter() {
            let sub_monomial = monomial.substitute_with_linears(linear_assignments);
            for (m, c) in sub_monomial.terms {
                substituted.add_term(m, c * *coefficient);
            }
        }
        substituted
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

impl SubstituteWithLinears for MonomialDyn {
    type Output = Polynomial;
    fn substitute_with_linears(&self, linear_assignments: &LinearAssignments) -> Self::Output {
        let mut substituted = Polynomial::one();
        let mut non_substituted = Vec::new();
        for var_id in self.iter() {
            if let Some(linear_func) = linear_assignments.get(var_id) {
                substituted = &substituted * linear_func;
            } else {
                non_substituted.push(*var_id);
            }
        }
        let non_substituted = Polynomial::from(MonomialDyn::from(non_substituted));
        &substituted * &non_substituted
    }
}
