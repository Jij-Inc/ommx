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
    pub fn with_parameters(self, parameters: crate::v1::Parameters) -> anyhow::Result<Instance> {
        use crate::ATol;
        use anyhow::bail;
        use std::collections::BTreeSet;

        // Convert v1::Parameters to BTreeMap for validation and processing
        let param_map: BTreeMap<VariableID, f64> = parameters
            .entries
            .iter()
            .map(|(k, v)| (VariableID::from(*k), *v))
            .collect();

        // Check that all required parameters are provided
        let required_ids: BTreeSet<VariableID> = self.parameters.keys().cloned().collect();
        let given_ids: BTreeSet<VariableID> = param_map.keys().cloned().collect();

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
            entries: parameters.entries.clone(),
        };
        let atol = ATol::default();

        // Partially evaluate the objective and constraints
        let mut objective = self.objective;
        objective.partial_evaluate(&state, atol)?;

        let mut constraints = self.constraints;
        for (_, constraint) in constraints.iter_mut() {
            constraint.function.partial_evaluate(&state, atol)?;
        }

        Ok(Instance {
            sense: self.sense,
            objective,
            decision_variables: self.decision_variables,
            constraints,
            removed_constraints: self.removed_constraints,
            decision_variable_dependency: self.decision_variable_dependency,
            constraint_hints: self.constraint_hints,
            parameters: Some(parameters),
            description: self.description,
        })
    }
}
