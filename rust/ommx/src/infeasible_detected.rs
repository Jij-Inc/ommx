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
