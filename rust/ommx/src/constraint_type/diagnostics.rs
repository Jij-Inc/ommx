use crate::{ATol, VariableID};

/// Family-specific diagnostic computation shared with Solution and SampleSet.
/// The result owners select violated rows and supply values from their own
/// validated tables. Keeping this contract internal prevents SDK callers from
/// combining a row with an unrelated table or tolerance.
pub(crate) trait DiagnosticRow {
    type Diagnostics;
    fn diagnose(&self, value_of: impl FnMut(VariableID) -> f64, atol: ATol) -> Self::Diagnostics;
}
