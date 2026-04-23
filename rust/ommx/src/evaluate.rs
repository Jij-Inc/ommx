use crate::{v1::State, Sampled, VariableIDSet};

/// Evaluate with a [State]
pub trait Evaluate {
    type Output;
    type SampledOutput;

    /// Evaluate to return the output with used variable ids
    fn evaluate(&self, state: &State, atol: crate::ATol) -> crate::Result<Self::Output>;

    /// Evaluate for each sample
    fn evaluate_samples(
        &self,
        samples: &Sampled<State>,
        atol: crate::ATol,
    ) -> crate::Result<Self::SampledOutput>;

    /// Partially evaluate the function to return the used variable ids
    fn partial_evaluate(&mut self, state: &State, atol: crate::ATol) -> crate::Result<()>;

    /// Decision variable IDs required for evaluation
    fn required_ids(&self) -> VariableIDSet;
}

/// Outcome of [`Propagate::propagate`].
///
/// `self` is consumed by propagation; the variant decides where it ends up.
/// Atomicity is the caller's responsibility: on error, the constraint is lost,
/// so callers that need atomicity should clone before calling (or snapshot the
/// containing structure).
#[derive(Debug, Clone)]
pub enum PropagateOutcome<T: Propagate> {
    /// Constraint remains active (possibly shrunk / modified).
    Active(T),
    /// Constraint is fully determined by the state. Move to the removed set as-is.
    Consumed(T),
    /// Constraint transformed into another type (e.g. IndicatorConstraint → Constraint).
    /// `original` goes to the removed set, `new` is the replacement.
    Transformed { original: T, new: T::Transformed },
}

/// Unit propagation trait for constraint types.
///
/// Consumes `self` and returns [`PropagateOutcome`] together with any
/// additional variable fixings discovered during propagation.
///
/// Atomicity note: on error, `self` is lost. Callers requiring atomicity must
/// clone `self` before calling, or snapshot the containing structure.
pub trait Propagate: Sized {
    type Transformed;

    /// Propagate variable fixings from `state` through this constraint.
    ///
    /// Returns `(outcome, additional_fixings)` where `additional_fixings`
    /// contains newly discovered variable values.
    fn propagate(
        self,
        state: &State,
        atol: crate::ATol,
    ) -> crate::Result<(PropagateOutcome<Self>, State)>;
}
