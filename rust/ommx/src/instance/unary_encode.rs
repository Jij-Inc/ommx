use super::Instance;
use crate::{substitute_one, ATol, Bound, Coefficient, Kind, Linear, VariableID};

/// Calculate the number of binary variables for unary encoding.
///
/// Returns `(num_binary_variables, constant_offset)`.
///
/// # Errors
///
/// Returns an error if the bound is not finite, or if no feasible integer
/// values exist within the bound.
fn unary_encoding_size(bound: Bound, max_range: usize, atol: ATol) -> crate::Result<(usize, f64)> {
    let integer_bound = bound.as_integer_bound(atol).ok_or_else(|| {
        crate::error!({ ?bound }, "no feasible integer values in bound for unary-encoding: {bound}")
    })?;
    if !integer_bound.is_finite() {
        crate::bail!({ ?bound }, "bound must be finite for unary-encoding: {bound}");
    }

    let width = integer_bound.width();
    if width < 0.0 {
        crate::bail!({ ?bound }, "no feasible integer values in bound for unary-encoding: {bound}");
    }

    if width > max_range as f64 {
        crate::bail!(
            { ?bound, width, max_range },
            "range is too large for unary-encoding: {width} > max_range({max_range})"
        );
    }

    if width > usize::MAX as f64 {
        crate::bail!(
            { ?bound, width },
            "range is too large for unary-encoding: {width}"
        );
    }

    Ok((width as usize, integer_bound.lower()))
}

impl Instance {
    /// Default maximum integer range accepted by [`Self::unary_encode`].
    ///
    /// Unary encoding creates `upper - lower` auxiliary binary variables for
    /// one integer variable. This guard keeps accidental calls on very wide
    /// integer ranges from allocating impractical numbers of variables. Use
    /// [`Self::unary_encode_with_max_range`] to choose an explicit limit.
    pub const DEFAULT_UNARY_ENCODING_MAX_RANGE: usize = 1024;

    /// Encode an integer decision variable into unary binary decision variables.
    ///
    /// For an integer variable `x` with feasible integer range `[lower, upper]`,
    /// this creates `upper - lower` binary variables `b_j` and substitutes:
    ///
    /// `x = lower + sum_j b_j`
    ///
    /// Every binary configuration maps to an integer in the original range, so
    /// this encoding does not require an additional encoding-validity
    /// constraint. The number of auxiliary variables grows linearly with the
    /// range width, so this is intended for narrow integer ranges.
    #[tracing::instrument(skip(self))]
    pub fn unary_encode(&mut self, id: VariableID) -> crate::Result<Linear> {
        self.unary_encode_with_atol(id, ATol::default())
    }

    /// Encode an integer decision variable using the given bound-normalization tolerance.
    #[tracing::instrument(skip(self))]
    pub fn unary_encode_with_atol(&mut self, id: VariableID, atol: ATol) -> crate::Result<Linear> {
        self.unary_encode_with_max_range(id, Self::DEFAULT_UNARY_ENCODING_MAX_RANGE, atol)
    }

    /// Encode an integer decision variable with explicit range and tolerance settings.
    ///
    /// `max_range` is an upper bound on `upper - lower`, which is also the
    /// number of auxiliary binary variables introduced for this decision
    /// variable. `atol` is used when normalizing the decision variable bound to
    /// an integer bound.
    #[tracing::instrument(skip(self))]
    pub fn unary_encode_with_max_range(
        &mut self,
        id: VariableID,
        max_range: usize,
        atol: ATol,
    ) -> crate::Result<Linear> {
        let v = self
            .decision_variables
            .get(&id)
            .ok_or_else(|| crate::error!({ ?id }, "unknown variable for unary-encoding: {id:?}"))?;
        let (num_binary_variables, offset) = unary_encoding_size(v.bound(), max_range, atol)?;

        // Safe unwrap: offset is always finite from unary_encoding_size.
        let mut linear = Linear::try_from(offset).unwrap();
        let coefficient = Coefficient::try_from(1.0).unwrap();
        for i in 0..num_binary_variables {
            let binary_id = self.new_decision_variable_with_label(
                Kind::Binary,
                Bound::of_binary(),
                crate::ModelingLabel {
                    name: Some("ommx.unary_encode".to_string()),
                    subscripts: vec![id.into_inner() as i64, i as i64],
                    ..Default::default()
                },
                None,
                atol,
            )?;
            linear.add_term(binary_id.into(), coefficient)?;
        }
        let f = linear.clone().into();
        // Safe unwrap: there is no recursive assignment and self-assignment.
        substitute_one(self, id, &f).unwrap();
        Ok(linear)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, v1::State, Bound, DecisionVariable, Evaluate, Instance, Kind, LinearMonomial,
        Solution,
    };
    use approx::relative_eq;
    use proptest::prelude::*;
    use std::collections::BTreeSet;

    const MAX_PROPTEST_UNARY_WIDTH: usize = 8;
    const EVALUATION_EQ_ABS_TOL: f64 = 1e-8;
    const EVALUATION_EQ_REL_TOL: f64 = 1e-12;

    #[derive(Clone, Debug)]
    struct UnaryEncodeTarget {
        id: VariableID,
        lower: i64,
        width: usize,
    }

    fn unary_range(bound: Bound, max_width: usize) -> Option<(i64, usize)> {
        let integer_bound = bound.as_integer_bound(ATol::default())?;
        if !integer_bound.is_finite() {
            return None;
        }
        let width = integer_bound.width();
        if width < 0.0 || width > max_width as f64 {
            return None;
        }
        Some((integer_bound.lower() as i64, width as usize))
    }

    fn active_special_constraint_variables(instance: &Instance) -> BTreeSet<VariableID> {
        let mut ids = BTreeSet::new();
        ids.extend(
            instance
                .indicator_constraints()
                .values()
                .map(|constraint| constraint.indicator_variable),
        );
        ids.extend(
            instance
                .one_hot_constraints()
                .values()
                .flat_map(|constraint| constraint.variables.iter().copied()),
        );
        ids.extend(
            instance
                .sos1_constraints()
                .values()
                .flat_map(|constraint| constraint.variables.iter().copied()),
        );
        ids
    }

    fn unary_encode_targets(instance: &Instance, max_width: usize) -> Vec<UnaryEncodeTarget> {
        let special_variables = active_special_constraint_variables(instance);
        instance
            .decision_variable_usage()
            .used_integer()
            .into_iter()
            .filter_map(|(id, bound)| {
                if special_variables.contains(&id) {
                    return None;
                }
                let (lower, width) = unary_range(bound, max_width)?;
                Some(UnaryEncodeTarget { id, lower, width })
            })
            .collect()
    }

    fn arbitrary_unary_encode_case() -> BoxedStrategy<(Instance, UnaryEncodeTarget, State)> {
        Instance::arbitrary()
            .prop_filter_map(
                "instance must contain an encodable used integer variable",
                |instance| {
                    let targets = unary_encode_targets(&instance, MAX_PROPTEST_UNARY_WIDTH);
                    (!targets.is_empty()).then_some((instance, targets))
                },
            )
            .prop_flat_map(|(instance, targets)| {
                proptest::sample::select(targets).prop_flat_map(move |target| {
                    let state = instance.arbitrary_state();
                    (Just(instance.clone()), Just(target), state)
                })
            })
            .boxed()
    }

    fn sorted_unary_binary_ids(encoding: &Linear) -> Vec<VariableID> {
        let mut ids: Vec<_> = encoding
            .iter()
            .filter_map(|(monomial, _)| match monomial {
                LinearMonomial::Variable(id) => Some(*id),
                LinearMonomial::Constant => None,
            })
            .collect();
        ids.sort();
        ids
    }

    fn decoded_unary_value(lower: i64, bits: &[bool]) -> f64 {
        lower as f64 + bits.iter().filter(|bit| **bit).count() as f64
    }

    fn state_with_original_value(
        mut state: State,
        target: &UnaryEncodeTarget,
        value: f64,
    ) -> State {
        state.entries.insert(target.id.into_inner(), value);
        state
    }

    fn state_with_unary_bits(
        mut state: State,
        target: &UnaryEncodeTarget,
        binary_ids: &[VariableID],
        bits: &[bool],
    ) -> State {
        state.entries.remove(&target.id.into_inner());
        for (id, bit) in binary_ids.iter().zip(bits) {
            state
                .entries
                .insert(id.into_inner(), if *bit { 1.0 } else { 0.0 });
        }
        state
    }

    fn assert_float_eq(
        context: &str,
        left: f64,
        right: f64,
    ) -> Result<(), proptest::test_runner::TestCaseError> {
        prop_assert!(
            relative_eq!(
                left,
                right,
                epsilon = EVALUATION_EQ_ABS_TOL,
                max_relative = EVALUATION_EQ_REL_TOL
            ),
            "{context}: left={left}, right={right}"
        );
        Ok(())
    }

    fn assert_same_observable_evaluation(
        expected: &Solution,
        actual: &Solution,
    ) -> Result<(), proptest::test_runner::TestCaseError> {
        assert_float_eq("objective", *expected.objective(), *actual.objective())?;
        prop_assert_eq!(expected.feasible(), actual.feasible());
        prop_assert_eq!(
            expected.feasible_constraints_relaxed(),
            actual.feasible_constraints_relaxed()
        );
        prop_assert_eq!(
            expected.evaluated_constraints().removed_reasons(),
            actual.evaluated_constraints().removed_reasons()
        );
        prop_assert_eq!(
            expected.evaluated_indicator_constraints().removed_reasons(),
            actual.evaluated_indicator_constraints().removed_reasons()
        );
        prop_assert_eq!(
            expected.evaluated_one_hot_constraints().removed_reasons(),
            actual.evaluated_one_hot_constraints().removed_reasons()
        );
        prop_assert_eq!(
            expected.evaluated_sos1_constraints().removed_reasons(),
            actual.evaluated_sos1_constraints().removed_reasons()
        );

        for (id, expected_constraint) in expected.evaluated_constraints().iter() {
            if expected
                .evaluated_constraints()
                .removed_reasons()
                .contains_key(id)
            {
                continue;
            }
            let actual_constraint = actual.evaluated_constraints().get(id).unwrap();
            assert_float_eq(
                "regular constraint",
                expected_constraint.stage.evaluated_value,
                actual_constraint.stage.evaluated_value,
            )?;
            prop_assert_eq!(
                expected_constraint.stage.feasible,
                actual_constraint.stage.feasible
            );
        }

        for (id, expected_constraint) in expected.evaluated_indicator_constraints().iter() {
            if expected
                .evaluated_indicator_constraints()
                .removed_reasons()
                .contains_key(id)
            {
                continue;
            }
            let actual_constraint = actual.evaluated_indicator_constraints().get(id).unwrap();
            assert_float_eq(
                "indicator constraint",
                expected_constraint.stage.evaluated_value,
                actual_constraint.stage.evaluated_value,
            )?;
            prop_assert_eq!(
                expected_constraint.stage.feasible,
                actual_constraint.stage.feasible
            );
            prop_assert_eq!(
                expected_constraint.stage.indicator_active,
                actual_constraint.stage.indicator_active
            );
        }

        for (id, expected_constraint) in expected.evaluated_one_hot_constraints().iter() {
            if expected
                .evaluated_one_hot_constraints()
                .removed_reasons()
                .contains_key(id)
            {
                continue;
            }
            let actual_constraint = actual.evaluated_one_hot_constraints().get(id).unwrap();
            prop_assert_eq!(
                expected_constraint.stage.feasible,
                actual_constraint.stage.feasible
            );
            prop_assert_eq!(
                expected_constraint.stage.active_variable,
                actual_constraint.stage.active_variable
            );
        }

        for (id, expected_constraint) in expected.evaluated_sos1_constraints().iter() {
            if expected
                .evaluated_sos1_constraints()
                .removed_reasons()
                .contains_key(id)
            {
                continue;
            }
            let actual_constraint = actual.evaluated_sos1_constraints().get(id).unwrap();
            prop_assert_eq!(
                expected_constraint.stage.feasible,
                actual_constraint.stage.feasible
            );
            prop_assert_eq!(
                expected_constraint.stage.active_variable,
                actual_constraint.stage.active_variable
            );
        }

        prop_assert_eq!(
            expected
                .evaluated_named_functions()
                .keys()
                .collect::<Vec<_>>(),
            actual
                .evaluated_named_functions()
                .keys()
                .collect::<Vec<_>>()
        );
        for (id, expected_named_function) in expected.evaluated_named_functions() {
            let actual_named_function = actual.evaluated_named_functions().get(id).unwrap();
            assert_float_eq(
                "named function",
                expected_named_function.evaluated_value(),
                actual_named_function.evaluated_value(),
            )?;
        }

        Ok(())
    }

    proptest! {
        #[test]
        fn unary_encode_preserves_full_v3_instance_evaluation(
            (instance, target, state, bits) in arbitrary_unary_encode_case()
                .prop_flat_map(|(instance, target, state)| {
                    let bits = proptest::collection::vec(any::<bool>(), target.width);
                    (Just(instance), Just(target), Just(state), bits)
                })
        ) {
            let decoded_value = decoded_unary_value(target.lower, &bits);
            let expected_state = state_with_original_value(state.clone(), &target, decoded_value);
            let expected = instance.evaluate(&expected_state, ATol::default()).unwrap();

            let mut encoded_instance = instance.clone();
            let encoding = encoded_instance.unary_encode(target.id).unwrap();
            let binary_ids = sorted_unary_binary_ids(&encoding);
            prop_assert_eq!(binary_ids.len(), target.width);

            let encoded_state = state_with_unary_bits(state, &target, &binary_ids, &bits);
            let actual = encoded_instance.evaluate(&encoded_state, ATol::default()).unwrap();

            assert_same_observable_evaluation(&expected, &actual)?;
        }

        #[test]
        fn unary_encode_depends_only_on_unary_bit_sum(
            (instance, target, state, bit_sum) in arbitrary_unary_encode_case()
                .prop_flat_map(|(instance, target, state)| {
                    let bit_sum = 0..=target.width;
                    (Just(instance), Just(target), Just(state), bit_sum)
                })
        ) {
            let mut encoded_instance = instance;
            let encoding = encoded_instance.unary_encode(target.id).unwrap();
            let binary_ids = sorted_unary_binary_ids(&encoding);
            prop_assert_eq!(binary_ids.len(), target.width);

            let mut prefix_bits = vec![false; target.width];
            prefix_bits.iter_mut().take(bit_sum).for_each(|bit| *bit = true);
            let mut suffix_bits = vec![false; target.width];
            suffix_bits
                .iter_mut()
                .rev()
                .take(bit_sum)
                .for_each(|bit| *bit = true);

            let prefix_state =
                state_with_unary_bits(state.clone(), &target, &binary_ids, &prefix_bits);
            let suffix_state = state_with_unary_bits(state, &target, &binary_ids, &suffix_bits);
            let prefix = encoded_instance.evaluate(&prefix_state, ATol::default()).unwrap();
            let suffix = encoded_instance.evaluate(&suffix_state, ATol::default()).unwrap();

            assert_same_observable_evaluation(&prefix, &suffix)?;
        }
    }

    #[test]
    fn test_unary_encode_instance() {
        // Create instance with integer variable in range [2, 5].
        let mut instance = Instance::default();
        let id = VariableID::from(0);
        let var = DecisionVariable::new(
            Kind::Integer,
            Bound::new(2.0, 5.0).unwrap(),
            crate::ATol::default(),
        )
        .unwrap();
        instance
            .add_decision_variable(id, var, Default::default())
            .unwrap();

        let encoded = instance.unary_encode(id).unwrap();

        // The original variable is still present but substituted.
        assert!(instance.decision_variables.contains_key(&id));

        // Check binary variables were created with correct labels.
        let store = instance.variable_labels();
        let binary_ids: Vec<_> = instance
            .decision_variables
            .iter()
            .filter(|(id, _)| {
                store.name(**id) == Some("ommx.unary_encode")
                    && store.subscripts(**id).first().copied() == Some(0)
            })
            .map(|(id, dv)| {
                assert_eq!(dv.kind(), Kind::Binary);
                *id
            })
            .collect();

        // For range [2, 5], unary encoding needs upper - lower = 3 bits.
        assert_eq!(binary_ids.len(), 3);

        assert_eq!(encoded.get(&LinearMonomial::Constant), Some(coeff!(2.0)));
        for id in binary_ids {
            assert_eq!(
                encoded.get(&LinearMonomial::Variable(id)),
                Some(coeff!(1.0))
            );
        }
    }

    #[test]
    fn test_unary_encoding_size() {
        let bound = Bound::new(0.0, 3.0).unwrap();
        let (num_binary_variables, offset) =
            unary_encoding_size(bound, 3, ATol::default()).unwrap();
        assert_eq!(num_binary_variables, 3);
        assert_eq!(offset, 0.0);

        let bound = Bound::new(1.0, 6.0).unwrap();
        let (num_binary_variables, offset) =
            unary_encoding_size(bound, 5, ATol::default()).unwrap();
        assert_eq!(num_binary_variables, 5);
        assert_eq!(offset, 1.0);

        let bound = Bound::new(1.000000000001, 2.999999999999).unwrap();
        let (num_binary_variables, offset) =
            unary_encoding_size(bound, 2, ATol::default()).unwrap();
        assert_eq!(num_binary_variables, 2);
        assert_eq!(offset, 1.0);

        let bound = Bound::new(2.0, 2.0).unwrap();
        let (num_binary_variables, offset) =
            unary_encoding_size(bound, 0, ATol::default()).unwrap();
        assert_eq!(num_binary_variables, 0);
        assert_eq!(offset, 2.0);

        let bound = Bound::new(1.3, 1.6).unwrap();
        assert!(unary_encoding_size(bound, 1, ATol::default()).is_err());
    }

    #[test]
    fn test_unary_encoding_size_respects_max_range() {
        let bound = Bound::new(0.0, 6.0).unwrap();
        let err = unary_encoding_size(bound, 5, ATol::default()).unwrap_err();
        assert!(err.to_string().contains("max_range(5)"));
    }
}
