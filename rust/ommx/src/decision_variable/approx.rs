use super::*;
use crate::ATol;
use ::approx::AbsDiffEq;

impl AbsDiffEq for DecisionVariable {
    type Epsilon = ATol;

    fn default_epsilon() -> Self::Epsilon {
        crate::ATol::default()
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        // Must be same ID
        if self.id != other.id {
            return false;
        }
        // For different bounds, they are always different.
        if !self.bound.abs_diff_eq(&other.bound, epsilon) {
            return false;
        }
        // If kinds are same and bounds are same, they are equal.
        if self.kind == other.kind {
            return true;
        }

        // Hereafter, bounds are same, but kinds are different.
        // We may consider them mathematically equal in several cases.

        // We regard point bound Continuous[a, a] and Integer[a, a] are identical mathematically.
        if let Some(lower) = self.bound.is_point(epsilon) {
            // If both are point bounds and they are [0, 0], they are considered equal for any kind.
            if lower.abs() < epsilon {
                return true;
            }

            // If a != 0, binary, integer, continuous are considered equal,
            // but semi-continuous and semi-integer are not because they can be a or 0.
            return same_kind_class(
                self.kind,
                other.kind,
                &[Kind::Binary, Kind::Integer, Kind::Continuous],
                &[Kind::SemiContinuous, Kind::SemiInteger],
            );
        }
        if self.bound.contains(0.0, epsilon) {
            // Bound contains 0, so semi-integer and semi-continuous are equal to integer and continuous.
            same_kind_class(
                self.kind,
                other.kind,
                &[Kind::Binary, Kind::Integer, Kind::SemiInteger],
                &[Kind::Continuous, Kind::SemiContinuous],
            )
        } else {
            // Only binary and integer are considered equal.
            match (self.kind, other.kind) {
                (Kind::Binary, Kind::Integer) | (Kind::Integer, Kind::Binary) => true,
                _ => false,
            }
        }
    }
}

fn same_kind_class(a: Kind, b: Kind, class1: &[Kind], class2: &[Kind]) -> bool {
    (class1.contains(&a) && class1.contains(&b)) || (class2.contains(&a) && class2.contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ATol, Bound, DecisionVariable, VariableID};
    use ::approx::{assert_abs_diff_eq, assert_abs_diff_ne};

    #[test]
    fn test_different_ids_not_equal() {
        // Variables with different IDs are never equal
        let var1 = DecisionVariable::new(
            VariableID::from(1),
            Kind::Continuous,
            Bound::new(0.0, 1.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        let var2 = DecisionVariable::new(
            VariableID::from(2),
            Kind::Continuous,
            Bound::new(0.0, 1.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        assert_abs_diff_ne!(var1, var2);
    }

    #[test]
    fn test_different_bounds_not_equal() {
        // Variables with different bounds are not equal
        let var1 = DecisionVariable::new(
            VariableID::from(1),
            Kind::Continuous,
            Bound::new(0.0, 1.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        let var2 = DecisionVariable::new(
            VariableID::from(1),
            Kind::Continuous,
            Bound::new(0.0, 2.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        assert_abs_diff_ne!(var1, var2);
    }

    #[test]
    fn test_same_kind_and_bound_equal() {
        // Variables with same ID, kind, and bounds are equal
        let var1 = DecisionVariable::new(
            VariableID::from(1),
            Kind::Integer,
            Bound::new(0.0, 10.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        let var2 = DecisionVariable::new(
            VariableID::from(1),
            Kind::Integer,
            Bound::new(0.0, 10.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        assert_abs_diff_eq!(var1, var2);
    }

    #[test]
    fn test_point_bound_continuous_integer_equal() {
        // Continuous[a, a] and Integer[a, a] are equal for point bounds
        let var1 = DecisionVariable::new(
            VariableID::from(1),
            Kind::Continuous,
            Bound::new(5.0, 5.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        let var2 = DecisionVariable::new(
            VariableID::from(1),
            Kind::Integer,
            Bound::new(5.0, 5.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        assert_abs_diff_eq!(var1, var2);
    }

    #[test]
    fn test_point_bound_zero_all_kinds_equal() {
        // For point bound [0, 0], all kinds are considered equal
        let continuous = DecisionVariable::new(
            VariableID::from(1),
            Kind::Continuous,
            Bound::new(0.0, 0.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        let integer = DecisionVariable::new(
            VariableID::from(1),
            Kind::Integer,
            Bound::new(0.0, 0.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        let binary = DecisionVariable::new(
            VariableID::from(1),
            Kind::Binary,
            Bound::new(0.0, 0.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        let semi_continuous = DecisionVariable::new(
            VariableID::from(1),
            Kind::SemiContinuous,
            Bound::new(0.0, 0.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        let semi_integer = DecisionVariable::new(
            VariableID::from(1),
            Kind::SemiInteger,
            Bound::new(0.0, 0.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        // All combinations should be equal
        assert_abs_diff_eq!(continuous, integer);
        assert_abs_diff_eq!(continuous, binary);
        assert_abs_diff_eq!(continuous, semi_continuous);
        assert_abs_diff_eq!(continuous, semi_integer);
        assert_abs_diff_eq!(integer, semi_integer);
    }

    #[test]
    fn test_point_bound_nonzero_semi_not_equal() {
        // For point bound [a, a] where a != 0, semi-continuous/semi-integer
        // are not equal to continuous/integer/binary
        let continuous = DecisionVariable::new(
            VariableID::from(1),
            Kind::Continuous,
            Bound::new(5.0, 5.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        let semi_continuous = DecisionVariable::new(
            VariableID::from(1),
            Kind::SemiContinuous,
            Bound::new(5.0, 5.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        assert_abs_diff_ne!(continuous, semi_continuous);
    }

    #[test]
    fn test_bound_contains_zero_semi_equal() {
        // When bound contains 0, semi-integer equals integer and semi-continuous equals continuous
        let integer = DecisionVariable::new(
            VariableID::from(1),
            Kind::Integer,
            Bound::new(-5.0, 5.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        let semi_integer = DecisionVariable::new(
            VariableID::from(1),
            Kind::SemiInteger,
            Bound::new(-5.0, 5.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        let continuous = DecisionVariable::new(
            VariableID::from(1),
            Kind::Continuous,
            Bound::new(-5.0, 5.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        let semi_continuous = DecisionVariable::new(
            VariableID::from(1),
            Kind::SemiContinuous,
            Bound::new(-5.0, 5.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        assert_abs_diff_eq!(integer, semi_integer);
        assert_abs_diff_eq!(continuous, semi_continuous);

        // Test binary-integer equality with compatible bounds
        // Binary is always [0, 1], so test with integer that has same bound
        let binary = DecisionVariable::new(
            VariableID::from(1),
            Kind::Binary,
            Bound::new(0.0, 1.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        let integer_01 = DecisionVariable::new(
            VariableID::from(1),
            Kind::Integer,
            Bound::new(0.0, 1.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        assert_abs_diff_eq!(binary, integer_01);
    }

    #[test]
    fn test_point_bound_nonzero_all_basic_kinds_equal() {
        // For point bound [a, a] where a != 0, binary, integer, and continuous are all equal
        // This is according to the logic in lines 36-43 of the implementation
        let binary = DecisionVariable::new(
            VariableID::from(1),
            Kind::Binary,
            Bound::new(1.0, 1.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        let integer = DecisionVariable::new(
            VariableID::from(1),
            Kind::Integer,
            Bound::new(1.0, 1.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        let continuous = DecisionVariable::new(
            VariableID::from(1),
            Kind::Continuous,
            Bound::new(1.0, 1.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        // For point bound [1,1] (not zero), binary, integer, and continuous are all equal
        assert_abs_diff_eq!(binary, integer);
        assert_abs_diff_eq!(binary, continuous);
        assert_abs_diff_eq!(integer, continuous);
    }


    #[test]
    fn test_tolerance_in_point_bound() {
        // Test that tolerance is considered for point bounds
        let var1 = DecisionVariable::new(
            VariableID::from(1),
            Kind::Continuous,
            Bound::new(1.0, 1.0 + 1e-10).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        let var2 = DecisionVariable::new(
            VariableID::from(1),
            Kind::Integer,
            Bound::new(1.0, 1.0 + 1e-10).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        // Should be equal with default tolerance
        assert_abs_diff_eq!(var1, var2);
    }
}
