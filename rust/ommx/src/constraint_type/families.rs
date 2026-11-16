//! SDK constraint families and their collection, builder, and variable-use mappings.
//!
//! Order is significant: common validators report the first error in this order.
//! The runtime kind comes from each family's `ConstraintType::KIND`.
//! This registry generates plumbing only; roots retain validation and diagnostics.
//! Builder setters connect stage transformations to results. Usage and exclusion
//! fields retain the concrete family ID in active-variable analysis.

macro_rules! with_constraint_families {
    ($callback:path; $args:tt) => {
        $callback! {
            $args;
            regular: $crate::Constraint {
                created: constraint_collection,
                evaluated: evaluated_constraints,
                sampled: constraints,
                evaluated_builder: evaluated_constraints_collection,
                sampled_builder: constraints_collection,
                usage: used_in_regular_constraints,
                except: regular_constraints,
            }
            indicator: $crate::IndicatorConstraint {
                created: indicator_constraint_collection,
                evaluated: evaluated_indicator_constraints,
                sampled: indicator_constraints,
                evaluated_builder: evaluated_indicator_constraints_collection,
                sampled_builder: indicator_constraints_collection,
                usage: used_in_indicator_constraints,
                except: indicator_constraints,
            }
            one_hot: $crate::OneHotConstraint {
                created: one_hot_constraint_collection,
                evaluated: evaluated_one_hot_constraints,
                sampled: one_hot_constraints,
                evaluated_builder: evaluated_one_hot_constraints_collection,
                sampled_builder: one_hot_constraints_collection,
                usage: used_in_one_hot_constraints,
                except: one_hot_constraints,
            }
            sos1: $crate::Sos1Constraint {
                created: sos1_constraint_collection,
                evaluated: evaluated_sos1_constraints,
                sampled: sos1_constraints,
                evaluated_builder: evaluated_sos1_constraints_collection,
                sampled_builder: sos1_constraints_collection,
                usage: used_in_sos1_constraints,
                except: sos1_constraints,
            }
        }
    };
}

// Both result owners and collection traversal consume the same registration.
pub(crate) use with_constraint_families;
