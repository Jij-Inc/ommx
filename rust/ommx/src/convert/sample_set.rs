use crate::v1::{SampleSet, SampledValues, Samples, Solution, State};
use anyhow::{bail, Context, Result};
use ordered_float::OrderedFloat;
use std::collections::HashMap;

impl From<HashMap<OrderedFloat<f64>, Vec<u64>>> for SampledValues {
    fn from(map: HashMap<OrderedFloat<f64>, Vec<u64>>) -> Self {
        Self {
            entries: map
                .into_iter()
                .map(|(value, ids)| {
                    let value = value.into_inner();
                    crate::v1::sampled_values::Entry { value, ids }
                })
                .collect(),
        }
    }
}

impl FromIterator<(u64, f64)> for SampledValues {
    fn from_iter<I: IntoIterator<Item = (u64, f64)>>(iter: I) -> Self {
        let mut map: HashMap<OrderedFloat<f64>, Vec<u64>> = HashMap::new();
        for (k, v) in iter {
            map.entry(OrderedFloat(v)).or_default().push(k);
        }
        map.into()
    }
}

impl SampledValues {
    pub fn iter(&self) -> impl Iterator<Item = (&u64, &f64)> {
        self.entries
            .iter()
            .flat_map(|v| v.ids.iter().map(|id| (id, &v.value)))
    }

    pub fn get(&self, sample_id: u64) -> Option<f64> {
        for entry in &self.entries {
            if entry.ids.contains(&sample_id) {
                return Some(entry.value);
            }
        }
        None
    }
}

impl Samples {
    pub fn ids(&self) -> impl Iterator<Item = &u64> {
        self.entries.iter().flat_map(|v| v.ids.iter())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&u64, &State)> {
        self.entries.iter().flat_map(|v| {
            v.ids.iter().map(move |id| {
                (
                    id,
                    v.state
                        .as_ref()
                        .expect("ommx.v1.Samples.Entry must has state. Broken Data."),
                )
            })
        })
    }

    /// Transpose `sample_id -> decision_variable_id -> value` to `decision_variable_id -> sample_id -> value`
    pub fn transpose(&self) -> HashMap<u64, SampledValues> {
        let mut map: HashMap<u64, HashMap<OrderedFloat<f64>, Vec<u64>>> = HashMap::new();
        for (sample_id, state) in self.iter() {
            for (decision_variable_id, value) in &state.entries {
                map.entry(*decision_variable_id)
                    .or_default()
                    .entry(OrderedFloat(*value))
                    .or_default()
                    .push(*sample_id);
            }
        }
        map.into_iter().map(|(k, v)| (k, v.into())).collect()
    }

    pub fn map(&self, mut f: impl FnMut(&State) -> Result<f64>) -> Result<SampledValues> {
        Ok(SampledValues {
            entries: self
                .entries
                .iter()
                .map(|v| {
                    Ok(crate::v1::sampled_values::Entry {
                        value: f(v
                            .state
                            .as_ref()
                            .context("ommx.v1.Samples.Entry must has state. Broken Data.")?)?,
                        ids: v.ids.clone(),
                    })
                })
                .collect::<Result<_>>()?,
        })
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
