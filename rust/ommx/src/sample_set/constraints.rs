use crate::{
    Constraint, ConstraintType, IndicatorConstraint, OneHotConstraint, SampleSet,
    SampledCollection, Sos1Constraint,
};

/// Read a constraint family's sampled rows from their owning result.
///
/// The returned collection includes every retained role and belongs to this
/// host: its sample IDs and variable references must agree with the host's
/// sampled values. The host's construction and parsing APIs preserve these
/// cross-table invariants. Reading the collection does not extract a Solution.
///
/// [`SampleSet`] implements this trait for each supported constraint family.
/// Generic callers of [`SampleSet::violation_diagnostics`] use
/// `SampleSet: SampledConstraints<C>` to express that the result stores family
/// `C`; [`ConstraintType`] itself only defines the family's types.
pub trait SampledConstraints<C: ConstraintType> {
    /// Borrow all retained rows of family `C`, including inactive rows.
    fn sampled_constraints(&self) -> &SampledCollection<C>;
}

impl SampledConstraints<Constraint> for SampleSet {
    fn sampled_constraints(&self) -> &SampledCollection<Constraint> {
        &self.constraints
    }
}

impl SampledConstraints<IndicatorConstraint> for SampleSet {
    fn sampled_constraints(&self) -> &SampledCollection<IndicatorConstraint> {
        &self.indicator_constraints
    }
}

impl SampledConstraints<OneHotConstraint> for SampleSet {
    fn sampled_constraints(&self) -> &SampledCollection<OneHotConstraint> {
        &self.one_hot_constraints
    }
}

impl SampledConstraints<Sos1Constraint> for SampleSet {
    fn sampled_constraints(&self) -> &SampledCollection<Sos1Constraint> {
        &self.sos1_constraints
    }
}
