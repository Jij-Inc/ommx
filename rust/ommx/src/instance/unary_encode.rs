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
fn unary_encoding_size(bound: Bound) -> crate::Result<(usize, f64)> {
    if !bound.lower().is_finite() || !bound.upper().is_finite() {
        crate::bail!({ ?bound }, "bound must be finite for unary-encoding: {bound}");
    }

    // Bound of integer may be non-integer value, so floor/ceil to get valid integer range.
    let upper = bound.upper().floor();
    let lower = bound.lower().ceil();
    let width = upper - lower;
    if width < 0.0 {
        crate::bail!({ ?bound }, "no feasible integer values in bound for unary-encoding: {bound}");
    }

    if width > usize::MAX as f64 {
        crate::bail!(
            { ?bound, width },
            "range is too large for unary-encoding: {width}"
        );
    }

    Ok((width as usize, lower))
}

impl Instance {
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
        let v = self
            .decision_variables
            .get(&id)
            .ok_or_else(|| crate::error!({ ?id }, "unknown variable for unary-encoding: {id:?}"))?;
        let (num_binary_variables, offset) = unary_encoding_size(v.bound())?;

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
                ATol::default(),
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
    use crate::{coeff, Bound, DecisionVariable, Instance, Kind, LinearMonomial};

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
        let (num_binary_variables, offset) = unary_encoding_size(bound).unwrap();
        assert_eq!(num_binary_variables, 3);
        assert_eq!(offset, 0.0);

        let bound = Bound::new(1.0, 6.0).unwrap();
        let (num_binary_variables, offset) = unary_encoding_size(bound).unwrap();
        assert_eq!(num_binary_variables, 5);
        assert_eq!(offset, 1.0);

        let bound = Bound::new(2.0, 2.0).unwrap();
        let (num_binary_variables, offset) = unary_encoding_size(bound).unwrap();
        assert_eq!(num_binary_variables, 0);
        assert_eq!(offset, 2.0);

        let bound = Bound::new(1.3, 1.6).unwrap();
        assert!(unary_encoding_size(bound).is_err());
    }
}
