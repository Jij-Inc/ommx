use super::Instance;
use crate::{ATol, Bound, Coefficient, Function, Kind, Linear, Substitute, VariableID};
use std::collections::{BTreeMap, BTreeSet};

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

    // There is only one feasible integer, and no need to encode
    if u_l == 0.0 {
        return Ok((vec![], integer_bound.lower()));
    }

    // Log-encoding: calculate number of binary variables needed
    let n = (u_l + 1.0).log2().ceil() as usize;
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
    /// Log-encode integer decision variables into binary decision variables.
    ///
    /// The instance is cloned once, every requested variable is encoded on a
    /// clone, and the result is committed back only if all encodings succeed.
    /// Duplicate IDs are encoded once. Pass a single-element iterator such as
    /// `[id]` to encode exactly one variable.
    ///
    /// `atol` is used when normalizing each decision variable bound to an
    /// integer bound.
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
        coeff, Bound, DecisionVariable, Function, Instance, Kind, Sense, Sos1Constraint,
        Sos1ConstraintID,
    };
    use std::collections::{BTreeMap, BTreeSet};

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
