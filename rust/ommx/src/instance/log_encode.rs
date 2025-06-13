use super::Instance;
use crate::{polynomial_base::Linear, Bound, Coefficient, VariableID};
use anyhow::{bail, ensure, Result};

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
fn log_encoding_coefficients(bound: &Bound) -> Result<(Vec<Coefficient>, f64)> {
    // Check bounds are finite
    if !bound.lower().is_finite() || !bound.upper().is_finite() {
        bail!(
            "Bound must be finite for log-encoding: lower={}, upper={}",
            bound.lower(),
            bound.upper()
        );
    }

    // Bound of integer may be non-integer value, so floor/ceil to get valid integer range
    let upper = bound.upper().floor();
    let lower = bound.lower().ceil();
    let u_l = upper - lower;

    ensure!(
        u_l >= 0.0,
        "No feasible integer found in the bound: lower={}, upper={}",
        bound.lower(),
        bound.upper()
    );

    // There is only one feasible integer, and no need to encode
    if u_l == 0.0 {
        return Ok((vec![], lower));
    }

    // Log-encoding: calculate number of binary variables needed
    let n = (u_l + 1.0).log2().ceil() as usize;

    let mut coefficients = Vec::new();

    // Calculate coefficient for each binary variable
    for i in 0..n {
        let coeff_value = if i == n - 1 {
            // Last binary variable gets special coefficient to handle exact range
            u_l - 2.0f64.powi(i as i32) + 1.0
        } else {
            // Other variables get power of 2 coefficients
            2.0f64.powi(i as i32)
        };

        // Convert to Coefficient, handling potential zero coefficients
        match Coefficient::try_from(coeff_value) {
            Ok(coeff) => coefficients.push(coeff),
            Err(_) => {
                // Skip zero coefficients
                // This shouldn't happen in log-encoding, but handle gracefully
                continue;
            }
        }
    }

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
    pub fn log_encode(&mut self, _id: VariableID) -> Result<Linear> {
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
