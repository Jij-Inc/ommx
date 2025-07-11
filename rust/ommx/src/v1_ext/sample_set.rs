use crate::v1::{
    instance::Sense, sampled_values::SampledValuesEntry, samples::SamplesEntry, SampleSet,
    SampledValues, Samples, Solution, State,
};
use anyhow::{bail, ensure, Context, Result};
use approx::AbsDiffEq;
use ordered_float::OrderedFloat;
use std::collections::{BTreeSet, HashMap};

impl From<HashMap<OrderedFloat<f64>, Vec<u64>>> for SampledValues {
    fn from(map: HashMap<OrderedFloat<f64>, Vec<u64>>) -> Self {
        Self {
            entries: map
                .into_iter()
                .map(|(value, ids)| {
                    let value = value.into_inner();
                    SampledValuesEntry { value, ids }
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

impl AbsDiffEq for SampledValues {
    type Epsilon = crate::ATol;

    fn default_epsilon() -> Self::Epsilon {
        crate::ATol::default()
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        let map: HashMap<u64, f64> = self.iter().map(|(k, v)| (*k, *v)).collect();
        for (k, v) in other.iter() {
            if let Some(v2) = map.get(k) {
                if !v.abs_diff_eq(v2, *epsilon) {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    }
}

impl SampledValues {
    pub fn constants(ids: impl Iterator<Item = u64>, value: f64) -> Self {
        Self {
            entries: vec![SampledValuesEntry {
                value,
                ids: ids.collect(),
            }],
        }
    }

    pub fn zeros(ids: impl Iterator<Item = u64>) -> Self {
        Self::constants(ids, 0.0)
    }

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

    pub fn len(&self) -> usize {
        self.entries.iter().map(|v| v.ids.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Samples {
    pub fn add_sample(&mut self, sample_id: u64, state: State) {
        let entry = self
            .entries
            .iter_mut()
            .find(|v| v.state.as_ref() == Some(&state));
        match entry {
            Some(entry) => entry.ids.push(sample_id),
            None => {
                self.entries.push(SamplesEntry {
                    state: Some(state),
                    ids: vec![sample_id],
                });
            }
        }
    }

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

    pub fn states_mut(&mut self) -> impl Iterator<Item = Result<&mut State>> {
        self.entries.iter_mut().map(|v| {
            v.state
                .as_mut()
                .context("ommx.v1.Samples.Entry must has state. Broken Data.")
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
                    Ok(SampledValuesEntry {
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
    fn objectives(&self) -> Result<&SampledValues> {
        self.objectives
            .as_ref()
            .context("SampleSet lacks objectives")
    }

    pub fn feasible_relaxed(&self) -> &HashMap<u64, bool> {
        if self.feasible_relaxed.is_empty() {
            &self.feasible
        } else {
            &self.feasible_relaxed
        }
    }

    pub fn feasible_unrelaxed(&self) -> &HashMap<u64, bool> {
        if self.feasible_relaxed.is_empty() {
            #[allow(deprecated)]
            &self.feasible_unrelaxed
        } else {
            &self.feasible
        }
    }

    pub fn num_samples(&self) -> Result<usize> {
        let objectives = self.objectives()?;
        ensure!(
            objectives.len() == self.feasible_relaxed().len()
                && objectives.len() == self.feasible_unrelaxed().len(),
            "SampleSet has inconsistent number of objectives and feasibility"
        );
        Ok(objectives.len())
    }

    pub fn sample_ids(&self) -> BTreeSet<u64> {
        self.feasible_relaxed().keys().cloned().collect()
    }

    pub fn feasible_ids(&self) -> BTreeSet<u64> {
        self.feasible_relaxed()
            .iter()
            .filter_map(|(id, is_feasible)| is_feasible.then_some(*id))
            .collect()
    }

    pub fn feasible_unrelaxed_ids(&self) -> BTreeSet<u64> {
        self.feasible_unrelaxed()
            .iter()
            .filter_map(|(id, is_feasible)| is_feasible.then_some(*id))
            .collect()
    }

    /// Find the best ID in terms of the total objective value.
    fn best(&self, ids: impl Iterator<Item = u64>) -> Result<u64> {
        let objectives = self.objectives()?;
        let obj = ids
            .map(|id| {
                Ok((
                    id,
                    objectives
                        .get(id)
                        .context(format!("SampleSet lacks objective for sample ID={id}"))?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let sense = Sense::try_from(self.sense).context("Invalid sense")?;
        obj.iter()
            .min_by(|(_, a), (_, b)| {
                if sense == Sense::Minimize {
                    a.total_cmp(b)
                } else {
                    b.total_cmp(a)
                }
            })
            .map(|(id, _)| *id)
            .context("No feasible solution found in SampleSet")
    }

    pub fn best_feasible_id(&self) -> Result<u64> {
        self.best(self.feasible_ids().into_iter())
    }

    pub fn best_feasible_unrelaxed_id(&self) -> Result<u64> {
        self.best(self.feasible_unrelaxed_ids().into_iter())
    }

    pub fn best_feasible(&self) -> Result<Solution> {
        self.get(self.best_feasible_id()?)
    }

    pub fn best_feasible_unrelaxed(&self) -> Result<Solution> {
        self.get(self.best_feasible_unrelaxed_id()?)
    }

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
            objective: self.objectives()?.get(sample_id).with_context(|| {
                format!("SampleSet lacks objective for sample with ID={sample_id}")
            })?,
            decision_variables,
            feasible_relaxed: Some(*self.feasible_relaxed().get(&sample_id).with_context(
                || format!("SampleSet lacks feasibility for sample with ID={sample_id}"),
            )?),
            feasible: *self.feasible_unrelaxed().get(&sample_id).with_context(|| {
                format!("SampleSet lacks unrelaxed feasibility for sample with ID={sample_id}")
            })?,
            evaluated_constraints,
            ..Default::default()
        })
    }
}
