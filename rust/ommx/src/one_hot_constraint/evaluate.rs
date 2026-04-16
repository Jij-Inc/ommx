use super::*;
use crate::{ATol, Evaluate, Propagate, VariableIDSet};

impl Propagate for OneHotConstraint<Created> {
    type Output = Option<Self>;

    fn propagate(
        mut self,
        state: &crate::v1::State,
        atol: ATol,
    ) -> anyhow::Result<(Self::Output, crate::v1::State)> {
        let mut fixed_to_one: Option<VariableID> = None;
        let mut unfixed = BTreeSet::new();

        for &var_id in &self.variables {
            let Some(&value) = state.entries.get(&var_id.into_inner()) else {
                unfixed.insert(var_id);
                continue;
            };

            if (value - 1.0).abs() < *atol {
                // Variable is ~1
                if let Some(first) = fixed_to_one {
                    anyhow::bail!(
                        "Multiple variables fixed to 1 in one-hot constraint {:?}: {:?} and {:?}",
                        self.id,
                        first,
                        var_id
                    );
                }
                fixed_to_one = Some(var_id);
            } else if value.abs() < *atol {
                // Variable is ~0, removed from set
            } else {
                anyhow::bail!(
                    "Variable {:?} in one-hot constraint {:?} fixed to invalid value {} (must be 0 or 1)",
                    var_id,
                    self.id,
                    value
                );
            }
        }

        if fixed_to_one.is_some() {
            // One variable is 1 → constraint satisfied, fix remaining unfixed to 0
            let mut additional = crate::v1::State::default();
            for var_id in &unfixed {
                additional.entries.insert(var_id.into_inner(), 0.0);
            }
            Ok((None, additional))
        } else if unfixed.is_empty() {
            // All variables fixed to 0 → infeasible
            anyhow::bail!(
                "All variables in one-hot constraint {:?} are fixed to 0, constraint cannot be satisfied",
                self.id
            );
        } else if unfixed.len() == 1 {
            // Unit propagation: exactly one unfixed variable → must be 1
            let var_id = *unfixed.iter().next().unwrap();
            let mut additional = crate::v1::State::default();
            additional.entries.insert(var_id.into_inner(), 1.0);
            Ok((None, additional))
        } else {
            // Multiple unfixed variables remain, constraint still active
            self.variables = unfixed;
            Ok((Some(self), crate::v1::State::default()))
        }
    }
}

impl Evaluate for OneHotConstraint<Created> {
    type Output = EvaluatedOneHotConstraint;
    type SampledOutput = SampledOneHotConstraint;

    fn evaluate(&self, state: &crate::v1::State, atol: ATol) -> anyhow::Result<Self::Output> {
        let used_decision_variable_ids = self.required_ids();
        let (feasible, active_variable) = check_one_hot(&self.variables, state, atol, self.id)?;

        Ok(OneHotConstraint {
            id: self.id,
            variables: self.variables.clone(),
            metadata: self.metadata.clone(),
            stage: OneHotEvaluatedData {
                feasible,
                active_variable,
                used_decision_variable_ids,
            },
        })
    }

    fn evaluate_samples(
        &self,
        samples: &crate::v1::Samples,
        atol: ATol,
    ) -> anyhow::Result<Self::SampledOutput> {
        let mut feasible = BTreeMap::new();
        let mut active_variable = BTreeMap::new();

        for (sample_id, state) in samples.iter() {
            let sample_id = crate::SampleID::from(*sample_id);
            let (f, av) = check_one_hot(&self.variables, state, atol, self.id)?;
            feasible.insert(sample_id, f);
            active_variable.insert(sample_id, av);
        }

        Ok(OneHotConstraint {
            id: self.id,
            variables: self.variables.clone(),
            metadata: self.metadata.clone(),
            stage: OneHotSampledData {
                feasible,
                active_variable,
                used_decision_variable_ids: self.required_ids(),
            },
        })
    }

    fn partial_evaluate(&mut self, state: &crate::v1::State, _atol: ATol) -> anyhow::Result<()> {
        for var_id in &self.variables {
            if state.entries.contains_key(&var_id.into_inner()) {
                anyhow::bail!(
                    "Cannot partially evaluate variable {:?} of one-hot constraint {:?}. \
                     Fixing a one-hot variable would change the constraint type.",
                    var_id,
                    self.id
                );
            }
        }
        Ok(())
    }

    fn required_ids(&self) -> VariableIDSet {
        self.variables.iter().copied().collect()
    }
}

/// Check one-hot feasibility for a single state.
///
/// Returns `(feasible, active_variable)`:
/// - feasible: exactly one variable is 1, the rest are 0
/// - active_variable: the variable that is 1 (None if infeasible)
fn check_one_hot(
    variables: &BTreeSet<VariableID>,
    state: &crate::v1::State,
    atol: ATol,
    constraint_id: OneHotConstraintID,
) -> anyhow::Result<(bool, Option<VariableID>)> {
    let mut active: Option<VariableID> = None;

    for &var_id in variables {
        let value = state.entries.get(&var_id.into_inner()).ok_or_else(|| {
            anyhow::anyhow!(
                "Variable {:?} not found in state for one-hot constraint {:?}",
                var_id,
                constraint_id
            )
        })?;

        if (value - 1.0).abs() < *atol {
            // Variable is ~1
            if active.is_some() {
                // Multiple variables are 1 → infeasible
                return Ok((false, None));
            }
            active = Some(var_id);
        } else if value.abs() < *atol {
            // Variable is ~0, OK
        } else {
            // Variable is neither 0 nor 1 → infeasible
            return Ok((false, None));
        }
    }

    match active {
        Some(var_id) => Ok((true, Some(var_id))),
        None => Ok((false, None)), // All zeros → infeasible for one-hot
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Evaluate, Propagate};
    use std::collections::HashMap;

    fn make_one_hot(id: u64, var_ids: &[u64]) -> OneHotConstraint {
        let vars = var_ids.iter().copied().map(VariableID::from).collect();
        OneHotConstraint::new(OneHotConstraintID::from(id), vars)
    }

    #[test]
    fn test_evaluate_feasible() {
        let c = make_one_hot(1, &[1, 2, 3]);
        // x1=0, x2=1, x3=0 → feasible, active=x2
        let state = crate::v1::State::from(HashMap::from([(1, 0.0), (2, 1.0), (3, 0.0)]));
        let result = c.evaluate(&state, ATol::default()).unwrap();
        assert!(result.stage.feasible);
        assert_eq!(result.stage.active_variable, Some(VariableID::from(2)));
    }

    #[test]
    fn test_evaluate_infeasible_multiple_ones() {
        let c = make_one_hot(1, &[1, 2, 3]);
        // x1=1, x2=1, x3=0 → infeasible
        let state = crate::v1::State::from(HashMap::from([(1, 1.0), (2, 1.0), (3, 0.0)]));
        let result = c.evaluate(&state, ATol::default()).unwrap();
        assert!(!result.stage.feasible);
        assert_eq!(result.stage.active_variable, None);
    }

    #[test]
    fn test_evaluate_infeasible_all_zeros() {
        let c = make_one_hot(1, &[1, 2, 3]);
        // x1=0, x2=0, x3=0 → infeasible (one-hot requires exactly one)
        let state = crate::v1::State::from(HashMap::from([(1, 0.0), (2, 0.0), (3, 0.0)]));
        let result = c.evaluate(&state, ATol::default()).unwrap();
        assert!(!result.stage.feasible);
        assert_eq!(result.stage.active_variable, None);
    }

    #[test]
    fn test_evaluate_infeasible_non_binary() {
        let c = make_one_hot(1, &[1, 2]);
        // x1=0.5, x2=0.5 → infeasible
        let state = crate::v1::State::from(HashMap::from([(1, 0.5), (2, 0.5)]));
        let result = c.evaluate(&state, ATol::default()).unwrap();
        assert!(!result.stage.feasible);
    }

    #[test]
    fn test_partial_evaluate_error() {
        let mut c = make_one_hot(1, &[1, 2, 3]);
        let state = crate::v1::State::from(HashMap::from([(2, 1.0)]));
        let result = c.partial_evaluate(&state, ATol::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_partial_evaluate_no_overlap() {
        let mut c = make_one_hot(1, &[1, 2, 3]);
        // Fixing a variable NOT in the one-hot set is fine
        let state = crate::v1::State::from(HashMap::from([(99, 1.0)]));
        let result = c.partial_evaluate(&state, ATol::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_required_ids() {
        let c = make_one_hot(1, &[1, 2, 3]);
        let ids = c.required_ids();
        assert!(ids.contains(&VariableID::from(1)));
        assert!(ids.contains(&VariableID::from(2)));
        assert!(ids.contains(&VariableID::from(3)));
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn test_evaluate_samples() {
        let c = make_one_hot(1, &[1, 2, 3]);

        use crate::v1::samples::SamplesEntry;
        let samples = crate::v1::Samples {
            entries: vec![
                // Sample 0: x1=1, x2=0, x3=0 → feasible, active=x1
                SamplesEntry {
                    state: Some(crate::v1::State::from(HashMap::from([
                        (1, 1.0),
                        (2, 0.0),
                        (3, 0.0),
                    ]))),
                    ids: vec![0],
                },
                // Sample 1: x1=1, x2=1, x3=0 → infeasible
                SamplesEntry {
                    state: Some(crate::v1::State::from(HashMap::from([
                        (1, 1.0),
                        (2, 1.0),
                        (3, 0.0),
                    ]))),
                    ids: vec![1],
                },
                // Sample 2: x1=0, x2=0, x3=0 → infeasible (all zeros)
                SamplesEntry {
                    state: Some(crate::v1::State::from(HashMap::from([
                        (1, 0.0),
                        (2, 0.0),
                        (3, 0.0),
                    ]))),
                    ids: vec![2],
                },
            ],
        };

        let result = c.evaluate_samples(&samples, ATol::default()).unwrap();

        let s0 = crate::SampleID::from(0);
        let s1 = crate::SampleID::from(1);
        let s2 = crate::SampleID::from(2);

        assert_eq!(result.stage.feasible[&s0], true);
        assert_eq!(result.stage.feasible[&s1], false);
        assert_eq!(result.stage.feasible[&s2], false);

        assert_eq!(result.stage.active_variable[&s0], Some(VariableID::from(1)));
        assert_eq!(result.stage.active_variable[&s1], None);
        assert_eq!(result.stage.active_variable[&s2], None);
    }

    // === Propagate tests ===

    #[test]
    fn test_propagate_var_one_fixes_rest() {
        let c = make_one_hot(1, &[1, 2, 3]);
        // x2=1 → x1=0, x3=0
        let state = crate::v1::State::from(HashMap::from([(2, 1.0)]));
        let (output, additional) = c.propagate(&state, ATol::default()).unwrap();
        assert!(output.is_none()); // constraint consumed
        assert_eq!(additional.entries.get(&1), Some(&0.0));
        assert_eq!(additional.entries.get(&3), Some(&0.0));
        assert_eq!(additional.entries.len(), 2);
    }

    #[test]
    fn test_propagate_var_zero_shrinks() {
        let c = make_one_hot(1, &[1, 2, 3]);
        // x1=0 → constraint shrinks to {x2, x3}
        let state = crate::v1::State::from(HashMap::from([(1, 0.0)]));
        let (output, additional) = c.propagate(&state, ATol::default()).unwrap();
        let shrunk = output.unwrap();
        assert_eq!(shrunk.variables.len(), 2);
        assert!(shrunk.variables.contains(&VariableID::from(2)));
        assert!(shrunk.variables.contains(&VariableID::from(3)));
        assert!(additional.entries.is_empty());
    }

    #[test]
    fn test_propagate_unit_clause() {
        let c = make_one_hot(1, &[1, 2, 3]);
        // x1=0, x2=0 → only x3 unfixed → x3 must be 1
        let state = crate::v1::State::from(HashMap::from([(1, 0.0), (2, 0.0)]));
        let (output, additional) = c.propagate(&state, ATol::default()).unwrap();
        assert!(output.is_none()); // constraint consumed
        assert_eq!(additional.entries.get(&3), Some(&1.0));
        assert_eq!(additional.entries.len(), 1);
    }

    #[test]
    fn test_propagate_all_zeros_error() {
        let c = make_one_hot(1, &[1, 2, 3]);
        let state = crate::v1::State::from(HashMap::from([(1, 0.0), (2, 0.0), (3, 0.0)]));
        let result = c.propagate(&state, ATol::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_propagate_multiple_ones_error() {
        let c = make_one_hot(1, &[1, 2, 3]);
        let state = crate::v1::State::from(HashMap::from([(1, 1.0), (2, 1.0)]));
        let result = c.propagate(&state, ATol::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_propagate_no_overlap() {
        let c = make_one_hot(1, &[1, 2, 3]);
        // No variables in state overlap → constraint unchanged
        let state = crate::v1::State::from(HashMap::from([(99, 5.0)]));
        let (output, additional) = c.propagate(&state, ATol::default()).unwrap();
        let same = output.unwrap();
        assert_eq!(same.variables.len(), 3);
        assert!(additional.entries.is_empty());
    }
}
