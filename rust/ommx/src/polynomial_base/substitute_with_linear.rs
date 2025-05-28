use crate::{
    substitute::{AcyclicLinearAssignments, SubstituteWithLinears},
    Linear, LinearMonomial, Monomial, MonomialDyn, Polynomial, PolynomialBase, Quadratic,
    QuadraticMonomial, VariableID,
};
use fnv::FnvHashMap;

impl<M> SubstituteWithLinears for PolynomialBase<M>
where
    M: Monomial + SubstituteWithLinears<Output = Self>,
{
    type Output = Self;
    fn substitute_with_linears(
        &self,
        linear_assignments: impl IntoIterator<Item = (VariableID, Linear)>,
    ) -> Self::Output {
        let acyclic_assignments = match AcyclicLinearAssignments::new(linear_assignments) {
            Ok(assignments) => assignments,
            Err(_) => {
                // If there are cycles in the assignments, we can't proceed safely
                // For now, return self unchanged. In the future, this could be handled
                // differently based on requirements.
                return self.clone();
            }
        };

        let assignments_map: FnvHashMap<VariableID, &Linear> =
            acyclic_assignments.sorted_iter().collect();

        let mut substituted = Self::default();
        for (monomial, coefficient) in self.terms.iter() {
            let sub_monomial = monomial.substitute_with_linears_map(&assignments_map);
            for (m, c) in sub_monomial.terms {
                substituted.add_term(m, c * *coefficient);
            }
        }
        substituted
    }
}

// Internal trait for substitution with a map reference
trait SubstituteWithLinearsMap {
    type Output;
    fn substitute_with_linears_map(
        &self,
        linear_assignments: &FnvHashMap<VariableID, &Linear>,
    ) -> Self::Output;
}

impl<M> SubstituteWithLinearsMap for M
where
    M: Monomial + SubstituteWithLinears<Output = PolynomialBase<M>>,
{
    type Output = PolynomialBase<M>;
    fn substitute_with_linears_map(
        &self,
        linear_assignments: &FnvHashMap<VariableID, &Linear>,
    ) -> Self::Output {
        // Create a temporary collection that implements the old interface
        let assignments_map: FnvHashMap<VariableID, Linear> = linear_assignments
            .iter()
            .map(|(&id, &linear)| (id, linear.clone()))
            .collect();
        let assignments_vec: Vec<(VariableID, Linear)> = assignments_map.into_iter().collect();
        self.substitute_with_linears(assignments_vec)
    }
}

impl SubstituteWithLinears for LinearMonomial {
    type Output = Linear;
    fn substitute_with_linears(
        &self,
        linear_assignments: impl IntoIterator<Item = (VariableID, Linear)>,
    ) -> Self::Output {
        let assignments_map: FnvHashMap<VariableID, Linear> =
            linear_assignments.into_iter().collect();
        match self {
            LinearMonomial::Variable(id) => {
                if let Some(linear_func) = assignments_map.get(id) {
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

    fn substitute_with_linears(
        &self,
        linear_assignments: impl IntoIterator<Item = (VariableID, Linear)>,
    ) -> Self::Output {
        let assignments_map: FnvHashMap<VariableID, Linear> =
            linear_assignments.into_iter().collect();
        match self {
            QuadraticMonomial::Pair(pair) => {
                let l_sub = LinearMonomial::Variable(pair.lower()).substitute_with_linears(
                    assignments_map
                        .iter()
                        .map(|(&id, linear)| (id, linear.clone())),
                );
                let u_sub = LinearMonomial::Variable(pair.upper()).substitute_with_linears(
                    assignments_map
                        .iter()
                        .map(|(&id, linear)| (id, linear.clone())),
                );
                &l_sub * &u_sub
            }
            QuadraticMonomial::Linear(id) => LinearMonomial::Variable(*id)
                .substitute_with_linears(
                    assignments_map
                        .iter()
                        .map(|(&id, linear)| (id, linear.clone())),
                )
                .into(),
            QuadraticMonomial::Constant => Quadratic::one(),
        }
    }
}

impl SubstituteWithLinears for MonomialDyn {
    type Output = Polynomial;
    fn substitute_with_linears(
        &self,
        linear_assignments: impl IntoIterator<Item = (VariableID, Linear)>,
    ) -> Self::Output {
        let assignments_map: FnvHashMap<VariableID, Linear> =
            linear_assignments.into_iter().collect();
        let mut substituted = Polynomial::one();
        let mut non_substituted = Vec::new();
        for var_id in self.iter() {
            if let Some(linear_func) = assignments_map.get(var_id) {
                substituted = &substituted * linear_func;
            } else {
                non_substituted.push(*var_id);
            }
        }
        let non_substituted = Polynomial::from(MonomialDyn::from(non_substituted));
        &substituted * &non_substituted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Coefficient, VariableID};

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

        let result = poly.substitute_with_linears(assignments);
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

        let result = q.substitute_with_linears(assignments);
        assert_eq!(result, ans);
    }
}
