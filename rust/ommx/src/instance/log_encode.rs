use super::Instance;
use crate::{ATol, Bound, Coefficient, Function, Kind, Linear, Substitute, VariableID};
use std::collections::{BTreeMap, BTreeSet};

const MAX_LOG_ENCODING_BITS: usize = 53;
const MAX_LOG_ENCODING_RANGE_WIDTH: u64 = (1_u64 << MAX_LOG_ENCODING_BITS) - 1;

fn log_encoding_bit_count(width: f64) -> crate::Result<usize> {
    if width == 0.0 {
        return Ok(0);
    }
    if width > MAX_LOG_ENCODING_RANGE_WIDTH as f64 {
        crate::bail!(
            { width, max_bits = MAX_LOG_ENCODING_BITS },
            "range is too large for log-encoding: width {width} requires more than {MAX_LOG_ENCODING_BITS} binary variables",
        );
    }
    if width.fract() != 0.0 {
        crate::bail!(
            { width },
            "integer range width is not exactly representable for log-encoding: {width}",
        );
    }
    let width = width as u64;
    Ok((u64::BITS - width.leading_zeros()) as usize)
}

/// Calculate log-encoding coefficients for a given bound.
///
/// Returns `(coefficients, constant_offset)` where:
/// - `coefficients`: Vector of coefficients for binary variables as `Coefficient` values
/// - `constant_offset`: Constant term to add
///
/// # Arguments
///
/// * `bound` - The bound of the integer variable to encode
///
/// # Errors
///
/// Returns an error if the bound is not finite, or if no feasible integer
/// values exist within the bound.
fn log_encoding_coefficients(bound: Bound, atol: ATol) -> crate::Result<(Vec<Coefficient>, f64)> {
    let integer_bound = bound.as_integer_bound(atol).ok_or_else(|| {
        crate::error!({ ?bound }, "no feasible integer values in bound for log-encoding: {bound}")
    })?;
    if !integer_bound.is_finite() {
        crate::bail!({ ?bound }, "bound must be finite for log-encoding: {bound}");
    }

    let u_l = integer_bound.width();
    if u_l < 0.0 {
        // No feasible integer values in the range
        crate::bail!({ ?bound }, "no feasible integer values in bound for log-encoding: {bound}");
    }

    let n = log_encoding_bit_count(u_l)?;
    // There is only one feasible integer, and no need to encode
    if n == 0 {
        return Ok((vec![], integer_bound.lower()));
    }

    let coefficients = (0..n)
        .map(|i| {
            // Calculate coefficient for each binary variable
            let coeff_value = if i == n - 1 {
                // Last binary variable gets special coefficient to handle exact range
                u_l - 2.0f64.powi(i as i32) + 1.0
            } else {
                // Other variables get power of 2 coefficients
                2.0f64.powi(i as i32)
            };
            Coefficient::try_from(coeff_value).unwrap()
        })
        .collect::<Vec<_>>();

    Ok((coefficients, integer_bound.lower()))
}

impl Instance {
    /// Maximum number of auxiliary binary variables introduced by log encoding
    /// for one integer decision variable.
    pub const MAX_LOG_ENCODING_BITS: usize = MAX_LOG_ENCODING_BITS;

    /// Log-encode integer decision variables into binary decision variables.
    ///
    /// The instance is cloned once, every requested variable is encoded on a
    /// clone, and the result is committed back only if all encodings succeed.
    /// Duplicate IDs are encoded once. Pass a single-element iterator such as
    /// `[id]` to encode exactly one variable.
    ///
    /// `atol` is used when normalizing each decision variable bound to an
    /// integer bound. Ranges that would require more than
    /// [`Self::MAX_LOG_ENCODING_BITS`] binary variables are rejected instead of
    /// creating an impractically large encoded search space.
    #[tracing::instrument(skip(self, ids))]
    pub fn log_encode(
        &mut self,
        ids: impl IntoIterator<Item = VariableID>,
        atol: ATol,
    ) -> crate::Result<BTreeMap<VariableID, Linear>> {
        let ids = ids.into_iter().collect::<BTreeSet<_>>();
        if ids.is_empty() {
            return Ok(BTreeMap::new());
        }

        let mut encoded = self.clone();
        let mut encoding_specs = Vec::new();
        for id in ids {
            let (coefficients, offset) = encoded.log_encoding_spec(id, atol)?;
            encoding_specs.push((id, coefficients, offset));
        }

        let mut encodings = BTreeMap::new();
        let mut assignments = Vec::new();
        for (id, coefficients, offset) in encoding_specs {
            let linear = encoded.create_log_encoding(id, coefficients, offset, atol)?;
            assignments.push((id, Function::from(linear.clone())));
            encodings.insert(id, linear);
        }
        encoded = encoded.substitute(assignments)?;
        *self = encoded;
        Ok(encodings)
    }

    fn log_encoding_spec(
        &self,
        id: VariableID,
        atol: ATol,
    ) -> crate::Result<(Vec<Coefficient>, f64)> {
        let v = self
            .decision_variables
            .get(&id)
            .ok_or_else(|| crate::error!({ ?id }, "unknown variable for log-encoding: {id:?}"))?;
        if self.fixed_decision_variable_value(id).is_some() {
            crate::bail!(
                { ?id },
                "fixed decision variable cannot be log-encoded: id={id:?}",
            );
        }
        if v.kind() != Kind::Integer {
            let kind = v.kind();
            crate::bail!(
                { ?id, ?kind },
                "variable must be integer for log-encoding: id={id:?}, kind={kind:?}",
            );
        }
        log_encoding_coefficients(v.bound(), atol)
    }

    fn create_log_encoding(
        &mut self,
        id: VariableID,
        coefficients: Vec<Coefficient>,
        offset: f64,
        atol: ATol,
    ) -> crate::Result<Linear> {
        // Safe unwrap: offset is always finite from log_encoding_coefficients
        let mut linear = Linear::try_from(offset).unwrap();
        for (i, coefficient) in coefficients.iter().enumerate() {
            let binary_id = self.new_decision_variable_with_label(
                Kind::Binary,
                Bound::of_binary(),
                crate::ModelingLabel {
                    name: Some("ommx.log_encode".to_string()),
                    subscripts: vec![id.into_inner() as i64, i as i64],
                    ..Default::default()
                },
                None,
                atol,
            )?;
            linear.add_term(binary_id.into(), *coefficient)?;
        }
        Ok(linear)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, v1::State, Bound, DecisionVariable, Equality, Evaluate, Function,
        IndicatorConstraint, IndicatorConstraintID, Instance, Kind, LinearMonomial,
        OneHotConstraint, OneHotConstraintID, Sense, Solution, Sos1Constraint, Sos1ConstraintID,
    };
    use approx::relative_eq;
    use proptest::prelude::*;
    use std::collections::{BTreeMap, BTreeSet};

    const MAX_PROPTEST_LOG_BITS: usize = 6;
    const EVALUATION_EQ_ABS_TOL: f64 = 1e-8;
    const EVALUATION_EQ_REL_TOL: f64 = 1e-12;

    fn aux_variable_count(instance: &Instance, label: &str) -> usize {
        let store = instance.variable_labels();
        instance
            .decision_variables
            .iter()
            .filter(|(id, _)| store.name(**id) == Some(label))
            .count()
    }

    fn fixed_integer_instance(id: VariableID) -> Instance {
        let var = DecisionVariable::new(
            Kind::Integer,
            Bound::new(0.0, 3.0).unwrap(),
            ATol::default(),
        )
        .unwrap();
        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([(id, var)]))
            .constraints(BTreeMap::new())
            .fixed_decision_variable_values(BTreeMap::from([(id, 1.0)]))
            .build()
            .unwrap()
    }

    #[derive(Clone, Debug)]
    struct LogEncodeTarget {
        id: VariableID,
        lower: i64,
        width: u64,
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

    fn log_range(bound: Bound, max_bits: usize) -> Option<(i64, u64)> {
        let integer_bound = bound.as_integer_bound(ATol::default())?;
        if !integer_bound.is_finite() {
            return None;
        }
        let width = integer_bound.width();
        if width < 0.0 || log_encoding_bit_count(width).ok()? > max_bits {
            return None;
        }
        if integer_bound.lower() < i64::MIN as f64 || integer_bound.lower() > i64::MAX as f64 {
            return None;
        }
        Some((integer_bound.lower() as i64, width as u64))
    }

    fn log_encode_targets(instance: &Instance, max_bits: usize) -> Vec<LogEncodeTarget> {
        let special_variables = active_special_constraint_variables(instance);
        instance
            .decision_variable_usage()
            .used_integer()
            .into_iter()
            .filter_map(|(id, bound)| {
                if special_variables.contains(&id)
                    || instance.fixed_decision_variable_value(id).is_some()
                {
                    return None;
                }
                let (lower, width) = log_range(bound, max_bits)?;
                Some(LogEncodeTarget { id, lower, width })
            })
            .collect()
    }

    fn arbitrary_log_encode_case() -> BoxedStrategy<(Instance, LogEncodeTarget, State)> {
        Instance::arbitrary()
            .prop_filter_map(
                "instance must contain an encodable used integer variable",
                |instance| {
                    let targets = log_encode_targets(&instance, MAX_PROPTEST_LOG_BITS);
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

    fn sorted_log_binary_ids(
        instance: &Instance,
        encoding: &Linear,
        original_id: VariableID,
    ) -> Vec<VariableID> {
        let store = instance.variable_labels();
        let mut ids: Vec<_> = encoding
            .iter()
            .filter_map(|(monomial, _)| match monomial {
                LinearMonomial::Variable(id) => Some(*id),
                LinearMonomial::Constant => None,
            })
            .collect();
        ids.sort_by_key(|id| {
            let subscripts = store.subscripts(*id);
            (
                subscripts.first().copied() != Some(original_id.into_inner() as i64),
                subscripts.get(1).copied().unwrap_or(i64::MAX),
                id.into_inner(),
            )
        });
        ids
    }

    fn log_bits_for_delta(delta: u64, coefficients: &[Coefficient]) -> Vec<bool> {
        if coefficients.is_empty() {
            return Vec::new();
        }
        let regular_bits = coefficients.len() - 1;
        let regular_capacity = (1_u64 << regular_bits) - 1;
        let last_coefficient = coefficients.last().unwrap().into_inner() as u64;
        let (last_bit, remainder) = if delta <= regular_capacity {
            (false, delta)
        } else {
            (true, delta - last_coefficient)
        };
        let mut bits: Vec<_> = (0..regular_bits)
            .map(|i| ((remainder >> i) & 1) == 1)
            .collect();
        bits.push(last_bit);
        bits
    }

    fn state_with_original_value(mut state: State, target: &LogEncodeTarget, value: f64) -> State {
        state.entries.insert(target.id.into_inner(), value);
        state
    }

    fn state_with_log_bits(
        mut state: State,
        target: &LogEncodeTarget,
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
        fn log_encode_preserves_full_v3_instance_evaluation(
            (instance, target, state, delta) in arbitrary_log_encode_case()
                .prop_flat_map(|(instance, target, state)| {
                    let delta = 0..=target.width;
                    (Just(instance), Just(target), Just(state), delta)
                })
        ) {
            let decoded_value = target.lower as f64 + delta as f64;
            let expected_state = state_with_original_value(state.clone(), &target, decoded_value);
            let expected = instance.evaluate(&expected_state, ATol::default()).unwrap();

            let (coefficients, _) =
                log_encoding_coefficients(instance.decision_variables().get(&target.id).unwrap().bound(), ATol::default()).unwrap();
            let bits = log_bits_for_delta(delta, &coefficients);

            let mut encoded_instance = instance.clone();
            let encoding = encoded_instance
                .log_encode([target.id], ATol::default())
                .unwrap();
            let encoding = encoding.get(&target.id).unwrap();
            let binary_ids = sorted_log_binary_ids(&encoded_instance, encoding, target.id);
            prop_assert_eq!(binary_ids.len(), bits.len());

            let encoded_state = state_with_log_bits(state, &target, &binary_ids, &bits);
            let actual = encoded_instance.evaluate(&encoded_state, ATol::default()).unwrap();

            assert_same_observable_evaluation(&expected, &actual)?;
        }
    }

    #[test]
    fn test_log_encode_instance() {
        // Create instance with integer variable in range [2, 7]
        let mut instance = Instance::default();
        let id = VariableID::from(0);
        let var = DecisionVariable::new(
            Kind::Integer,
            Bound::new(2.0, 7.0).unwrap(),
            crate::ATol::default(),
        )
        .unwrap();
        instance
            .add_decision_variable(id, var, Default::default())
            .unwrap();

        // Perform log encoding
        let encoded = instance.log_encode([id], ATol::default()).unwrap();
        let encoded = encoded.get(&id).unwrap();

        // The original variable is still present but substituted
        assert!(instance.decision_variables.contains_key(&id));

        // Check binary variables were created with correct labels
        let store = instance.variable_labels();
        let binary_vars: Vec<_> = instance
            .decision_variables
            .iter()
            .filter(|(id, _)| {
                store.name(**id) == Some("ommx.log_encode")
                    && store.subscripts(**id).first().copied() == Some(0)
            })
            .map(|(_, dv)| dv)
            .collect();

        // For range [2, 7] (6 values), we need ceil(log2(6)) = 3 bits
        assert_eq!(binary_vars.len(), 3);

        // Check all are binary variables
        for var in &binary_vars {
            assert_eq!(var.kind(), Kind::Binary);
        }

        // Check the encoded linear expression has correct number of terms
        // Should have 3 terms for binary variables + 1 constant term
        assert_eq!(encoded.num_terms(), 4);
    }

    #[test]
    fn test_log_encoding_coefficients() {
        // 2^3 case
        let bound = Bound::new(0.0, 7.0).unwrap();
        let (coefficients, offset) = log_encoding_coefficients(bound, ATol::default()).unwrap();
        assert_eq!(coefficients, vec![coeff!(1.0), coeff!(2.0), coeff!(4.0)]);
        assert_eq!(offset, 0.0);

        // [1, 6] should be x = 1 + b1 + 2*b2 + 2*b3, the last coefficient is shifted
        // Then, 1 + 1 + 2 + 2 = 6
        let bound = Bound::new(1.0, 6.0).unwrap();
        let (coefficients, offset) = log_encoding_coefficients(bound, ATol::default()).unwrap();
        assert_eq!(coefficients, vec![coeff!(1.0), coeff!(2.0), coeff!(2.0)]);
        assert_eq!(offset, 1.0);
        assert_eq!(
            offset + coefficients.iter().map(|c| c.into_inner()).sum::<f64>(),
            6.0
        );

        let bound = Bound::new(1.000000000001, 6.000000000001).unwrap();
        let (coefficients, offset) = log_encoding_coefficients(bound, ATol::default()).unwrap();
        assert_eq!(coefficients, vec![coeff!(1.0), coeff!(2.0), coeff!(2.0)]);
        assert_eq!(offset, 1.0);

        // [2, 2] should be x = 2, no binary variables needed
        let bound = Bound::new(2.0, 2.0).unwrap();
        let (coefficients, offset) = log_encoding_coefficients(bound, ATol::default()).unwrap();
        assert!(coefficients.is_empty());
        assert_eq!(offset, 2.0);

        // No feasible integer values
        let bound = Bound::new(1.3, 1.6).unwrap();
        assert!(log_encoding_coefficients(bound, ATol::default()).is_err());
    }

    #[test]
    fn test_log_encoding_rejects_range_requiring_too_many_bits() {
        let accepted_bound = Bound::new(0.0, MAX_LOG_ENCODING_RANGE_WIDTH as f64).unwrap();
        let (coefficients, offset) =
            log_encoding_coefficients(accepted_bound, ATol::default()).unwrap();
        assert_eq!(coefficients.len(), Instance::MAX_LOG_ENCODING_BITS);
        assert_eq!(offset, 0.0);

        let rejected_upper = 2.0_f64.powi(Instance::MAX_LOG_ENCODING_BITS as i32);
        let rejected_bound = Bound::new(0.0, rejected_upper).unwrap();
        let err = log_encoding_coefficients(rejected_bound, ATol::default()).unwrap_err();
        assert!(err.to_string().contains("too large for log-encoding"));
    }

    #[test]
    fn test_log_encode_rejects_non_integer_variables() {
        let cases = [
            (Kind::Binary, Bound::of_binary()),
            (Kind::Continuous, Bound::new(0.0, 3.0).unwrap()),
            (Kind::SemiInteger, Bound::new(0.0, 3.0).unwrap()),
            (Kind::SemiContinuous, Bound::new(0.0, 3.0).unwrap()),
        ];

        for (kind, bound) in cases {
            let mut instance = Instance::default();
            let id = VariableID::from(0);
            let var = DecisionVariable::new(kind, bound, ATol::default()).unwrap();
            instance
                .add_decision_variable(id, var, Default::default())
                .unwrap();

            let err = instance.log_encode([id], ATol::default()).unwrap_err();
            assert!(err.to_string().contains("must be integer"));
            assert_eq!(aux_variable_count(&instance, "ommx.log_encode"), 0);
        }
    }

    #[test]
    fn test_log_encode_rejects_fixed_variable() {
        let id = VariableID::from(0);
        let mut instance = fixed_integer_instance(id);

        let err = instance.log_encode([id], ATol::default()).unwrap_err();
        assert!(err.to_string().contains("fixed decision variable"));
        assert_eq!(instance.fixed_decision_variable_value(id), Some(1.0));
        assert!(instance.decision_variable_dependency.get(&id).is_none());
        assert_eq!(aux_variable_count(&instance, "ommx.log_encode"), 0);
    }

    #[test]
    fn test_log_encode_rejects_indicator_variable_without_side_effects() {
        let indicator_id = VariableID::from(0);
        let body_id = VariableID::from(1);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(crate::linear!(1)))
            .decision_variables(BTreeMap::from([
                (indicator_id, DecisionVariable::binary()),
                (
                    body_id,
                    DecisionVariable::new(
                        Kind::Integer,
                        Bound::new(0.0, 3.0).unwrap(),
                        ATol::default(),
                    )
                    .unwrap(),
                ),
            ]))
            .constraints(BTreeMap::new())
            .indicator_constraints(BTreeMap::from([(
                IndicatorConstraintID::from(0),
                IndicatorConstraint::new(
                    indicator_id,
                    Equality::LessThanOrEqualToZero,
                    Function::from(crate::linear!(1)),
                ),
            )]))
            .build()
            .unwrap();

        let err = instance
            .log_encode([indicator_id], ATol::default())
            .unwrap_err();
        assert!(err.to_string().contains("must be integer"));
        assert!(instance
            .decision_variable_dependency
            .get(&indicator_id)
            .is_none());
        assert_eq!(aux_variable_count(&instance, "ommx.log_encode"), 0);
    }

    #[test]
    fn test_log_encode_rejects_one_hot_member_without_side_effects() {
        let id0 = VariableID::from(0);
        let id1 = VariableID::from(1);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(crate::linear!(0)))
            .decision_variables(BTreeMap::from([
                (id0, DecisionVariable::binary()),
                (id1, DecisionVariable::binary()),
            ]))
            .constraints(BTreeMap::new())
            .one_hot_constraints(BTreeMap::from([(
                OneHotConstraintID::from(0),
                OneHotConstraint::new(BTreeSet::from([id0, id1])).unwrap(),
            )]))
            .build()
            .unwrap();

        let err = instance.log_encode([id0], ATol::default()).unwrap_err();
        assert!(err.to_string().contains("must be integer"));
        assert!(instance.decision_variable_dependency.get(&id0).is_none());
        assert_eq!(aux_variable_count(&instance, "ommx.log_encode"), 0);
    }

    #[test]
    fn test_log_encode_is_atomic_when_substitution_fails() {
        let id = VariableID::from(0);
        let var = DecisionVariable::new(
            Kind::Integer,
            Bound::new(0.0, 3.0).unwrap(),
            ATol::default(),
        )
        .unwrap();
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(crate::linear!(0)))
            .decision_variables(BTreeMap::from([(id, var)]))
            .constraints(BTreeMap::new())
            .sos1_constraints(BTreeMap::from([(
                Sos1ConstraintID::from(0),
                Sos1Constraint::new(BTreeSet::from([id])).unwrap(),
            )]))
            .build()
            .unwrap();

        let err = instance.log_encode([id], ATol::default()).unwrap_err();
        assert!(err.to_string().contains("SOS1"));
        assert!(instance.decision_variable_dependency.get(&id).is_none());
        assert_eq!(aux_variable_count(&instance, "ommx.log_encode"), 0);
    }

    #[test]
    fn test_log_encode_is_atomic_when_later_id_fails() {
        let id0 = VariableID::from(0);
        let id1 = VariableID::from(1);
        let var0 = DecisionVariable::new(
            Kind::Integer,
            Bound::new(0.0, 3.0).unwrap(),
            ATol::default(),
        )
        .unwrap();
        let var1 = DecisionVariable::integer();
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(
                (crate::linear!(0) + crate::linear!(1)).unwrap(),
            ))
            .decision_variables(BTreeMap::from([(id0, var0), (id1, var1)]))
            .constraints(BTreeMap::new())
            .build()
            .unwrap();

        let err = instance
            .log_encode([id0, id1], ATol::default())
            .unwrap_err();
        assert!(err.to_string().contains("bound must be finite"));
        assert!(instance.decision_variable_dependency.get(&id0).is_none());
        assert!(instance.decision_variable_dependency.get(&id1).is_none());
        assert_eq!(aux_variable_count(&instance, "ommx.log_encode"), 0);
    }

    #[test]
    fn test_log_encode_validates_all_ids_before_creating_aux_variables() {
        let id0 = VariableID::from(0);
        let id1 = VariableID::from(1);
        let var0 = DecisionVariable::new(
            Kind::Integer,
            Bound::new(0.0, 3.0).unwrap(),
            ATol::default(),
        )
        .unwrap();
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(crate::linear!(0)))
            .decision_variables(BTreeMap::from([(id0, var0)]))
            .constraints(BTreeMap::new())
            .build()
            .unwrap();

        let err = instance
            .log_encode([id0, id1], ATol::default())
            .unwrap_err();
        assert!(err.to_string().contains("unknown variable"));
        assert!(instance.decision_variable_dependency.get(&id0).is_none());
        assert_eq!(aux_variable_count(&instance, "ommx.log_encode"), 0);
    }
}
