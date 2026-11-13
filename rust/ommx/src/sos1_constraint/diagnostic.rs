use crate::{ATol, VariableID, VariableIDSet};
use std::collections::BTreeMap;

/// Diagnostics for a violated SOS1 constraint at the result's saved tolerance.
///
/// [`crate::Solution::violation_diagnostics`] and
/// [`crate::SampleSet::violation_diagnostics`] return this only for violated rows.
/// The returned `values` map is nonempty and contains original signed member
/// values exceeding the tolerance in absolute value, after retaining one member
/// of largest absolute value (ties retain the largest ID). This is one possible
/// repair choice, not a uniquely intended selection.
///
/// Fields may be added in minor releases; removing or changing existing
/// information requires a major release. Mutating this detached diagnostic does
/// not change the result or its feasibility.
///
/// Diagnostics are obtained from results rather than constructed externally:
/// ```compile_fail,E0639
/// let diagnostic = ommx::Sos1Violation { values: Default::default() };
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Sos1Violation {
    /// Excess member values keyed by variable ID.
    pub values: BTreeMap<VariableID, f64>,
}

pub fn diagnose(
    variables: &VariableIDSet,
    mut value_of: impl FnMut(VariableID) -> f64,
    atol: ATol,
) -> Sos1Violation {
    let retained = variables
        .iter()
        .map(|&id| (id, value_of(id)))
        .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
        .map(|(id, _)| id);
    let values = variables
        .iter()
        .filter_map(|&id| {
            let value = value_of(id);
            (Some(id) != retained && !atol.approx_is_zero(value)).then_some((id, value))
        })
        .collect();
    Sos1Violation { values }
}
