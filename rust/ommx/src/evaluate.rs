use crate::{v1::State, Sampled, VariableID, VariableIDSet};

/// Signal that an evaluation state omits values required by the evaluated object.
///
/// Evaluation APIs continue to return [`crate::Result`]. Callers can add the
/// missing values and retry after downcasting [`crate::Error`] to this signal.
#[derive(Debug, thiserror::Error)]
#[error("state is missing required variable IDs: {ids:?}")]
#[non_exhaustive]
pub struct MissingStateEntries {
    /// Variable IDs whose values are required for evaluation.
    pub ids: VariableIDSet,
}

/// Signal that an evaluation state contains variables outside the evaluated instance.
///
/// Callers can use the IDs to reject a state associated with another instance
/// or remove the unrelated entries before retrying.
#[derive(Debug, thiserror::Error)]
#[error("state contains unknown variable IDs: {ids:?}")]
#[non_exhaustive]
pub struct UnknownStateEntries {
    /// Variable IDs not owned by the evaluated instance.
    pub ids: VariableIDSet,
}

/// Signal that a dependent-variable assertion conflicts with its evaluated value.
///
/// The dependency is authoritative. Callers can remove or correct the asserted
/// state entry before retrying partial evaluation.
#[derive(Debug, thiserror::Error)]
#[error(
    "state value for dependent variable {id:?} is inconsistent with dependency (state={state_value}, dependency={dependency_value})"
)]
#[non_exhaustive]
pub struct InconsistentDependentValue {
    /// Dependent variable whose asserted value is inconsistent.
    pub id: VariableID,
    /// Value asserted by the caller.
    pub state_value: f64,
    /// Value computed from the dependency.
    pub dependency_value: f64,
}

/// Signal that a dependent-variable assertion cannot yet be verified.
///
/// Callers can omit the assertion or provide the remaining dependency values
/// before retrying partial evaluation.
#[derive(Debug, thiserror::Error)]
#[error(
    "Dependent variable (ID={}) cannot be asserted by partial_evaluate before its dependency is fully evaluated; missing dependency variable IDs: {required_ids:?}",
    id.into_inner()
)]
#[non_exhaustive]
pub struct UnverifiableDependentAssertion {
    /// Dependent variable whose asserted value cannot yet be verified.
    pub id: VariableID,
    /// Remaining variable IDs required to evaluate the dependency.
    pub required_ids: VariableIDSet,
}

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
