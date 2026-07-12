use num::{BigRational, Signed, ToPrimitive};

/// Exact rational used by the private proof kernel.
///
/// Every finite OMMX `f64` is interpreted as its exact IEEE-754 dyadic value;
/// no decimal approximation or tolerance is involved.
pub type ExactRational = BigRational;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ExactArithmeticError {
    #[error("proof arithmetic requires a finite IEEE-754 value")]
    NonFinite,
    #[error("exact rational is outside the finite f64 range")]
    OutsideF64Range,
}

pub fn from_f64(value: f64) -> Result<ExactRational, ExactArithmeticError> {
    ExactRational::from_float(value).ok_or(ExactArithmeticError::NonFinite)
}

/// Convert an exact value to the least finite `f64` greater than or equal to it.
///
/// This is not used while checking proofs. It is provided for later lowering
/// code that must encode an exact bound back into OMMX's `f64` coefficient
/// representation without rounding inward.
pub fn to_f64_up(value: &ExactRational) -> Result<f64, ExactArithmeticError> {
    let nearest = match value.to_f64() {
        Some(nearest) if nearest.is_finite() => nearest,
        Some(f64::NEG_INFINITY) | None if value.is_negative() => -f64::MAX,
        Some(f64::INFINITY) | None => return Err(ExactArithmeticError::OutsideF64Range),
        Some(_) => unreachable!("BigRational conversion cannot produce NaN"),
    };
    let represented = from_f64(nearest).expect("finite f64 has an exact rational image");
    let outward = if represented < *value {
        nearest.next_up()
    } else {
        nearest
    };
    outward
        .is_finite()
        .then_some(outward)
        .ok_or(ExactArithmeticError::OutsideF64Range)
}

/// Convert an exact value to the greatest finite `f64` less than or equal to it.
pub fn to_f64_down(value: &ExactRational) -> Result<f64, ExactArithmeticError> {
    let nearest = match value.to_f64() {
        Some(nearest) if nearest.is_finite() => nearest,
        Some(f64::INFINITY) | None if value.is_positive() => f64::MAX,
        Some(f64::NEG_INFINITY) | None => return Err(ExactArithmeticError::OutsideF64Range),
        Some(_) => unreachable!("BigRational conversion cannot produce NaN"),
    };
    let represented = from_f64(nearest).expect("finite f64 has an exact rational image");
    let outward = if represented > *value {
        nearest.next_down()
    } else {
        nearest
    };
    outward
        .is_finite()
        .then_some(outward)
        .ok_or(ExactArithmeticError::OutsideF64Range)
}

#[cfg(test)]
mod tests {
    use super::*;
    use num::{BigInt, One};
    use proptest::prelude::*;

    #[test]
    fn decimal_spelling_is_not_used() {
        assert_eq!(
            from_f64(0.1).unwrap(),
            BigRational::new(
                BigInt::from(3_602_879_701_896_397_i64),
                BigInt::from(36_028_797_018_963_968_i64),
            )
        );
    }

    #[test]
    fn finite_extremes_are_exact() {
        for value in [f64::from_bits(1), f64::MAX, -f64::MAX, 0.0, -0.0] {
            let exact = from_f64(value).unwrap();
            assert_eq!(exact.to_f64().unwrap(), value);
        }
        assert_eq!(from_f64(-0.0).unwrap(), BigRational::from_integer(0.into()));
    }

    #[test]
    fn non_finite_values_are_rejected() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(from_f64(value), Err(ExactArithmeticError::NonFinite));
        }
    }

    #[test]
    fn conversion_can_round_outward() {
        let one = from_f64(1.0).unwrap();
        let next = from_f64(1.0_f64.next_up()).unwrap();
        let midpoint = (one.clone() + next) / BigRational::from_integer(2.into());

        assert_eq!(to_f64_down(&midpoint).unwrap(), 1.0);
        assert_eq!(to_f64_up(&midpoint).unwrap(), 1.0_f64.next_up());

        let too_large = BigRational::from_integer(BigInt::one() << 1024usize);
        assert_eq!(to_f64_down(&too_large).unwrap(), f64::MAX);
        assert_eq!(
            to_f64_up(&too_large),
            Err(ExactArithmeticError::OutsideF64Range)
        );

        let too_negative = -too_large;
        assert_eq!(to_f64_up(&too_negative).unwrap(), -f64::MAX);
        assert_eq!(
            to_f64_down(&too_negative),
            Err(ExactArithmeticError::OutsideF64Range)
        );
    }

    proptest! {
        #[test]
        fn finite_f64_round_trips_exactly(value in any::<f64>().prop_filter(
            "proof values must be finite",
            |value| value.is_finite(),
        )) {
            let round_trip = from_f64(value).unwrap().to_f64().unwrap();
            if value == 0.0 {
                prop_assert_eq!(round_trip, 0.0);
            } else {
                prop_assert_eq!(round_trip.to_bits(), value.to_bits());
            }
        }
    }
}
