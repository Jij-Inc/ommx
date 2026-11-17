use super::Bound;
use proptest::prelude::*;

/// Validated parameters for generating integer-valued `f64` test values.
///
/// [`Self::new`] validates that the bound contains an integer and the scale is
/// positive. The private fields retain the inward-rounded integer bound and
/// validated scale, so [`Self::strategy`] is infallible and reusable.
///
/// Values use the same representation as [`crate::v1::State`], including
/// representable integers outside `i64`. The generator does not sample all
/// mathematical integers uniformly. Finite intervals are not clipped to the
/// scale; infinite intervals use the window semantics of
/// [`Bound::arbitrary_containing`]. Values with overflowing bound residuals are
/// excluded consistently with domain validation.
#[derive(Debug, Clone, Copy)]
pub struct IntegerValueParameters {
    bound: Bound,
    scale: u64,
}

impl IntegerValueParameters {
    /// Validate parameters before creating an integer-value strategy.
    ///
    /// # Errors
    /// Returns an error if `scale` is zero or `bound` contains no integer.
    ///
    /// ```
    /// use ommx::{Bound, IntegerValueParameters, random::sample_deterministic};
    ///
    /// let parameters = IntegerValueParameters::new(Bound::new(101.0, f64::INFINITY)?, 100)?;
    /// let value = sample_deterministic(parameters.strategy());
    /// assert!(value.is_finite() && value >= 101.0 && value == value.round());
    ///
    /// assert!(IntegerValueParameters::new(Bound::new(0.1, 0.9)?, 100).is_err());
    /// # Ok::<(), ommx::Error>(())
    /// ```
    pub fn new(bound: Bound, scale: u64) -> crate::Result<Self> {
        crate::ensure!(scale > 0, "Integer-value generation scale must be positive");
        let integer_bound =
            Bound::new(bound.lower().ceil(), bound.upper().floor()).map_err(|_| {
                crate::error!(
                    "Cannot generate integer values within {bound}: no integer in the bound"
                )
            })?;
        Ok(Self {
            bound: integer_bound,
            scale,
        })
    }

    /// Create a test-value strategy from already validated parameters.
    /// Distributions and generated sequences may change between versions.
    pub fn strategy(&self) -> BoxedStrategy<f64> {
        self.bound
            .arbitrary_containing(self.scale as f64)
            .prop_map(f64::round)
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::{sample, Rng};

    #[test]
    fn rejects_empty_integer_domains_during_parameter_construction() {
        for (lower, upper) in [(0.1, 0.9), (-0.9, -0.1), (101.1, 101.9)] {
            let error =
                IntegerValueParameters::new(Bound::new(lower, upper).unwrap(), 100).unwrap_err();
            assert!(error.to_string().contains("no integer in the bound"));
        }
    }

    #[test]
    fn rejects_zero_scale_before_strategy_creation() {
        for bound in [Bound::default(), Bound::new(101.0, 102.0).unwrap()] {
            let error = IntegerValueParameters::new(bound, 0).unwrap_err();
            assert!(error.to_string().contains("scale must be positive"));
        }
    }

    #[test]
    fn validated_parameters_are_reusable_at_scale_and_representation_limits() {
        let mut rng = Rng::deterministic();
        for bound in [
            Bound::default(),
            Bound::new(101.0, f64::INFINITY).unwrap(),
            Bound::new(f64::NEG_INFINITY, -101.0).unwrap(),
            Bound::new(101.2, 102.8).unwrap(),
            Bound::new(f64::MAX, f64::INFINITY).unwrap(),
            Bound::new(-f64::MAX, f64::MAX).unwrap(),
        ] {
            for scale in [1, 100, u64::MAX] {
                let parameters = IntegerValueParameters::new(bound, scale).unwrap();
                for _ in 0..16 {
                    let value = sample(&mut rng, parameters.strategy());
                    assert!(value.is_finite());
                    assert_eq!(value, value.round());
                    assert!(value >= bound.lower() && value <= bound.upper());
                    assert!(bound.contains(value, crate::ATol::default()));
                }
            }
        }
    }
}
