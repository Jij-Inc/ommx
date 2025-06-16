use super::Instance;
use crate::{substitute_one, Bound, Coefficient, Linear, VariableID};

#[derive(Debug, thiserror::Error)]
pub enum LogEncodingError {
    #[error("Unknown variable with ID: {0}")]
    UnknownVariable(VariableID),
    #[error("Bound must be finite for log-encoding: {0}")]
    NonFiniteBound(Bound),
    #[error("No feasible integer values found in the bound: {0}")]
    NoFeasibleInteger(Bound),
}

/// Calculate log-encoding coefficients for a given bound.
///
/// This function computes the coefficients needed for logarithmic encoding
/// of an integer variable with the given bounds without modifying any Instance.
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
/// Returns [`anyhow::Error`] in the following cases:
/// - The bound is not finite
/// - No feasible integer values exist within the bounds
///
/// # Examples
///
/// ```rust
/// use ommx::{coeff, Bound};
///
/// ```
fn log_encoding_coefficients(bound: Bound) -> Result<(Vec<Coefficient>, f64), LogEncodingError> {
    // Check bounds are finite
    if !bound.lower().is_finite() || !bound.upper().is_finite() {
        return Err(LogEncodingError::NonFiniteBound(bound.clone()));
    }

    // Bound of integer may be non-integer value, so floor/ceil to get valid integer range
    let upper = bound.upper().floor();
    let lower = bound.lower().ceil();
    let u_l = upper - lower;
    if u_l < 0.0 {
        // No feasible integer values in the range
        return Err(LogEncodingError::NoFeasibleInteger(bound.clone()));
    }

    // There is only one feasible integer, and no need to encode
    if u_l == 0.0 {
        return Ok((vec![], lower));
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

    Ok((coefficients, lower))
}

impl Instance {
    /// Encode an integer decision variable into binary decision variables.
    pub fn log_encode(&mut self, id: VariableID) -> Result<Linear, LogEncodingError> {
        let v = self
            .decision_variables
            .get(&id)
            .ok_or_else(|| LogEncodingError::UnknownVariable(id))?;
        let (coefficients, offset) = log_encoding_coefficients(v.bound())?;
        let mut linear = Linear::try_from(offset).unwrap();
        for (i, coefficient) in coefficients.iter().enumerate() {
            // Create binary variables for each coefficient
            let binary = self.new_binary();
            binary.name = Some("ommx.log_encode".to_string());
            binary.subscripts = vec![id.into_inner() as i64, i as i64];
            linear.add_term(binary.id().into(), *coefficient);
        }
        let f = linear.clone().into();
        substitute_one(self, id, &f).unwrap();
        Ok(linear)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, Bound, DecisionVariable, Instance, Kind};

    #[test]
    fn test_log_encode_instance() {
        // Create instance with integer variable in range [2, 7]
        let mut instance = Instance::default();
        let id = VariableID::from(0);
        let var = DecisionVariable::new(
            id,
            Kind::Integer,
            Bound::new(2.0, 7.0).unwrap(),
            None,
            crate::ATol::default(),
        )
        .unwrap();
        instance.decision_variables.insert(id, var);

        // Perform log encoding
        let encoded = instance.log_encode(id).unwrap();

        // The original variable is still present but substituted
        assert!(instance.decision_variables.contains_key(&id));

        // Check binary variables were created with correct metadata
        let binary_vars: Vec<_> = instance
            .decision_variables
            .values()
            .filter(|dv| dv.name == Some("ommx.log_encode".to_string()) && dv.subscripts[0] == 0)
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
        let (coefficients, offset) = log_encoding_coefficients(bound).unwrap();
        assert_eq!(coefficients, vec![coeff!(1.0), coeff!(2.0), coeff!(4.0)]);
        assert_eq!(offset, 0.0);

        // [1, 6] should be x = 1 + b1 + 2*b2 + 2*b3, the last coefficient is shifted
        // Then, 1 + 1 + 2 + 2 = 6
        let bound = Bound::new(1.0, 6.0).unwrap();
        let (coefficients, offset) = log_encoding_coefficients(bound).unwrap();
        assert_eq!(coefficients, vec![coeff!(1.0), coeff!(2.0), coeff!(2.0)]);
        assert_eq!(offset, 1.0);
        assert_eq!(
            offset + coefficients.iter().map(|c| c.into_inner()).sum::<f64>(),
            6.0
        );

        // [2, 2] should be x = 2, no binary variables needed
        let bound = Bound::new(2.0, 2.0).unwrap();
        let (coefficients, offset) = log_encoding_coefficients(bound).unwrap();
        assert!(coefficients.is_empty());
        assert_eq!(offset, 2.0);

        // No feasible integer values
        let bound = Bound::new(1.3, 1.6).unwrap();
        assert!(log_encoding_coefficients(bound).is_err());
    }
}
