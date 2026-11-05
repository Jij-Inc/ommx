//! Shared mathematical detection signals for analysis, presolve, and solvers.
//!
//! Producers own detection and its numerical assumptions. These signals let
//! callers select further analysis without depending on a producer's status
//! codes; they do not carry independently validated certificates.

use thiserror::Error;

use crate::{Bound, ConstraintID};

/// Error type for when the instance is proofed to be infeasible
#[derive(Debug, Error)]
pub enum InfeasibleDetected {
    #[error(
        "The bound of `f(x)` in inequality constraint({id:?}) `f(x) <= 0` is positive: {bound:?}"
    )]
    InequalityConstraintBound { id: ConstraintID, bound: Bound },
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
