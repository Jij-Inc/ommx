use crate::{ATol, VariableID, VariableIDSet};
use std::collections::BTreeMap;

/// Why a OneHot candidate fails at the result's retained evaluation tolerance.
///
/// Returned on demand by [`crate::Solution::violation_diagnostics`] and
/// [`crate::SampleSet::violation_diagnostics`]. Every variant contains all
/// finite member values, including exact zeros. Feasible constraints are omitted
/// from the host diagnostic map. Minor releases may add variants; consumers
/// must retain a fallback when matching the enum. Existing variants have fixed
/// fields. Adding or changing their fields, or removing or changing existing
/// information, requires a major release.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum OneHotViolationDiagnostics {
    /// Every member is within tolerance of zero, but the row is infeasible.
    /// No particular member is designated as the missing selection.
    MissingSelection { values: BTreeMap<VariableID, f64> },
    /// At least two members are within tolerance of one and the row is infeasible.
    /// `selected_variables` contains all such members; `values` also retains any
    /// other member errors, which can coexist with multiple selections.
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
    ValueErrors {
        target_variable: VariableID,
        errors: BTreeMap<VariableID, f64>,
        values: BTreeMap<VariableID, f64>,
    },
}

fn diagnose(
    variables: &VariableIDSet,
    mut value_of: impl FnMut(VariableID) -> f64,
    atol: ATol,
) -> OneHotViolationDiagnostics {
    let values: BTreeMap<_, _> = variables.iter().map(|&id| (id, value_of(id))).collect();
    if values.values().all(|&value| atol.approx_is_zero(value)) {
        return OneHotViolationDiagnostics::MissingSelection { values };
    }
    let selected_variables: VariableIDSet = values
        .iter()
        .filter(|(_, value)| atol.approx_eq(**value, 1.0))
        .map(|(&id, _)| id)
        .collect();
    if selected_variables.len() > 1 {
        return OneHotViolationDiagnostics::MultipleSelections {
            selected_variables,
            values,
        };
    }
    let target_variable =
        super::select_one_hot_member(values.iter().map(|(&id, &value)| (id, value)))
            .expect("OneHot constraints have at least one member");
    let errors = values
        .iter()
        .filter_map(|(&id, &value)| {
            let error = (value - if id == target_variable { 1.0 } else { 0.0 }).abs();
            (!atol.approx_is_zero(error)).then_some((id, error))
        })
        .collect();
    OneHotViolationDiagnostics::ValueErrors {
        target_variable,
        errors,
        values,
    }
}

impl<S: crate::Stage<Self>> crate::constraint_type::DiagnosticRow for super::OneHotConstraint<S> {
    type Diagnostics = OneHotViolationDiagnostics;

    fn diagnose(&self, value_of: impl FnMut(VariableID) -> f64, atol: ATol) -> Self::Diagnostics {
        diagnose(&self.variables, value_of, atol)
    }
}
