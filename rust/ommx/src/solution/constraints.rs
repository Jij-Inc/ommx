use crate::{ConstraintRole, ConstraintType, EvaluatedCollection};
use std::collections::BTreeMap;

/// Query a constraint family's evaluated rows from their owning result.
///
/// The returned collection includes every retained role and belongs to this
/// host: its variable references must resolve to the host's evaluated values.
/// The host's construction and parsing APIs preserve these cross-table
/// invariants. Queries use the host's own values and saved tolerance.
///
/// [`crate::Solution`] implements this trait for each supported constraint family.
/// Generic callers of [`crate::Solution::violation_diagnostics`] use
/// `Solution: EvaluatedConstraints<C>` to express that the result stores family
/// `C`; [`ConstraintType`] itself only defines the family's types.
pub trait EvaluatedConstraints<C: ConstraintType> {
    /// Borrow all retained rows of family `C`, including inactive rows.
    fn evaluated_constraints(&self) -> &EvaluatedCollection<C>;

    /// Generate diagnostics using this result's rows, values, and saved tolerance.
    /// See [`crate::Solution::violation_diagnostics`] for role selection and feasibility semantics.
    fn violation_diagnostics(
        &self,
        min_role: ConstraintRole,
    ) -> BTreeMap<C::ID, C::ViolationDiagnostics>;
}
