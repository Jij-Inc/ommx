use super::{Instance, Sense};
use crate::{BinaryIdPair, BinaryIds, Evaluate};
use anyhow::{bail, Result};
use std::collections::BTreeMap;

impl Instance {
    /// Create QUBO (Quadratic Unconstrained Binary Optimization) dictionary from the instance.
    ///
    /// Before calling this method, you should check that this instance is suitable for QUBO:
    ///
    /// - This instance has no constraints
    ///   - Use penalty method (TODO: ALM will be added) to convert into an unconstrained problem.
    /// - The objective function uses only binary decision variables.
    ///   - TODO: Binary encoding will be added.
    /// - The objective is a polynomial of degree at most 2.
    ///
    /// # Numerical behavior
    ///
    /// Finite nonzero coefficients and constant offsets are retained regardless
    /// of magnitude, including values at or below [`f64::EPSILON`]. Terms with
    /// the same binary key are summed using `f64` arithmetic; only exact zero
    /// entries are omitted. Aggregation can round or overflow, and resulting
    /// sums are not checked for finiteness. Evaluation tolerances do not affect
    /// formatting, and the instance is not mutated.
    ///
    /// # Postconditions
    ///
    /// The returned QUBO encodes the active objective, while evaluation uses the output objective.
    ///
    /// ```
    /// use ommx::{
    ///     linear, v1::State, ATol, BinaryIdPair, DecisionVariable, Evaluate,
    ///     Function, Instance, Sense, VariableID,
    /// };
    /// use std::collections::BTreeMap;
    ///
    /// let mut instance = Instance::builder()
    ///     .sense(Sense::Maximize)
    ///     .objective(Function::from(linear!(1)))
    ///     .decision_variables(BTreeMap::from([(
    ///         VariableID::from(1),
    ///         DecisionVariable::binary(),
    ///     )]))
    ///     .constraints(BTreeMap::new())
    ///     .build()
    ///     .unwrap();
    /// assert!(instance.convert_active_objective(Sense::Minimize));
    ///
    /// let (qubo, offset) = instance.as_qubo_format().unwrap();
    /// assert_eq!(qubo.get(&BinaryIdPair(1, 1)), Some(&-1.0));
    /// assert_eq!(offset, 0.0);
    ///
    /// let solution = instance
    ///     .evaluate(&State::from_iter([(1, 1.0)]), ATol::default())
    ///     .unwrap();
    /// assert_eq!(*solution.sense(), Some(Sense::Maximize));
    /// assert_eq!(*solution.objective(), 1.0);
    /// ```
    #[tracing::instrument(skip_all)]
    pub fn as_qubo_format(&self) -> Result<(BTreeMap<BinaryIdPair, f64>, f64)> {
        let Some(terms) = self.objective().iter() else {
            bail!("QUBO format does not support a non-polynomial objective function.");
        };
        if self.sense() == Sense::Maximize {
            bail!("QUBO format is only for minimization problems.");
        }
        if !self.constraints().is_empty() {
            bail!("The instance still has constraints. Use penalty method or other way to translate into unconstrained problem first.");
        }
        let special_constraint_kinds = self.active_special_constraint_kinds();
        if !special_constraint_kinds.is_empty() {
            bail!(
                "QUBO format does not support these special constraint types: {special_constraint_kinds:?}. Lower or convert them via an appropriate method first."
            );
        }
        if !self
            .objective()
            .required_ids()
            .is_subset(&self.binary_ids())
        {
            bail!("The objective function uses non-binary decision variables.");
        }
        let mut constant = 0.0;
        let mut quad: BTreeMap<BinaryIdPair, f64> = BTreeMap::new();
        for (ids, coefficient) in terms {
            let c = coefficient.into_inner();
            if ids.is_empty() {
                constant += c;
            } else {
                let key = BinaryIdPair::try_from(ids)?;
                let value = quad.entry(key).and_modify(|v| *v += c).or_insert(c);
                if *value == 0.0 {
                    quad.remove(&key);
                }
            }
        }
        Ok((quad, constant))
    }

    /// Create HUBO (Higher-Order Unconstrained Binary Optimization) dictionary from the instance.
    ///
    /// Before calling this method, you should check that this instance is suitable for HUBO:
    ///
    /// - This instance has no constraints
    ///   - Use penalty method (TODO: ALM will be added) to convert into an unconstrained problem.
    /// - The objective function uses only binary decision variables.
    ///   - TODO: Binary encoding will be added.
    /// - The objective is polynomial.
    ///
    /// # Numerical behavior
    ///
    /// Finite nonzero coefficients and constant offsets are retained regardless
    /// of magnitude, including values at or below [`f64::EPSILON`]. Terms with
    /// the same binary key are summed using `f64` arithmetic; only exact zero
    /// entries are omitted. Aggregation can round or overflow, and resulting
    /// sums are not checked for finiteness. Evaluation tolerances do not affect
    /// formatting, and the instance is not mutated.
    ///
    /// # Postconditions
    ///
    /// The returned HUBO encodes the active objective, while evaluation uses the output objective.
    ///
    /// ```
    /// use ommx::{
    ///     linear, v1::State, ATol, DecisionVariable, Evaluate, Function,
    ///     Instance, Sense, VariableID,
    /// };
    /// use std::collections::BTreeMap;
    ///
    /// let mut instance = Instance::builder()
    ///     .sense(Sense::Maximize)
    ///     .objective(Function::from(linear!(1)))
    ///     .decision_variables(BTreeMap::from([(
    ///         VariableID::from(1),
    ///         DecisionVariable::binary(),
    ///     )]))
    ///     .constraints(BTreeMap::new())
    ///     .build()
    ///     .unwrap();
    /// assert!(instance.convert_active_objective(Sense::Minimize));
    ///
    /// let (hubo, offset) = instance.as_hubo_format().unwrap();
    /// assert_eq!(hubo.len(), 1);
    /// assert_eq!(hubo.values().next(), Some(&-1.0));
    /// assert_eq!(offset, 0.0);
    ///
    /// let solution = instance
    ///     .evaluate(&State::from_iter([(1, 1.0)]), ATol::default())
    ///     .unwrap();
    /// assert_eq!(*solution.sense(), Some(Sense::Maximize));
    /// assert_eq!(*solution.objective(), 1.0);
    /// ```
    #[tracing::instrument(skip_all)]
    pub fn as_hubo_format(&self) -> Result<(BTreeMap<BinaryIds, f64>, f64)> {
        let Some(terms) = self.objective().iter() else {
            bail!("HUBO format does not support a non-polynomial objective function.");
        };
        if self.sense() == Sense::Maximize {
            bail!("HUBO format is only for minimization problems.");
        }
        if !self.constraints().is_empty() {
            bail!("The instance still has constraints. Use penalty method or other way to translate into unconstrained problem first.");
        }
        let special_constraint_kinds = self.active_special_constraint_kinds();
        if !special_constraint_kinds.is_empty() {
            bail!(
                "HUBO format does not support these special constraint types: {special_constraint_kinds:?}. Lower or convert them via an appropriate method first."
            );
        }
        if !self
            .objective()
            .required_ids()
            .is_subset(&self.binary_ids())
        {
            bail!("The objective function uses non-binary decision variables.");
        }
        let mut constant = 0.0;
        let mut hubo: BTreeMap<BinaryIds, f64> = BTreeMap::new();
        for (ids, coefficient) in terms {
            let c = coefficient.into_inner();
            if ids.is_empty() {
                constant += c;
            } else {
                let key = BinaryIds::from(ids);
                let value = hubo.entry(key.clone()).and_modify(|v| *v += c).or_insert(c);
                if *value == 0.0 {
                    hubo.remove(&key);
                }
            }
        }
        Ok((hubo, constant))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, linear, quadratic, v1::State, ATol, DecisionVariable, Function, MonomialDyn,
        Polynomial, VariableID,
    };
    use maplit::btreemap;
    use std::collections::BTreeMap;

    fn binary_vars(ids: impl IntoIterator<Item = u64>) -> BTreeMap<VariableID, DecisionVariable> {
        ids.into_iter()
            .map(|i| {
                let id = VariableID::from(i);
                (id, DecisionVariable::binary())
            })
            .collect()
    }

    #[test]
    fn qubo_and_hubo_preserve_small_coefficients_and_offsets() {
        for magnitude in [
            f64::from_bits(1),
            f64::MIN_POSITIVE,
            1e-20,
            f64::EPSILON / 2.0,
            f64::EPSILON,
            f64::from_bits(f64::EPSILON.to_bits() + 1),
        ] {
            for c in [magnitude, -magnitude] {
                let objective = ((coeff!(c) * linear!(1)).unwrap() + coeff!(c)).unwrap();
                let instance = Instance::new(
                    Sense::Minimize,
                    objective.into(),
                    binary_vars([1]),
                    BTreeMap::new(),
                )
                .unwrap();
                let (qubo, qubo_offset) = instance.as_qubo_format().unwrap();
                let (hubo, hubo_offset) = instance.as_hubo_format().unwrap();
                let key = BinaryIds::from(MonomialDyn::new(vec![VariableID::from(1)]));
                assert_eq!(qubo, BTreeMap::from([(BinaryIdPair(1, 1), c)]));
                assert_eq!(hubo, BTreeMap::from([(key.clone(), c)]));
                assert_eq!(qubo_offset, c);
                assert_eq!(hubo_offset, c);

                for x in [0.0, 1.0] {
                    let state = State::from_iter([(1, x)]);
                    let expected = instance
                        .objective()
                        .evaluate(&state, ATol::default())
                        .unwrap();
                    assert_eq!(qubo_offset + qubo[&BinaryIdPair(1, 1)] * x, expected);
                    assert_eq!(hubo_offset + hubo[&key] * x, expected);
                }
                // The formatter must preserve which of x=0 and x=1 is better.
                assert_eq!(
                    qubo_offset < qubo_offset + qubo[&BinaryIdPair(1, 1)],
                    c > 0.0
                );
                assert_eq!(hubo_offset < hubo_offset + hubo[&key], c > 0.0);
            }
        }
    }

    #[test]
    fn qubo_and_hubo_preserve_small_interaction_terms() {
        for degree in [2, 3] {
            for c in [1e-20, -1e-20] {
                let monomial = MonomialDyn::new((1..=degree).map(VariableID::from).collect());
                let objective = Polynomial::single_term(monomial.clone(), coeff!(c));
                let instance = Instance::new(
                    Sense::Minimize,
                    objective.into(),
                    binary_vars(1..=degree),
                    BTreeMap::new(),
                )
                .unwrap();
                let (hubo, offset) = instance.as_hubo_format().unwrap();
                assert_eq!(hubo, BTreeMap::from([(BinaryIds::from(monomial), c)]));
                assert_eq!(offset, 0.0);
                for mask in 0..(1 << degree) {
                    let state = State::from_iter(
                        (1..=degree).map(|id| (id, f64::from((mask >> (id - 1)) & 1))),
                    );
                    let expected = instance
                        .objective()
                        .evaluate(&state, ATol::default())
                        .unwrap();
                    let actual = offset
                        + hubo
                            .iter()
                            .map(|(ids, c)| {
                                c * ids.iter().map(|id| state.entries[id]).product::<f64>()
                            })
                            .sum::<f64>();
                    assert_eq!(actual, expected);
                    if degree == 2 {
                        let (qubo, offset) = instance.as_qubo_format().unwrap();
                        assert_eq!(qubo, BTreeMap::from([(BinaryIdPair(1, 2), c)]));
                        assert_eq!(offset, 0.0);
                        assert_eq!(
                            offset
                                + qubo[&BinaryIdPair(1, 2)] * state.entries[&1] * state.entries[&2],
                            expected
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn qubo_and_hubo_remove_only_exact_zero_after_binary_aggregation() {
        for residual in [0.0, f64::EPSILON / 2.0, -f64::EPSILON / 2.0] {
            let sign = if residual < 0.0 { -1.0 } else { 1.0 };
            let objective = (Function::from(coeff!(sign) * linear!(1))
                + Function::from(coeff!(-sign * (1.0 - residual.abs())) * quadratic!(1, 1)))
            .unwrap();
            let instance = Instance::new(
                Sense::Minimize,
                objective,
                binary_vars([1]),
                BTreeMap::new(),
            )
            .unwrap();
            let (qubo, qubo_offset) = instance.as_qubo_format().unwrap();
            let (hubo, hubo_offset) = instance.as_hubo_format().unwrap();
            let key = BinaryIds::from(MonomialDyn::new(vec![VariableID::from(1)]));
            if residual == 0.0 {
                assert!(qubo.is_empty());
                assert!(hubo.is_empty());
            } else {
                assert_eq!(qubo, BTreeMap::from([(BinaryIdPair(1, 1), residual)]));
                assert_eq!(hubo, BTreeMap::from([(key.clone(), residual)]));
            }
            assert_eq!(qubo_offset, 0.0);
            assert_eq!(hubo_offset, 0.0);
            for x in [0.0, 1.0] {
                let state = State::from_iter([(1, x)]);
                let expected = instance
                    .objective()
                    .evaluate(&state, ATol::default())
                    .unwrap();
                assert_eq!(
                    qubo.get(&BinaryIdPair(1, 1)).copied().unwrap_or(0.0) * x,
                    expected
                );
                assert_eq!(hubo.get(&key).copied().unwrap_or(0.0) * x, expected);
            }
        }
    }

    #[test]
    fn qubo_from_quadratic_objective() {
        // min x1 + 2*x2 + 3*x1*x2 with binary x1, x2
        let objective =
            (Function::from(linear!(1)) + Function::from(coeff!(2.0) * linear!(2))).unwrap();
        let objective = (objective + Function::from(coeff!(3.0) * quadratic!(1, 2))).unwrap();
        let instance = Instance::new(
            Sense::Minimize,
            objective,
            binary_vars([1, 2]),
            BTreeMap::new(),
        )
        .unwrap();

        let (quad, constant) = instance.as_qubo_format().unwrap();
        assert_eq!(constant, 0.0);
        assert_eq!(quad.get(&BinaryIdPair(1, 1)), Some(&1.0));
        assert_eq!(quad.get(&BinaryIdPair(2, 2)), Some(&2.0));
        assert_eq!(quad.get(&BinaryIdPair(1, 2)), Some(&3.0));
    }

    #[test]
    fn qubo_rejects_maximization() {
        let instance = Instance::new(
            Sense::Maximize,
            linear!(1).into(),
            binary_vars([1]),
            BTreeMap::new(),
        )
        .unwrap();
        let err = instance.as_qubo_format().unwrap_err();
        assert!(err.to_string().contains("minimization"));
    }

    #[test]
    fn qubo_and_hubo_reject_non_polynomial_objectives() {
        let objective = Function::from(linear!(VariableID::from(1))).abs();
        let instance = Instance::new(
            Sense::Minimize,
            objective,
            binary_vars([1]),
            BTreeMap::new(),
        )
        .unwrap();

        let qubo_error = instance.as_qubo_format().unwrap_err();
        assert!(qubo_error.to_string().contains("non-polynomial objective"));

        let hubo_error = instance.as_hubo_format().unwrap_err();
        assert!(hubo_error.to_string().contains("non-polynomial objective"));
    }

    #[test]
    fn qubo_rejects_instances_with_constraints() {
        let constraints = btreemap! {
            crate::ConstraintID::from(0) =>
                crate::Constraint::equal_to_zero(linear!(1).into()),
        };
        let instance = Instance::new(
            Sense::Minimize,
            linear!(1).into(),
            binary_vars([1]),
            constraints,
        )
        .unwrap();
        let err = instance.as_qubo_format().unwrap_err();
        assert!(err.to_string().contains("constraints"));
    }

    #[test]
    fn qubo_rejects_non_binary_decision_variables() {
        let mut dv = binary_vars([1]);
        let id = VariableID::from(2);
        dv.insert(id, DecisionVariable::integer());
        let instance = Instance::new(
            Sense::Minimize,
            (linear!(1) + linear!(2)).into(),
            dv,
            BTreeMap::new(),
        )
        .unwrap();
        let err = instance.as_qubo_format().unwrap_err();
        assert!(err.to_string().contains("non-binary"));
    }

    #[test]
    fn qubo_rejects_instances_with_one_hot_constraints() {
        use crate::{OneHotConstraint, OneHotConstraintID};
        use std::collections::BTreeSet;

        let mut instance = Instance::new(
            Sense::Minimize,
            linear!(1).into(),
            binary_vars([1, 2]),
            BTreeMap::new(),
        )
        .unwrap();
        let one_hot = OneHotConstraint::new(
            [VariableID::from(1), VariableID::from(2)]
                .into_iter()
                .collect::<BTreeSet<_>>(),
        )
        .unwrap();
        instance
            .one_hot_constraint_collection
            .insert_active_with_context(
                OneHotConstraintID::from(0),
                one_hot,
                crate::ConstraintContext::default(),
            )
            .unwrap();

        let err = instance.as_qubo_format().unwrap_err();
        let msg = err.to_string() + " " + &err.root_cause().to_string();
        assert!(
            msg.contains("QUBO") || msg.contains("Unsupported"),
            "expected rejection message, got: {msg}"
        );
    }

    #[test]
    fn hubo_from_cubic_objective() {
        // min x1*x2*x3 + x1 with binary x1, x2, x3
        use crate::MonomialDyn;
        let cubic = crate::Polynomial::single_term(
            MonomialDyn::new(vec![
                VariableID::from(1),
                VariableID::from(2),
                VariableID::from(3),
            ]),
            coeff!(1.0),
        );
        let objective = (Function::from(linear!(1)) + Function::from(cubic)).unwrap();
        let instance = Instance::new(
            Sense::Minimize,
            objective,
            binary_vars([1, 2, 3]),
            BTreeMap::new(),
        )
        .unwrap();

        let (hubo, constant) = instance.as_hubo_format().unwrap();
        assert_eq!(constant, 0.0);
        // Cubic term
        let cubic_key = BinaryIds::from(MonomialDyn::new(vec![
            VariableID::from(1),
            VariableID::from(2),
            VariableID::from(3),
        ]));
        assert_eq!(hubo.get(&cubic_key), Some(&1.0));
        // Linear term
        let linear_key = BinaryIds::from(MonomialDyn::new(vec![VariableID::from(1)]));
        assert_eq!(hubo.get(&linear_key), Some(&1.0));
    }
}
