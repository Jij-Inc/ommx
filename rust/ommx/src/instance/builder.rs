use super::*;

/// Builder for creating [`Instance`] with validation.
///
/// This builder allows constructing an `Instance` step by step,
/// with optional fields that can be set before calling `build()`.
///
/// # Example
/// ```
/// use ommx::{Instance, Sense, Function};
/// use std::collections::BTreeMap;
///
/// let instance = Instance::builder()
///     .sense(Sense::Minimize)
///     .objective(Function::Zero)
///     .decision_variables(BTreeMap::new())
///     .constraints(BTreeMap::new())
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone, Default)]
pub struct InstanceBuilder {
    sense: Option<Sense>,
    objective: Option<Function>,
    decision_variables: Option<BTreeMap<VariableID, DecisionVariable>>,
    constraints: Option<BTreeMap<ConstraintID, Constraint>>,
    removed_constraints: BTreeMap<ConstraintID, RemovedConstraint>,
    decision_variable_dependency: AcyclicAssignments,
    constraint_hints: ConstraintHints,
    parameters: Option<v1::Parameters>,
    description: Option<v1::instance::Description>,
}

impl InstanceBuilder {
    /// Creates a new `InstanceBuilder` with all fields unset.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the optimization sense (minimize or maximize).
    pub fn sense(mut self, sense: Sense) -> Self {
        self.sense = Some(sense);
        self
    }

    /// Sets the objective function.
    pub fn objective(mut self, objective: Function) -> Self {
        self.objective = Some(objective);
        self
    }

    /// Sets the decision variables.
    pub fn decision_variables(
        mut self,
        decision_variables: BTreeMap<VariableID, DecisionVariable>,
    ) -> Self {
        self.decision_variables = Some(decision_variables);
        self
    }

    /// Sets the constraints.
    pub fn constraints(mut self, constraints: BTreeMap<ConstraintID, Constraint>) -> Self {
        self.constraints = Some(constraints);
        self
    }

    /// Sets the removed constraints.
    pub fn removed_constraints(
        mut self,
        removed_constraints: BTreeMap<ConstraintID, RemovedConstraint>,
    ) -> Self {
        self.removed_constraints = removed_constraints;
        self
    }

    /// Sets the decision variable dependency.
    pub fn decision_variable_dependency(
        mut self,
        decision_variable_dependency: AcyclicAssignments,
    ) -> Self {
        self.decision_variable_dependency = decision_variable_dependency;
        self
    }

    /// Sets the constraint hints.
    pub fn constraint_hints(mut self, constraint_hints: ConstraintHints) -> Self {
        self.constraint_hints = constraint_hints;
        self
    }

    /// Sets the parameters.
    pub fn parameters(mut self, parameters: v1::Parameters) -> Self {
        self.parameters = Some(parameters);
        self
    }

    /// Sets the description.
    pub fn description(mut self, description: v1::instance::Description) -> Self {
        self.description = Some(description);
        self
    }

    /// Builds the `Instance` with validation.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Required fields (`sense`, `objective`, `decision_variables`, `constraints`) are not set
    /// - The objective function references undefined variable IDs
    /// - Any constraint references undefined variable IDs
    /// - The keys of `constraints` and `removed_constraints` are not disjoint
    /// - The keys of `decision_variable_dependency` are in `decision_variables`
    pub fn build(self) -> anyhow::Result<Instance> {
        let sense = self
            .sense
            .ok_or_else(|| anyhow::anyhow!("sense is required"))?;
        let objective = self
            .objective
            .ok_or_else(|| anyhow::anyhow!("objective is required"))?;
        let decision_variables = self
            .decision_variables
            .ok_or_else(|| anyhow::anyhow!("decision_variables is required"))?;
        let constraints = self
            .constraints
            .ok_or_else(|| anyhow::anyhow!("constraints is required"))?;

        // Validate that all variable IDs in objective and constraints are defined
        let variable_ids: VariableIDSet = decision_variables.keys().cloned().collect();
        for id in objective.required_ids() {
            if !variable_ids.contains(&id) {
                return Err(InstanceError::UndefinedVariableID { id }.into());
            }
        }
        for constraint in constraints.values() {
            for id in constraint.required_ids() {
                if !variable_ids.contains(&id) {
                    return Err(InstanceError::UndefinedVariableID { id }.into());
                }
            }
        }

        // Validate that constraints and removed_constraints keys are disjoint
        for id in self.removed_constraints.keys() {
            if constraints.contains_key(id) {
                anyhow::bail!(
                    "Constraint ID {:?} is in both constraints and removed_constraints",
                    id
                );
            }
        }

        // Validate that decision_variable_dependency keys are not in decision_variables
        for id in self.decision_variable_dependency.keys() {
            if variable_ids.contains(&id) {
                anyhow::bail!(
                    "Variable ID {:?} is in both decision_variables and decision_variable_dependency",
                    id
                );
            }
        }

        Ok(Instance {
            sense,
            objective,
            decision_variables,
            constraints,
            removed_constraints: self.removed_constraints,
            decision_variable_dependency: self.decision_variable_dependency,
            constraint_hints: self.constraint_hints,
            parameters: self.parameters,
            description: self.description,
        })
    }
}

impl Instance {
    /// Creates a new `InstanceBuilder`.
    pub fn builder() -> InstanceBuilder {
        InstanceBuilder::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, linear};

    #[test]
    fn test_builder_basic() {
        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .constraints(BTreeMap::new())
            .build()
            .unwrap();

        assert_eq!(instance.sense(), Sense::Minimize);
        assert!(instance.decision_variables().is_empty());
        assert!(instance.constraints().is_empty());
    }

    #[test]
    fn test_builder_missing_required_field() {
        // Missing sense
        let result = Instance::builder()
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .constraints(BTreeMap::new())
            .build();
        assert!(result.is_err());

        // Missing objective
        let result = Instance::builder()
            .sense(Sense::Minimize)
            .decision_variables(BTreeMap::new())
            .constraints(BTreeMap::new())
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_with_optional_fields() {
        let params = v1::Parameters::default();
        let desc = v1::instance::Description::default();

        let instance = Instance::builder()
            .sense(Sense::Maximize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .constraints(BTreeMap::new())
            .parameters(params.clone())
            .description(desc.clone())
            .build()
            .unwrap();

        assert_eq!(instance.sense(), Sense::Maximize);
        assert!(instance.parameters.is_some());
        assert!(instance.description.is_some());
    }

    #[test]
    fn test_builder_undefined_variable_in_objective() {
        // Create objective function that uses undefined variable ID 999
        let objective = (linear!(999) + coeff!(1.0)).into();

        let result = Instance::builder()
            .sense(Sense::Minimize)
            .objective(objective)
            .decision_variables(BTreeMap::new())
            .constraints(BTreeMap::new())
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_builder_overlapping_constraint_ids() {
        use crate::{Constraint, RemovedConstraint};
        use maplit::btreemap;

        let constraint_id = ConstraintID::from(1);
        let constraint = Constraint::equal_to_zero(constraint_id, Function::Zero);
        let removed_constraint = RemovedConstraint {
            constraint: constraint.clone(),
            removed_reason: "test".to_string(),
            removed_reason_parameters: Default::default(),
        };

        let result = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .constraints(btreemap! { constraint_id => constraint })
            .removed_constraints(btreemap! { constraint_id => removed_constraint })
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_builder_overlapping_variable_dependency() {
        use maplit::btreemap;

        let var_id = VariableID::from(1);
        let decision_variables = btreemap! {
            var_id => DecisionVariable::binary(var_id),
        };

        // Create a dependency that assigns to the same variable ID
        let dependency = AcyclicAssignments::new(btreemap! {
            var_id => Function::Zero,
        })
        .unwrap();

        let result = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(decision_variables)
            .constraints(BTreeMap::new())
            .decision_variable_dependency(dependency)
            .build();

        assert!(result.is_err());
    }
}
