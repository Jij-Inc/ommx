use crate::{
    check_self_assignment, substitute::Substitute, Coefficient, Function, Linear, LinearMonomial,
    Monomial, MonomialDyn, Polynomial, PolynomialBase, QuadraticMonomial, VariableID,
};
use crate::{substitute::ResolvedAssignments, Evaluate};

fn substitute_monomial(
    ids: impl Iterator<Item = VariableID>,
    resolved: &ResolvedAssignments<'_>,
) -> Result<Function, crate::SubstitutionError> {
    let mut term = Function::one();
    let mut unchanged = Vec::new();
    for id in ids {
        if let Some(function) = resolved.get(&id) {
            term.try_mul_assign_in_place(function)?;
        } else {
            unchanged.push(id);
        }
    }
    let unchanged = match unchanged.as_slice() {
        [] => return Ok(term),
        [id] => Function::from(crate::linear!(*id)),
        [left, right] => Function::from(crate::quadratic!(*left, *right)),
        _ => Function::from(Polynomial::from(MonomialDyn::from(unchanged))),
    };
    term.try_mul_assign_in_place(&unchanged)?;
    Ok(term)
}

impl<M> PolynomialBase<M>
where
    M: Monomial,
    PolynomialBase<M>: Into<Function>,
{
    /// Function owns dependency resolution; each compact polynomial then visits
    /// its terms once and looks up only the variables present in each monomial.
    pub(crate) fn substitute_resolved(
        self,
        resolved: &ResolvedAssignments<'_>,
    ) -> Result<Function, crate::SubstitutionError> {
        let mut output = Function::Zero;
        for (monomial, coefficient) in self.terms {
            let mut term = substitute_monomial(monomial.ids(), resolved)?;
            term.try_scale_assign_in_place(coefficient)?;
            output.try_add_assign_in_place(term)?;
        }
        Ok(output.normalize())
    }
}

impl<M> Substitute for PolynomialBase<M>
where
    M: Monomial + Substitute<Output = Function>,
    PolynomialBase<M>: Into<Function>,
{
    type Output = Function;

    fn substitute_acyclic(
        self,
        acyclic: &crate::AcyclicAssignments,
    ) -> Result<Self::Output, crate::SubstitutionError> {
        if acyclic.is_empty() {
            return Ok(self.into());
        }
        let resolved = acyclic.resolve_for(&self.required_ids())?;
        self.substitute_resolved(&resolved)
    }

    fn substitute_one(
        self,
        assigned: VariableID,
        f: &Function,
    ) -> Result<Function, crate::substitute::SubstitutionError> {
        check_self_assignment(assigned, f)?;
        let mut substituted = Function::Zero;
        for (monomial, coefficient) in self.terms {
            let mut term = monomial.substitute_one(assigned, f)?;
            term.try_scale_assign_in_place(coefficient)?;
            substituted.try_add_assign_in_place(term)?;
        }
        Ok(substituted.normalize())
    }
}

impl Substitute for LinearMonomial {
    type Output = Function;

    fn substitute_acyclic(
        self,
        acyclic: &crate::AcyclicAssignments,
    ) -> Result<Self::Output, crate::SubstitutionError> {
        let resolved = acyclic.resolve_for(&self.ids().collect())?;
        substitute_monomial(self.ids(), &resolved)
    }

    fn substitute_one(
        self,
        assigned: VariableID,
        f: &Function,
    ) -> Result<Function, crate::substitute::SubstitutionError> {
        check_self_assignment(assigned, f)?;
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
    type Output = Function;

    fn substitute_acyclic(
        self,
        acyclic: &crate::AcyclicAssignments,
    ) -> Result<Self::Output, crate::SubstitutionError> {
        let resolved = acyclic.resolve_for(&self.ids().collect())?;
        substitute_monomial(self.ids(), &resolved)
    }

    fn substitute_one(
        self,
        assigned: VariableID,
        f: &Function,
    ) -> Result<Function, crate::substitute::SubstitutionError> {
        check_self_assignment(assigned, f)?;
        match self {
            QuadraticMonomial::Pair(pair) => {
                let l_sub = LinearMonomial::Variable(pair.lower()).substitute_one(assigned, f)?;
                let u_sub = LinearMonomial::Variable(pair.upper()).substitute_one(assigned, f)?;
                Ok((&l_sub * &u_sub)?)
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
    type Output = Function;

    fn substitute_acyclic(
        self,
        acyclic: &crate::AcyclicAssignments,
    ) -> Result<Self::Output, crate::SubstitutionError> {
        let resolved = acyclic.resolve_for(&self.ids().collect())?;
        substitute_monomial(self.ids(), &resolved)
    }

    fn substitute_one(
        self,
        assigned: VariableID,
        f: &Function,
    ) -> Result<Function, crate::substitute::SubstitutionError> {
        check_self_assignment(assigned, f)?;
        let mut substituted = Function::one();
        let mut non_substituted = Vec::new();
        for var_id in self.iter() {
            if *var_id == assigned {
                substituted.try_mul_assign_in_place(f)?;
            } else {
                non_substituted.push(*var_id);
            }
        }
        substituted.try_mul_polynomial_assign_in_place(Polynomial::from(MonomialDyn::from(
            non_substituted,
        )))?;
        Ok(substituted.normalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assign, coeff, linear, ATol, AcyclicAssignments, Evaluate, QuadraticMonomial, VariableID,
        VariableIDSet,
    };
    use ::approx::assert_abs_diff_eq;
    use proptest::prelude::*;

    #[test]
    fn batch_substitution_resolves_chained_and_repeated_variables() {
        let function = Function::from((crate::quadratic!(0, 0) + crate::quadratic!(0, 1)).unwrap());
        let assignments = AcyclicAssignments::new([
            (
                0.into(),
                Function::from((linear!(1) + coeff!(1.0)).unwrap()),
            ),
            (
                1.into(),
                Function::from((coeff!(2.0) * linear!(2)).unwrap()),
            ),
            (
                2.into(),
                Function::from((linear!(3) + coeff!(-1.0)).unwrap()),
            ),
        ])
        .unwrap();
        let sequential: Function =
            crate::substitute_acyclic_via_one(function.clone(), &assignments).unwrap();
        let batch = function.substitute_acyclic(&assignments).unwrap();
        assert_abs_diff_eq!(batch, sequential);
        assert_eq!(batch.required_ids(), [VariableID::from(3)].into());
        assert_eq!(
            batch
                .evaluate(&crate::v1::State::from_iter([(3, 3.0)]), ATol::default())
                .unwrap(),
            45.0,
        );
    }

    proptest! {
        #[test]
        fn batch_substitution_preserves_polynomial_evaluation(
            terms in prop::collection::vec((prop::collection::vec(0_u64..8, 0..5), -4_i32..5), 1..32),
            values in prop::collection::vec(0_u32..7, 24)
        ) {
            let mut polynomial = Polynomial::default();
            for (ids, value) in terms {
                if value != 0 {
                    polynomial.add_term(MonomialDyn::from(ids.into_iter().map(VariableID::from).collect::<Vec<_>>()), Coefficient::try_from(value as f64).unwrap()).unwrap();
                }
            }
            let assignments = AcyclicAssignments::new((0_u64..8).map(|id| {
                let function = ((linear!(id + 8) + (coeff!(2.0) * linear!(id + 16)).unwrap()).unwrap() + coeff!(1.0)).unwrap();
                (id.into(), Function::from(function))
            })).unwrap();
            let mut state = crate::v1::State::from_iter((8_u64..24).map(|id| (id, values[id as usize] as f64)));
            for id in 0_u64..8 {
                state.entries.insert(id, values[id as usize + 8] as f64 + 2.0 * values[id as usize + 16] as f64 + 1.0);
            }
            let expected = polynomial.evaluate(&state, ATol::default()).unwrap();
            let function = Function::from(polynomial);
            let sequential: Function = crate::substitute_acyclic_via_one(function.clone(), &assignments).unwrap();
            let batch = function.substitute_acyclic(&assignments).unwrap();
            prop_assert_eq!(batch.evaluate(&state, ATol::default()).unwrap(), expected);
            prop_assert_eq!(batch.evaluate(&state, ATol::default()).unwrap(), sequential.evaluate(&state, ATol::default()).unwrap());
        }
    }

    #[test]
    fn substitute_linear_to_linear() {
        // Poly: 2.0 * x0 + 1.0 (using improved syntax)
        let poly = ((coeff!(2.0) * linear!(0)).unwrap() + Linear::one()).unwrap();

        // Assignments: x0 <- 0.5 * x1 + 1.0
        let assignments = assign! {
            0 <- ((coeff!(0.5) * linear!(1)).unwrap() + Linear::one()).unwrap()
        };

        // 2.0 * (0.5 * x1 + 1.0) + 1.0 = x1 + 3.0
        let expected = (linear!(1) + coeff!(3.0)).unwrap();

        let result = poly.substitute_acyclic(&assignments).unwrap();
        assert_abs_diff_eq!(result, expected.into());
    }

    #[test]
    fn substitute_linear_to_quadratic() {
        // q = 2 * x0 * x1 (using improved syntax)
        let q = (coeff!(2.0) * QuadraticMonomial::from((VariableID::from(0), VariableID::from(1))))
            .unwrap();

        // x0 = 2*x1 + 1
        let assignments = assign! {
            0 <- ((coeff!(2.0) * linear!(1)).unwrap() + Linear::one()).unwrap()
        };

        // 2 * (2 * x1 + 1) * x1 = 4 * x1^2 + 2 * x1
        let ans = ((coeff!(4.0)
            * QuadraticMonomial::from((VariableID::from(1), VariableID::from(1))))
        .unwrap()
            + (coeff!(2.0) * QuadraticMonomial::from(VariableID::from(1))).unwrap())
        .unwrap();

        let result = q.substitute_acyclic(&assignments).unwrap();
        assert_abs_diff_eq!(result, ans.into());
    }

    proptest! {
        #[test]
        fn removes_assigned_variables(
            f in Linear::arbitrary(),
            acyclic_assignments in AcyclicAssignments::arbitrary()
        ) {
            let original = f.required_ids();
            let assigned: VariableIDSet = acyclic_assignments.keys().collect();
            let substituted = f.substitute_acyclic(&acyclic_assignments).unwrap();
            let result_vars = substituted.required_ids();
            prop_assert!(
                result_vars.is_disjoint(&assigned),
                "original={original:?}, assigned={assigned:?}, variables after substituted={result_vars:?}",
            );
        }
    }
}
