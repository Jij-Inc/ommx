use super::*;
use std::ops::Neg;

impl Instance {
    /// Convert the instance to a minimization problem.
    ///
    /// If the instance is already a minimization problem, this does nothing.
    /// Otherwise, it negates the objective function and changes the sense to minimize.
    ///
    /// Returns `true` if the instance was converted, `false` if it was already a minimization problem.
    pub fn as_minimization_problem(&mut self) -> bool {
        if self.sense == Sense::Minimize {
            false
        } else {
            self.sense = Sense::Minimize;
            self.objective = std::mem::take(&mut self.objective).neg();
            true
        }
    }

    /// Convert the instance to a maximization problem.
    ///
    /// If the instance is already a maximization problem, this does nothing.
    /// Otherwise, it negates the objective function and changes the sense to maximize.
    ///
    /// Returns `true` if the instance was converted, `false` if it was already a maximization problem.
    pub fn as_maximization_problem(&mut self) -> bool {
        if self.sense == Sense::Maximize {
            false
        } else {
            self.sense = Sense::Maximize;
            self.objective = std::mem::take(&mut self.objective).neg();
            true
        }
    }
}

impl From<Instance> for ParametricInstance {
    fn from(
        Instance {
            sense,
            objective,
            decision_variables,
            constraints,
            removed_constraints,
            decision_variable_dependency,
            constraint_hints,
            description,
            ..
        }: Instance,
    ) -> Self {
        ParametricInstance {
            sense,
            objective,
            decision_variables,
            parameters: BTreeMap::default(),
            constraints,
            removed_constraints,
            decision_variable_dependency,
            constraint_hints,
            description,
        }
    }
}

impl ParametricInstance {
    pub fn with_parameters(
        self,
        parameters: BTreeMap<VariableID, f64>,
    ) -> anyhow::Result<Instance> {
        use crate::{v1, ATol};
        use anyhow::bail;
        use std::collections::BTreeSet;

        // Check that all required parameters are provided
        let required_ids: BTreeSet<VariableID> = self.parameters.keys().cloned().collect();
        let given_ids: BTreeSet<VariableID> = parameters.keys().cloned().collect();

        if !required_ids.is_subset(&given_ids) {
            let missing_ids: Vec<_> = required_ids.difference(&given_ids).collect();
            for id in &missing_ids {
                if let Some(param) = self.parameters.get(id) {
                    log::error!("Missing parameter: {param:?}");
                }
            }
            bail!(
                "Missing parameters: Required IDs {:?}, got {:?}",
                required_ids,
                given_ids
            );
        }

        // Create state from parameters
        let state = crate::v1::State {
            entries: parameters
                .clone()
                .into_iter()
                .map(|(k, v)| (k.into_inner(), v))
                .collect(),
        };
        let atol = ATol::default();

        // Partially evaluate the objective and constraints
        let mut objective = self.objective;
        objective.partial_evaluate(&state, atol)?;

        let mut constraints = self.constraints;
        for (_, constraint) in constraints.iter_mut() {
            constraint.function.partial_evaluate(&state, atol)?;
        }

        // Convert parameters to v1::Parameters
        let v1_parameters = v1::Parameters {
            entries: parameters
                .into_iter()
                .map(|(k, v)| (k.into_inner(), v))
                .collect(),
        };

        Ok(Instance {
            sense: self.sense,
            objective,
            decision_variables: self.decision_variables,
            constraints,
            removed_constraints: self.removed_constraints,
            decision_variable_dependency: self.decision_variable_dependency,
            constraint_hints: self.constraint_hints,
            parameters: Some(v1_parameters),
            description: self.description,
        })
    }
}
