use crate::{
    v1::{Samples, State},
    VariableIDSet,
};
use anyhow::Result;

/// Evaluate with a [State]
pub trait Evaluate {
    type Output;
    type SampledOutput;

    /// Evaluate to return the output with used variable ids
    fn evaluate(&self, state: &State, atol: crate::ATol) -> Result<Self::Output>;

    /// Evaluate for each sample
    fn evaluate_samples(&self, samples: &Samples, atol: crate::ATol)
        -> Result<Self::SampledOutput>;

    /// Partially evaluate the function to return the used variable ids
    fn partial_evaluate(&mut self, state: &State, atol: crate::ATol) -> Result<()>;

    /// Decision variable IDs required for evaluation
    fn required_ids(&self) -> VariableIDSet;
}
