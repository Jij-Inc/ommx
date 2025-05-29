use crate::{
    substitute::SubstituteWithLinears, Evaluate, Linear, LinearMonomial, Monomial, MonomialDyn,
    Polynomial, PolynomialBase, Quadratic, QuadraticMonomial, VariableID,
};

impl<M> SubstituteWithLinears for PolynomialBase<M>
where
    M: Monomial + SubstituteWithLinears<Output = Self>,
{
    type Output = Self;

    fn substitute_with_linear(
        self,
        assigned: VariableID,
        linear: Linear,
    ) -> Result<Self::Output, crate::substitute::RecursiveAssignmentError> {
        // Check for self-assignment (x = x + ...)
        if linear.required_ids().contains(&assigned) {
            return Err(crate::substitute::RecursiveAssignmentError { var_id: assigned });
        }
        let mut substituted = Self::default();
        for (monomial, coefficient) in self.terms {
            substituted +=
                coefficient * monomial.substitute_with_linear(assigned, linear.clone())?;
        }
        Ok(substituted)
    }
}

impl SubstituteWithLinears for LinearMonomial {
    type Output = Linear;

    fn substitute_with_linear(
        self,
        assigned: VariableID,
        linear: Linear,
    ) -> Result<Self::Output, crate::substitute::RecursiveAssignmentError> {
        // Check for self-assignment (x = x + ...)
        if linear.required_ids().contains(&assigned) {
            return Err(crate::substitute::RecursiveAssignmentError { var_id: assigned });
        }

        match self {
            LinearMonomial::Variable(id) => {
                if id == assigned {
                    Ok(linear)
                } else {
                    Ok(Linear::from(self))
                }
            }
            LinearMonomial::Constant => Ok(Linear::one()),
        }
    }
}

impl SubstituteWithLinears for QuadraticMonomial {
    type Output = Quadratic;

    fn substitute_with_linear(
        self,
        assigned: VariableID,
        linear: Linear,
    ) -> Result<Self::Output, crate::substitute::RecursiveAssignmentError> {
        // Check for self-assignment (x = x + ...)
        if linear.required_ids().contains(&assigned) {
            return Err(crate::substitute::RecursiveAssignmentError { var_id: assigned });
        }

        match self {
            QuadraticMonomial::Pair(pair) => {
                let l_sub = LinearMonomial::Variable(pair.lower())
                    .substitute_with_linear(assigned, linear.clone())?;
                let u_sub = LinearMonomial::Variable(pair.upper())
                    .substitute_with_linear(assigned, linear)?;
                Ok(&l_sub * &u_sub)
            }
            QuadraticMonomial::Linear(id) => {
                let result =
                    LinearMonomial::Variable(id).substitute_with_linear(assigned, linear)?;
                Ok(result.into())
            }
            QuadraticMonomial::Constant => Ok(Quadratic::one()),
        }
    }
}

impl SubstituteWithLinears for MonomialDyn {
    type Output = Polynomial;

    fn substitute_with_linear(
        self,
        assigned: VariableID,
        linear: Linear,
    ) -> Result<Self::Output, crate::substitute::RecursiveAssignmentError> {
        // Check for self-assignment (x = x + ...)
        if linear.required_ids().contains(&assigned) {
            return Err(crate::substitute::RecursiveAssignmentError { var_id: assigned });
        }

        let mut substituted = Polynomial::one();
        let mut non_substituted = Vec::new();
        for var_id in self.iter() {
            if *var_id == assigned {
                substituted = &substituted * &linear;
            } else {
                non_substituted.push(*var_id);
            }
        }
        let non_substituted = Polynomial::from(MonomialDyn::from(non_substituted));
        Ok(&substituted * &non_substituted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AcyclicLinearAssignments, Coefficient, Evaluate, VariableID, VariableIDSet};
    use proptest::prelude::*;

    #[test]
    fn substitute_linear_to_linear() {
        // Poly: 2.0 * x0 + 1.0
        let poly = Linear::single_term(
            LinearMonomial::Variable(0.into()),
            Coefficient::try_from(2.0).unwrap(),
        ) + Linear::one();

        // Assignments: x0 = 0.5 * x1 + 1.0
        let assign_x0 = Linear::single_term(
            LinearMonomial::Variable(1.into()),
            Coefficient::try_from(0.5).unwrap(),
        ) + Linear::one();
        let assignments = vec![(0.into(), assign_x0)];

        // 2.0 * (0.5 * x1 + 1.0) + 1.0 = x1 + 3.0
        let expected = Linear::single_term(LinearMonomial::Variable(1.into()), Coefficient::one())
            + Linear::from(Coefficient::try_from(3.0).unwrap());

        let result = poly.substitute_with_linears(assignments).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn substitute_linear_to_quadratic() {
        // q = 2 * x0 * x1
        let q = Quadratic::single_term(
            (VariableID::from(0), VariableID::from(1)).into(),
            Coefficient::try_from(2.0).unwrap(),
        );

        // x0 = 2*x1 + 1
        let assign_x0 = Linear::single_term(
            LinearMonomial::Variable(1.into()),
            Coefficient::try_from(2.0).unwrap(),
        ) + Linear::one();
        let assignments = vec![(0.into(), assign_x0)];

        // 2 * (2 * x1 + 1) * x1 = 4 * x1^2 + 2 * x1
        let ans = Quadratic::single_term(
            (VariableID::from(1), VariableID::from(1)).into(),
            Coefficient::try_from(4.0).unwrap(),
        ) + Quadratic::single_term(
            VariableID::from(1).into(),
            Coefficient::try_from(2.0).unwrap(),
        );

        let result = q.substitute_with_linears(assignments).unwrap();
        assert_eq!(result, ans);
    }

    proptest! {
        #[test]
        fn removes_assigned_variables(
            f in Linear::arbitrary(),
            acyclic_assignments in AcyclicLinearAssignments::arbitrary()
        ) {
            let original = f.required_ids();
            let assigned: VariableIDSet = acyclic_assignments.keys().collect();
            let substituted = f.substitute_with_linears_acyclic(&acyclic_assignments);
            let result_vars = substituted.required_ids();
            prop_assert!(
                result_vars.is_disjoint(&assigned),
                "orignail={original:?}, assigned={assigned:?}, variables after substituted={result_vars:?}",
            );
        }
    }
}
