use crate::{
    Constraint, ConstraintType, EvaluatedCollection, IndicatorConstraint, OneHotConstraint,
    Solution, Sos1Constraint,
};

/// Read a constraint family's evaluated rows from their owning result.
///
/// The returned collection includes every retained role and belongs to this
/// host: its variable references must resolve to the host's evaluated values.
/// The host's construction and parsing APIs preserve these cross-table
/// invariants. This trait only exposes a shared borrow of that collection.
///
/// [`Solution`] implements this trait for each supported constraint family.
/// Generic callers of [`Solution::violation_diagnostics`] use
/// `Solution: EvaluatedConstraints<C>` to express that the result stores family
/// `C`; [`ConstraintType`] itself only defines the family's types.
pub trait EvaluatedConstraints<C: ConstraintType> {
    /// Borrow all retained rows of family `C`, including inactive rows.
    fn evaluated_constraints(&self) -> &EvaluatedCollection<C>;
}

impl EvaluatedConstraints<Constraint> for Solution {
    fn evaluated_constraints(&self) -> &EvaluatedCollection<Constraint> {
        &self.evaluated_constraints
    }
}

impl EvaluatedConstraints<IndicatorConstraint> for Solution {
    fn evaluated_constraints(&self) -> &EvaluatedCollection<IndicatorConstraint> {
        &self.evaluated_indicator_constraints
    }
}

impl EvaluatedConstraints<OneHotConstraint> for Solution {
    fn evaluated_constraints(&self) -> &EvaluatedCollection<OneHotConstraint> {
        &self.evaluated_one_hot_constraints
    }
}

impl EvaluatedConstraints<Sos1Constraint> for Solution {
    fn evaluated_constraints(&self) -> &EvaluatedCollection<Sos1Constraint> {
        &self.evaluated_sos1_constraints
    }
}
