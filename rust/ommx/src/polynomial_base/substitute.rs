use crate::{
    substitute::Substitute, Coefficient, Evaluate, Function, Linear, LinearMonomial, Monomial,
    MonomialDyn, Polynomial, PolynomialBase, QuadraticMonomial, VariableID,
};
use num::One;

impl<M> Substitute for PolynomialBase<M>
where
    M: Monomial + Substitute,
    PolynomialBase<M>: Into<Function>,
{
    fn substitute_one(
        self,
        assigned: VariableID,
        f: &Function,
    ) -> Result<Function, crate::substitute::SubstitutionError> {
        // Check for self-assignment (x = x + ...)
        if f.required_ids().contains(&assigned) {
            return Err(crate::substitute::SubstitutionError::RecursiveAssignment {
                var_id: assigned,
            });
        }
        let mut substituted = Function::Zero;
        for (monomial, coefficient) in self.terms {
            substituted += coefficient * monomial.substitute_one(assigned, f)?;
        }
        Ok(substituted)
    }
}

impl Substitute for LinearMonomial {
    fn substitute_one(
        self,
        assigned: VariableID,
        f: &Function,
    ) -> Result<Function, crate::substitute::SubstitutionError> {
        // Check for self-assignment (x = x + ...)
        if f.required_ids().contains(&assigned) {
            return Err(crate::substitute::SubstitutionError::RecursiveAssignment {
                var_id: assigned,
            });
        }

        match self {
            LinearMonomial::Variable(id) => {
                if id == assigned {
                    Ok(f.clone())
                } else {
                    Ok(Linear::from(self).into())
                }
            }
            LinearMonomial::Constant => Ok(Function::Constant(Coefficient::one())),
        }
    }
}

impl Substitute for QuadraticMonomial {
    fn substitute_one(
        self,
        assigned: VariableID,
        f: &Function,
    ) -> Result<Function, crate::substitute::SubstitutionError> {
        // Check for self-assignment (x = x + ...)
        if f.required_ids().contains(&assigned) {
            return Err(crate::substitute::SubstitutionError::RecursiveAssignment {
                var_id: assigned,
            });
        }

        match self {
            QuadraticMonomial::Pair(pair) => {
                let l_sub = LinearMonomial::Variable(pair.lower()).substitute_one(assigned, f)?;
                let u_sub = LinearMonomial::Variable(pair.upper()).substitute_one(assigned, f)?;
                Ok(&l_sub * &u_sub)
            }
            QuadraticMonomial::Linear(id) => {
                let result = LinearMonomial::Variable(id).substitute_one(assigned, f)?;
                Ok(result)
            }
            QuadraticMonomial::Constant => Ok(Function::one()),
        }
    }
}

impl Substitute for MonomialDyn {
    fn substitute_one(
        self,
        assigned: VariableID,
        f: &Function,
    ) -> Result<Function, crate::substitute::SubstitutionError> {
        // Check for self-assignment (x = x + ...)
        if f.required_ids().contains(&assigned) {
            return Err(crate::substitute::SubstitutionError::RecursiveAssignment {
                var_id: assigned,
            });
        }

        let mut substituted = Function::one();
        let mut non_substituted = Vec::new();
        for var_id in self.iter() {
            if *var_id == assigned {
                substituted *= f;
            } else {
                non_substituted.push(*var_id);
            }
        }
        substituted *= Polynomial::from(MonomialDyn::from(non_substituted));
        Ok(substituted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assign, coeff, linear, AcyclicAssignments, QuadraticMonomial, VariableID, VariableIDSet,
    };
    use proptest::prelude::*;

    #[test]
    fn substitute_linear_to_linear() {
        // Poly: 2.0 * x0 + 1.0 (using improved syntax)
        let poly = coeff!(2.0) * linear!(0) + Linear::one();

        // Assignments: x0 <- 0.5 * x1 + 1.0
        let assignments = assign! {
            0 <- coeff!(0.5) * linear!(1) + Linear::one()
        };

        // 2.0 * (0.5 * x1 + 1.0) + 1.0 = x1 + 3.0
        let expected = Linear::from(linear!(1) + coeff!(3.0));

        let result = poly.substitute_acyclic(&assignments);
        assert_eq!(result, expected.into());
    }

    #[test]
    fn substitute_linear_to_quadratic() {
        // q = 2 * x0 * x1 (using improved syntax)
        let q = coeff!(2.0) * QuadraticMonomial::from((VariableID::from(0), VariableID::from(1)));

        // x0 = 2*x1 + 1
        let assignments = assign! {
            0 <- coeff!(2.0) * linear!(1) + Linear::one()
        };

        // 2 * (2 * x1 + 1) * x1 = 4 * x1^2 + 2 * x1
        let ans = coeff!(4.0) * QuadraticMonomial::from((VariableID::from(1), VariableID::from(1)))
            + coeff!(2.0) * QuadraticMonomial::from(VariableID::from(1));

        let result = q.substitute_acyclic(&assignments);
        assert_eq!(result, ans.into());
    }

    proptest! {
        #[test]
        fn removes_assigned_variables(
            f in Linear::arbitrary(),
            acyclic_assignments in AcyclicAssignments::arbitrary()
        ) {
            let original = f.required_ids();
            let assigned: VariableIDSet = acyclic_assignments.keys().collect();
            let substituted = f.substitute_acyclic(&acyclic_assignments);
            let result_vars = substituted.required_ids();
            prop_assert!(
                result_vars.is_disjoint(&assigned),
                "original={original:?}, assigned={assigned:?}, variables after substituted={result_vars:?}",
            );
        }
    }
}
