use super::Instance;
use crate::{Bound, Coefficient, Linear, VariableID};

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
    ///
    /// This method performs logarithmic encoding of an integer variable into binary variables.
    /// For an integer variable with range [lower, upper], it creates ceil(log₂(range + 1)) binary
    /// variables where each binary variable represents a power of 2, allowing representation of
    /// any integer value in the original range.
    ///
    pub fn log_encode(&mut self, id: VariableID) -> Result<Linear, LogEncodingError> {
        let v = self
            .decision_variables
            .get(&id)
            .ok_or_else(|| LogEncodingError::UnknownVariable(id))?;
        let (coefficients, offset) = log_encoding_coefficients(v.bound())?;
        let mut linear = Linear::try_from(offset).unwrap();
        for c in &coefficients {
            // Create binary variables for each coefficient
            let binary_id = self.new_binary();
            todo!()
        }
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, Bound};

    #[test]
    fn test_log_encoding_coefficients() {
        // 2^3 case
        let bound = Bound::new(0.0, 7.0).unwrap();
        let (coefficients, offset) = log_encoding_coefficients(&bound).unwrap();
        assert_eq!(coefficients, vec![coeff!(1.0), coeff!(2.0), coeff!(4.0)]);
        assert_eq!(offset, 0.0);

        // [1, 6] should be x = 1 + b1 + 2*b2 + 2*b3, the last coefficient is shifted
        // Then, 1 + 1 + 2 + 2 = 6
        let bound = Bound::new(1.0, 6.0).unwrap();
        let (coefficients, offset) = log_encoding_coefficients(&bound).unwrap();
        assert_eq!(coefficients, vec![coeff!(1.0), coeff!(2.0), coeff!(2.0)]);
        assert_eq!(offset, 1.0);
        assert_eq!(
            offset + coefficients.iter().map(|c| c.into_inner()).sum::<f64>(),
            6.0
        );

        // [2, 2] should be x = 2, no binary variables needed
        let bound = Bound::new(2.0, 2.0).unwrap();
        let (coefficients, offset) = log_encoding_coefficients(&bound).unwrap();
        assert!(coefficients.is_empty());
        assert_eq!(offset, 2.0);

        // No feasible integer values
        let bound = Bound::new(1.3, 1.6).unwrap();
        assert!(log_encoding_coefficients(&bound).is_err());
    }
}
