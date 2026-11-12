use crate::{ATol, VariableID, VariableIDSet};
use std::collections::BTreeMap;

/// Why a OneHot candidate fails at the result's retained evaluation tolerance.
///
/// Returned on demand by [`crate::Solution::violation_diagnostics`] and
/// [`crate::SampleSet::violation_diagnostics`]. Every variant contains all
/// finite member values, including exact zeros. Feasible constraints are omitted
/// from the host diagnostic map. New variants and fields may be added; consumers
/// must retain a fallback and use `..` when matching fields.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum OneHotViolation {
    /// Every member is within tolerance of zero, but the row is infeasible.
    /// No particular member is designated as the missing selection.
    #[non_exhaustive]
    MissingSelection { values: BTreeMap<VariableID, f64> },
    /// At least two members are within tolerance of one and the row is infeasible.
    /// `selected_variables` contains all such members; `values` also retains any
    /// other member errors, which can coexist with multiple selections.
    #[non_exhaustive]
    MultipleSelections {
        selected_variables: VariableIDSet,
        values: BTreeMap<VariableID, f64>,
    },
    /// Other deviations from a OneHot assignment.
    ///
    /// `target_variable` is one closest assignment's member to set to one:
    /// the largest member value, with ties resolved by the largest variable ID.
    /// It is a repair candidate, not a uniquely intended selection.
    /// `errors` contains absolute changes exceeding tolerance: distance to one
    /// for the target and distance to zero for every other member.
    #[non_exhaustive]
    ValueErrors {
        target_variable: VariableID,
        errors: BTreeMap<VariableID, f64>,
        values: BTreeMap<VariableID, f64>,
    },
}

pub fn diagnose(
    variables: &VariableIDSet,
    mut value_of: impl FnMut(VariableID) -> f64,
    atol: ATol,
) -> OneHotViolation {
    let values: BTreeMap<_, _> = variables.iter().map(|&id| (id, value_of(id))).collect();
    if values.values().all(|&value| atol.approx_is_zero(value)) {
        return OneHotViolation::MissingSelection { values };
    }
    let selected_variables: VariableIDSet = values
        .iter()
        .filter(|(_, value)| atol.approx_eq(**value, 1.0))
        .map(|(&id, _)| id)
        .collect();
    if selected_variables.len() > 1 {
        return OneHotViolation::MultipleSelections {
            selected_variables,
            values,
        };
    }
    let (&target_variable, _) = values
        .iter()
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .expect("OneHot constraints have at least one member");
    let errors = values
        .iter()
        .filter_map(|(&id, &value)| {
            let error = (value - if id == target_variable { 1.0 } else { 0.0 }).abs();
            (!atol.approx_is_zero(error)).then_some((id, error))
        })
        .collect();
    OneHotViolation::ValueErrors {
        target_variable,
        errors,
        values,
    }
}
