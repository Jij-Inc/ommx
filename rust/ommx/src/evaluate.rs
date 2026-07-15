use crate::{v1::State, Sampled, VariableID, VariableIDSet};

/// Caller-provided state errors detected while evaluating an OMMX domain object.
///
/// Evaluation APIs continue to return [`crate::Result`]. This signal is stored
/// in the error chain so callers can distinguish invalid state input from
/// internal evaluation failures by downcasting [`crate::Error`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EvaluationError {
    /// A function references a variable whose value is absent from the state.
    #[error("Missing entry for id: {}", id.into_inner())]
    MissingStateEntry { id: VariableID },

    /// The state contains variables that do not belong to the evaluated instance.
    #[error("state contains unknown variable IDs: {ids:?}")]
    UnknownStateEntries { ids: VariableIDSet },

    /// The state omits variables required by the evaluated instance.
    #[error("state is missing required variable IDs: {ids:?}")]
    MissingRequiredStateEntries { ids: VariableIDSet },

    /// A caller-provided state value is not finite.
    #[error(
        "state value for variable ID={} must be finite (value={value})",
        id.into_inner()
    )]
    NonFiniteStateValue { id: VariableID, value: f64 },

    /// A state value conflicts with the value fixed or derived by the instance.
    #[error(
        "state value for variable {id:?} is inconsistent with instance (state={state_value}, instance={instance_value})"
    )]
    InconsistentStateValue {
        id: VariableID,
        state_value: f64,
        instance_value: f64,
    },

    /// Evaluating a dependent variable produced a non-finite value.
    #[error("dependent variable {id:?} evaluated to non-finite value: {value}")]
    NonFiniteDependentValue { id: VariableID, value: f64 },

    /// A caller-provided dependent-variable value conflicts with its dependency.
    #[error(
        "state value for dependent variable {id:?} is inconsistent with dependency (state={state_value}, dependency={dependency_value})"
    )]
    InconsistentDependentValue {
        id: VariableID,
        state_value: f64,
        dependency_value: f64,
    },

    /// A dependent value was asserted before its dependency could be evaluated.
    #[error(
        "Dependent variable (ID={}) cannot be asserted by partial_evaluate before its dependency is fully evaluated",
        id.into_inner()
    )]
    UnverifiableDependentAssertion { id: VariableID },
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
