use super::*;
use crate::{v1, ATol, Evaluate, Result, VariableIDSet};

impl<T: ConstraintType> Evaluate for ConstraintCollection<T> {
    type Output = EvaluatedCollection<T>;
    type SampledOutput = SampledCollection<T>;

    fn evaluate(&self, state: &v1::State, atol: ATol) -> Result<Self::Output> {
        let mut results = BTreeMap::new();
        let mut inactive_metadata = BTreeMap::new();
        for (id, constraint) in &self.active {
            let evaluated = constraint.evaluate(state, atol).inspect_err(|e| {
                tracing::error!(?id, error = %e, "failed to evaluate active constraint");
            })?;
            results.insert(*id, evaluated);
        }
        for (id, (constraint, reason)) in &self.inactive {
            let evaluated = constraint.evaluate(state, atol).inspect_err(|e| {
                tracing::error!(?id, error = %e, "failed to evaluate inactive constraint");
            })?;
            results.insert(*id, evaluated);
            inactive_metadata.insert(*id, reason.clone());
        }
        EvaluatedCollection::with_context(results, inactive_metadata, self.context.clone())
    }

    fn evaluate_samples(
        &self,
        samples: &crate::Sampled<v1::State>,
        atol: ATol,
    ) -> Result<Self::SampledOutput> {
        let mut results = BTreeMap::new();
        let mut inactive_metadata = BTreeMap::new();
        for (id, constraint) in &self.active {
            let evaluated = constraint.evaluate_samples(samples, atol).inspect_err(|e| {
                tracing::error!(?id, error = %e, "failed to evaluate_samples active constraint");
            })?;
            results.insert(*id, evaluated);
        }
        for (id, (constraint, reason)) in &self.inactive {
            let evaluated = constraint.evaluate_samples(samples, atol).inspect_err(|e| {
                tracing::error!(?id, error = %e, "failed to evaluate_samples inactive constraint");
            })?;
            results.insert(*id, evaluated);
            inactive_metadata.insert(*id, reason.clone());
        }
        SampledCollection::with_context(results, inactive_metadata, self.context.clone())
    }

    fn partial_evaluate(&mut self, state: &v1::State, atol: ATol) -> Result<()> {
        for (id, constraint) in self.active.iter_mut() {
            constraint.partial_evaluate(state, atol).inspect_err(|e| {
                tracing::error!(?id, error = %e, "failed to partial_evaluate constraint");
            })?;
        }
        Ok(())
    }

    /// IDs needed to evaluate every retained row, including `EvaluationOnly` rows.
    ///
    /// A collection does not populate fixed, dependent, or irrelevant values;
    /// its caller supplies the values referenced by all rows.
    fn required_ids(&self) -> VariableIDSet {
        let mut ids = VariableIDSet::default();
        for constraint in self.values() {
            ids.extend(constraint.required_ids());
        }
        ids
    }
}
