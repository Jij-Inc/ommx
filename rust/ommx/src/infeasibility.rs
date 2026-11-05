//! Shared mathematical detection signals for analysis, presolve, and solvers.
//!
//! Producers own detection and its numerical assumptions. These signals let
//! callers select further analysis without depending on a producer's status
//! codes; they do not carry independently validated certificates.

use thiserror::Error;

use crate::{Bound, ConstraintID};

/// Reports detected infeasibility, with an extensible set of reasons.
///
/// Downcast to this type to handle infeasibility regardless of its reason.
/// Callers inspecting a particular reason must allow unknown variants and
/// additional fields. Producers can use the named constructors without
/// depending on a variant's complete representation.
///
/// ```
/// use ommx::{Bound, ConstraintID, Error, InfeasibleDetected};
///
/// let error = Error::from(InfeasibleDetected::inequality_constraint_bound(
///     ConstraintID::from(3), Bound::new(1.0, 2.0)?,
/// )).context("constraint analysis");
/// let signal = error.downcast_ref::<InfeasibleDetected>().unwrap();
/// match signal {
///     InfeasibleDetected::InequalityConstraintBound { id, bound, .. } => {
///         assert_eq!(*id, ConstraintID::from(3));
///         assert_eq!(bound.lower(), 1.0);
///     }
///     _ => { /* Handle other detected-infeasible reasons. */ }
/// }
/// # Ok::<(), ommx::Error>(())
/// ```
///
/// Exhaustive matching is intentionally rejected outside the SDK:
///
/// ```compile_fail,E0004
/// use ommx::InfeasibleDetected;
/// fn inspect(signal: &InfeasibleDetected) {
///     match signal {
///         InfeasibleDetected::InequalityConstraintBound { .. } => {}
///     }
/// }
/// ```
///
/// A known reason's fields are also open to extension:
///
/// ```compile_fail,E0638
/// use ommx::InfeasibleDetected;
/// fn inspect(signal: &InfeasibleDetected) {
///     match signal {
///         InfeasibleDetected::InequalityConstraintBound { id, bound } => {}
///         _ => {}
///     }
/// }
/// ```
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum InfeasibleDetected {
    /// A constraint's function bound establishes that its inequality is infeasible.
    #[non_exhaustive]
    #[error(
        "The bound of `f(x)` in inequality constraint({id:?}) `f(x) <= 0` is positive: {bound:?}"
    )]
    InequalityConstraintBound { id: ConstraintID, bound: Bound },
}

impl InfeasibleDetected {
    /// Report an inequality detected to be infeasible from its function bound.
    ///
    /// The producer owns the bound calculation and numerical policy establishing
    /// infeasibility. This constructor records the report; it does not validate
    /// a certificate against an Instance.
    pub fn inequality_constraint_bound(id: ConstraintID, bound: Bound) -> Self {
        Self::InequalityConstraintBound { id, bound }
    }
}

/// Reports that a model is infeasible or unbounded, without distinguishing which.
///
/// This signal is shared by solver integrations and presolve/analysis operations.
/// A producer must have established or received that disjunctive conclusion
/// under its numerical contract. An unknown status, a time limit, or failure to
/// find a feasible solution is not enough to emit it.
///
/// Callers may retain the unresolved result or explicitly select further
/// analysis or another solve to distinguish the alternatives. They must not
/// treat this as definite [`InfeasibleDetected`] or definite unboundedness.
/// Emitting the signal does not itself certify the producer's conclusion or
/// authorize transporting it to a different, unreconstructed source model.
/// Backend-specific evidence and diagnostic context remain with the producer.
///
/// The marker can be preserved in [`crate::Error`] through ordinary context:
///
/// ```
/// use ommx::{Error, InfeasibleOrUnboundedDetected};
///
/// let error = Error::from(InfeasibleOrUnboundedDetected)
///     .context("presolver returned an unresolved terminal result");
/// assert!(error.is::<InfeasibleOrUnboundedDetected>());
/// assert!(!error.is::<ommx::InfeasibleDetected>());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error(
    "The model was detected to be infeasible or unbounded; which alternative holds is unresolved"
)]
pub struct InfeasibleOrUnboundedDetected;
