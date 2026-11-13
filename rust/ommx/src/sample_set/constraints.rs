use crate::{ConstraintRole, ConstraintType, SampleID, SampledCollection};
use std::collections::BTreeMap;

/// Query a constraint family's sampled rows from their owning result.
///
/// The returned collection includes every retained role and belongs to this
/// host: its sample IDs and variable references must agree with the host's
/// sampled values. The host's construction and parsing APIs preserve these
/// cross-table invariants. Reading the collection does not extract a Solution.
///
/// [`crate::SampleSet`] implements this trait for each supported constraint family.
/// Generic callers of [`crate::SampleSet::violation_diagnostics`] use
/// `SampleSet: SampledConstraints<C>` to express that the result stores family
/// `C`; [`ConstraintType`] itself only defines the family's types.
pub trait SampledConstraints<C: ConstraintType> {
    /// Borrow all retained rows of family `C`, including inactive rows.
    fn sampled_constraints(&self) -> &SampledCollection<C>;

    /// Generate diagnostics for a sample using this result's rows and values.
    /// See [`crate::SampleSet::violation_diagnostics`] for role selection and feasibility semantics.
    ///
    /// # Errors
    /// Returns [`crate::SampleSetError::UnknownSampleID`] if the sample is absent,
    /// including when this family has no rows.
    fn violation_diagnostics(
        &self,
        sample_id: SampleID,
        min_role: ConstraintRole,
    ) -> crate::Result<BTreeMap<C::ID, C::ViolationDiagnostics>>;
}
