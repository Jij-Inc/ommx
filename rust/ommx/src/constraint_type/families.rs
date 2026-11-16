//! SDK constraint families and their collection fields/accessors on each root.
//!
//! Order is significant: common validators report the first error in this order.
//! The runtime kind comes from each family's `ConstraintType::KIND`.
//! This registry generates plumbing only; roots retain validation and diagnostics.

macro_rules! with_constraint_families {
    ($callback:path; $args:tt) => {
        $callback! {
            $args;
            regular: $crate::Constraint {
                created: constraint_collection,
                evaluated: evaluated_constraints,
                sampled: constraints,
            }
            indicator: $crate::IndicatorConstraint {
                created: indicator_constraint_collection,
                evaluated: evaluated_indicator_constraints,
                sampled: indicator_constraints,
            }
            one_hot: $crate::OneHotConstraint {
                created: one_hot_constraint_collection,
                evaluated: evaluated_one_hot_constraints,
                sampled: one_hot_constraints,
            }
            sos1: $crate::Sos1Constraint {
                created: sos1_constraint_collection,
                evaluated: evaluated_sos1_constraints,
                sampled: sos1_constraints,
            }
        }
    };
}

// Both result owners and collection traversal consume the same registration.
pub(crate) use with_constraint_families;
