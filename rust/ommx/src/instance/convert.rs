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
            named_functions,
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
            named_functions,
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

        // Partially evaluate the objective, constraints, and named functions
        let mut objective = self.objective;
        objective.partial_evaluate(&state, atol)?;

        let mut constraints = self.constraints;
        for (_, constraint) in constraints.iter_mut() {
            constraint.function.partial_evaluate(&state, atol)?;
        }

        let mut named_functions = self.named_functions;
        for (_, named_function) in named_functions.iter_mut() {
            named_function.partial_evaluate(&state, atol)?;
        }

        // Dependency RHS expressions may reference parameter IDs. Substitute
        // them before materializing an Instance, which has no parameter set.
        let mut decision_variable_dependency = self.decision_variable_dependency;
        decision_variable_dependency.partial_evaluate(&state, atol)?;

        Ok(Instance {
            sense: self.sense,
            objective,
            decision_variables: self.decision_variables,
            constraints,
            named_functions,
            removed_constraints: self.removed_constraints,
            decision_variable_dependency,
            constraint_hints: self.constraint_hints,
            parameters: Some(parameters),
            description: self.description,
        })
    }
}

#[cfg(test)]
mod with_parameters_tests {
    use super::*;
    use crate::{linear, Function};
    use maplit::btreemap;

    #[test]
    fn decision_variable_dependency_rhs_is_substituted() {
        use crate::AcyclicAssignments;

        let x = VariableID::from(1);
        let dep = VariableID::from(2);
        let p = VariableID::from(100);
        let assignments =
            AcyclicAssignments::new(vec![(dep, Function::from(linear!(1) + linear!(100)))])
                .unwrap();

        let parametric = ParametricInstance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(btreemap! {
                x => DecisionVariable::binary(x),
                dep => DecisionVariable::binary(dep),
            })
            .parameters(btreemap! {
                p => crate::v1::Parameter { id: 100, ..Default::default() },
            })
            .constraints(BTreeMap::new())
            .decision_variable_dependency(assignments)
            .build()
            .unwrap();

        let params = crate::v1::Parameters {
            entries: std::collections::HashMap::from([(100, 1.0)]),
        };
        let instance = parametric.with_parameters(params).unwrap();

        let dep_rhs = instance
            .decision_variable_dependency()
            .get(&dep)
            .expect("dependency entry survives materialization");
        let rhs_required: VariableIDSet = dep_rhs.required_ids();
        assert!(
            !rhs_required.contains(&p),
            "parameter id {p:?} survived in dependency RHS: {rhs_required:?}",
        );
        assert!(
            rhs_required.contains(&x),
            "decision variable id {x:?} should remain in dependency RHS: {rhs_required:?}",
        );
    }
}
