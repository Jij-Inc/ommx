use crate::v1::{SampleSet, SampledValues, Samples, Solution, State};
use anyhow::{bail, Context, Result};
use std::collections::HashMap;

impl FromIterator<(u64, f64)> for SampledValues {
    fn from_iter<I: IntoIterator<Item = (u64, f64)>>(iter: I) -> Self {
        Self {
            values: iter.into_iter().collect(),
        }
    }
}

impl IntoIterator for SampledValues {
    type Item = (u64, f64);
    type IntoIter = std::collections::hash_map::IntoIter<u64, f64>;

    fn into_iter(self) -> Self::IntoIter {
        self.values.into_iter()
    }
}

impl SampledValues {
    pub fn iter(&self) -> impl Iterator<Item = (&u64, &f64)> {
        self.values.iter()
    }

    pub fn get(&self, sample_id: u64) -> Option<f64> {
        self.values.get(&sample_id).cloned()
    }
}

impl Samples {
    pub fn iter(&self) -> impl Iterator<Item = (&u64, &State)> {
        self.states.iter()
    }

    /// Transpose `sample_id -> decision_variable_id -> value` to `decision_variable_id -> sample_id -> value`
    pub fn transpose(&self) -> HashMap<u64, SampledValues> {
        let mut out = HashMap::new();
        for (sample_id, state) in self.iter() {
            for (decision_variable_id, value) in state.entries.iter() {
                out.entry(*decision_variable_id)
                    .or_insert_with(SampledValues::default)
                    .values
                    .insert(*sample_id, *value);
            }
        }
        out
    }
}

impl SampleSet {
    pub fn get(&self, sample_id: u64) -> Result<Solution> {
        let mut decision_variables = Vec::new();
        let mut state = State::default();

        let evaluated_constraints = self
            .constraints
            .iter()
            .map(|c| c.get(sample_id))
            .collect::<Result<Vec<_>>>()?;

        for sampled in &self.decision_variables {
            let v = sampled
                .decision_variable
                .clone()
                .context("SampledDecisionVariable lacks decision_variable")?;
            if let Some(value) = v.substituted_value {
                state.entries.insert(v.id, value);
            } else if let Some(value) = sampled.samples.as_ref().and_then(|s| s.get(sample_id)) {
                state.entries.insert(v.id, value);
            } else {
                bail!("Missing value for decision_variable with ID={}", v.id);
            }
            decision_variables.push(v);
        }

        Ok(Solution {
            state: Some(state),
            objective: self
                .objectives
                .as_ref()
                .context("SampleSet lacks objectives")?
                .get(sample_id)
                .with_context(|| {
                    format!("SampleSet lacks objective for sample with ID={}", sample_id)
                })?,
            decision_variables,
            feasible: *self.feasible.get(&sample_id).with_context(|| {
                format!(
                    "SampleSet lacks feasibility for sample with ID={}",
                    sample_id
                )
            })?,
            evaluated_constraints,
            optimality: Default::default(),
            relaxation: Default::default(),
        })
    }
}
