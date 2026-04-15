use super::*;
use crate::constraint_type::ConstraintCollection;
use crate::parse::Parse;

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
    named_functions: BTreeMap<NamedFunctionID, NamedFunction>,
    removed_constraints: BTreeMap<ConstraintID, RemovedConstraint>,
    indicator_constraints: BTreeMap<crate::IndicatorConstraintID, crate::IndicatorConstraint>,
    removed_indicator_constraints: BTreeMap<
        crate::IndicatorConstraintID,
        crate::indicator_constraint::RemovedIndicatorConstraint,
    >,
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

    /// Sets the named functions.
    pub fn named_functions(
        mut self,
        named_functions: BTreeMap<NamedFunctionID, NamedFunction>,
    ) -> Self {
        self.named_functions = named_functions;
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

    /// Sets the indicator constraints.
    pub fn indicator_constraints(
        mut self,
        indicator_constraints: BTreeMap<crate::IndicatorConstraintID, crate::IndicatorConstraint>,
    ) -> Self {
        self.indicator_constraints = indicator_constraints;
        self
    }

    /// Sets the removed indicator constraints.
    pub fn removed_indicator_constraints(
        mut self,
        removed_indicator_constraints: BTreeMap<
            crate::IndicatorConstraintID,
            crate::indicator_constraint::RemovedIndicatorConstraint,
        >,
    ) -> Self {
        self.removed_indicator_constraints = removed_indicator_constraints;
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
    /// - Map keys don't match their value's ID (decision_variables, constraints, removed_constraints)
    /// - The objective function or constraints reference undefined variable IDs
    /// - The keys of `constraints` and `removed_constraints` are not disjoint
    /// - The keys of `decision_variable_dependency` are not in `decision_variables`
    /// - `used`, `fixed`, and `dependent` are not pairwise disjoint (see [`DecisionVariableAnalysis`])
    pub fn build(self) -> anyhow::Result<Instance> {
        let sense = self
            .sense
            .ok_or(InstanceError::MissingRequiredField { field: "sense" })?;
        let objective = self
            .objective
            .ok_or(InstanceError::MissingRequiredField { field: "objective" })?;
        let decision_variables =
            self.decision_variables
                .ok_or(InstanceError::MissingRequiredField {
                    field: "decision_variables",
                })?;
        let constraints = self
            .constraints
            .ok_or(InstanceError::MissingRequiredField {
                field: "constraints",
            })?;

        // Validate that decision variable map keys match their value's id
        for (key, value) in &decision_variables {
            if *key != value.id() {
                return Err(InstanceError::InconsistentDecisionVariableID {
                    key: *key,
                    value_id: value.id(),
                }
                .into());
            }
        }

        // Validate that constraint map keys match their value's id
        for (key, value) in &constraints {
            if *key != value.id {
                return Err(InstanceError::InconsistentConstraintID {
                    key: *key,
                    value_id: value.id,
                }
                .into());
            }
        }

        // Validate that removed constraint map keys match their value's id
        for (key, value) in &self.removed_constraints {
            if *key != value.id {
                return Err(InstanceError::InconsistentRemovedConstraintID {
                    key: *key,
                    value_id: value.id,
                }
                .into());
            }
        }

        // Collect all variable IDs for validation
        let variable_ids: VariableIDSet = decision_variables.keys().cloned().collect();

        // Validate that all variable IDs in objective and constraints are defined
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
        // Validate that all variable IDs in removed_constraints are defined
        // (removed_constraints may contain fixed or dependent variable IDs)
        for removed in self.removed_constraints.values() {
            for id in removed.required_ids() {
                if !variable_ids.contains(&id) {
                    return Err(InstanceError::UndefinedVariableID { id }.into());
                }
            }
        }

        // Validate named_functions: key must match value's id, and all variable IDs must exist
        for (key, nf) in &self.named_functions {
            if *key != nf.id {
                return Err(InstanceError::InconsistentNamedFunctionID {
                    key: *key,
                    id: nf.id,
                }
                .into());
            }
            for id in nf.function.required_ids() {
                if !variable_ids.contains(&id) {
                    return Err(InstanceError::UndefinedVariableID { id }.into());
                }
            }
        }

        // Validate indicator constraints
        for (key, value) in &self.indicator_constraints {
            if *key != value.id {
                return Err(InstanceError::InconsistentIndicatorConstraintID {
                    key: *key,
                    value_id: value.id,
                }
                .into());
            }
            // Check that indicator_variable exists and is binary
            let indicator_id = value.indicator_variable;
            let Some(dv) = decision_variables.get(&indicator_id) else {
                return Err(InstanceError::UndefinedIndicatorVariable { id: indicator_id }.into());
            };
            if dv.kind() != crate::decision_variable::Kind::Binary {
                return Err(InstanceError::IndicatorVariableNotBinary { id: indicator_id }.into());
            }
            // Check that all variable IDs in function are defined
            for id in value.required_ids() {
                if !variable_ids.contains(&id) {
                    return Err(InstanceError::UndefinedVariableID { id }.into());
                }
            }
        }
        for (key, value) in &self.removed_indicator_constraints {
            if *key != value.id {
                return Err(InstanceError::InconsistentRemovedIndicatorConstraintID {
                    key: *key,
                    value_id: value.id,
                }
                .into());
            }
            // Check that indicator_variable exists and is binary
            let indicator_id = value.indicator_variable;
            let Some(dv) = decision_variables.get(&indicator_id) else {
                return Err(InstanceError::UndefinedIndicatorVariable { id: indicator_id }.into());
            };
            if dv.kind() != crate::decision_variable::Kind::Binary {
                return Err(InstanceError::IndicatorVariableNotBinary { id: indicator_id }.into());
            }
            for id in value.required_ids() {
                if !variable_ids.contains(&id) {
                    return Err(InstanceError::UndefinedVariableID { id }.into());
                }
            }
        }
        // Validate disjointness of indicator active/removed
        for id in self.removed_indicator_constraints.keys() {
            if self.indicator_constraints.contains_key(id) {
                return Err(InstanceError::OverlappingIndicatorConstraintID { id: *id }.into());
            }
        }

        // Validate that constraints and removed_constraints keys are disjoint
        for id in self.removed_constraints.keys() {
            if constraints.contains_key(id) {
                return Err(InstanceError::OverlappingConstraintID { id: *id }.into());
            }
        }

        // Validate that decision_variable_dependency keys are in decision_variables
        // (dependent variables must exist as decision variables to get kind/bound info)
        for id in self.decision_variable_dependency.keys() {
            if !variable_ids.contains(&id) {
                return Err(InstanceError::UndefinedDependentVariableID { id }.into());
            }
        }

        // Invariant: used, fixed, and dependent must be pairwise disjoint.
        // See DecisionVariableAnalysis for details.
        // - used: IDs appearing in objective or constraints
        // - fixed: IDs with substituted_value set
        // - dependent: keys of decision_variable_dependency
        let mut used: VariableIDSet = objective.required_ids().into_iter().collect();
        for constraint in constraints.values() {
            used.extend(constraint.required_ids());
        }
        for ic in self.indicator_constraints.values() {
            used.extend(ic.required_ids());
        }
        let fixed: VariableIDSet = decision_variables
            .values()
            .filter(|dv| dv.substituted_value().is_some())
            .map(|dv| dv.id())
            .collect();
        let dependent: VariableIDSet = self.decision_variable_dependency.keys().collect();

        // Check used ∩ dependent = ∅
        if let Some(id) = used.intersection(&dependent).next() {
            return Err(InstanceError::DependentVariableUsed { id: *id }.into());
        }

        // Check used ∩ fixed = ∅
        if let Some(id) = used.intersection(&fixed).next() {
            return Err(InstanceError::FixedVariableUsed { id: *id }.into());
        }

        // Check fixed ∩ dependent = ∅
        if let Some(id) = fixed.intersection(&dependent).next() {
            return Err(InstanceError::FixedAndDependentVariable { id: *id }.into());
        }

        // Validate constraint_hints using Parse trait.
        // Unlike `add_constraint_hints` which errors on removed constraint references,
        // the builder uses Parse which silently filters invalid hints (with debug log).
        // This is intentional: the builder may receive data from old serialized instances
        // where hints may reference constraints that have since been removed, and we want
        // to heal such inconsistencies rather than fail.
        // Move values into context tuple to avoid cloning, then destructure to recover ownership.
        let hints: v1::ConstraintHints = self.constraint_hints.into();
        let context = (decision_variables, constraints, self.removed_constraints);
        let constraint_hints = hints.parse(&context)?;
        let (decision_variables, constraints, removed_constraints) = context;

        Ok(Instance {
            sense,
            objective,
            decision_variables,
            constraint_collection: ConstraintCollection::new(constraints, removed_constraints),
            indicator_constraint_collection: ConstraintCollection::new(
                self.indicator_constraints,
                self.removed_indicator_constraints,
            ),
            named_functions: self.named_functions,
            decision_variable_dependency: self.decision_variable_dependency,
            constraint_hints,
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
        let err = Instance::builder()
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .constraints(BTreeMap::new())
            .build()
            .unwrap_err();
        let instance_err = err.downcast_ref::<InstanceError>().unwrap();
        assert!(matches!(
            instance_err,
            InstanceError::MissingRequiredField { field: "sense" }
        ));

        // Missing objective
        let err = Instance::builder()
            .sense(Sense::Minimize)
            .decision_variables(BTreeMap::new())
            .constraints(BTreeMap::new())
            .build()
            .unwrap_err();
        let instance_err = err.downcast_ref::<InstanceError>().unwrap();
        assert!(matches!(
            instance_err,
            InstanceError::MissingRequiredField { field: "objective" }
        ));
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

        let err = Instance::builder()
            .sense(Sense::Minimize)
            .objective(objective)
            .decision_variables(BTreeMap::new())
            .constraints(BTreeMap::new())
            .build()
            .unwrap_err();

        let instance_err = err.downcast_ref::<InstanceError>().unwrap();
        assert!(matches!(
            instance_err,
            InstanceError::UndefinedVariableID {
                id
            } if *id == VariableID::from(999)
        ));
    }

    #[test]
    fn test_builder_overlapping_constraint_ids() {
        use crate::{Constraint, RemovedConstraint};
        use maplit::btreemap;

        let constraint_id = ConstraintID::from(1);
        let constraint = Constraint::equal_to_zero(constraint_id, Function::Zero);
        let removed_constraint = RemovedConstraint {
            id: constraint_id,
            equality: constraint.equality,
            metadata: constraint.metadata.clone(),
            stage: crate::constraint::RemovedData {
                function: constraint.stage.function.clone(),
                removed_reason: crate::constraint::RemovedReason {
                    reason: "test".to_string(),
                    parameters: Default::default(),
                },
            },
        };

        let err = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .constraints(btreemap! { constraint_id => constraint })
            .removed_constraints(btreemap! { constraint_id => removed_constraint })
            .build()
            .unwrap_err();

        let instance_err = err.downcast_ref::<InstanceError>().unwrap();
        assert!(matches!(
            instance_err,
            InstanceError::OverlappingConstraintID { id } if *id == constraint_id
        ));
    }

    #[test]
    fn test_builder_undefined_variable_dependency() {
        use maplit::btreemap;

        let var_id = VariableID::from(1);
        let undefined_var_id = VariableID::from(999);
        let decision_variables = btreemap! {
            var_id => DecisionVariable::binary(var_id),
        };

        // Create a dependency that references a variable not in decision_variables
        let dependency = AcyclicAssignments::new(btreemap! {
            undefined_var_id => Function::Zero,
        })
        .unwrap();

        let err = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(decision_variables)
            .constraints(BTreeMap::new())
            .decision_variable_dependency(dependency)
            .build()
            .unwrap_err();

        let instance_err = err.downcast_ref::<InstanceError>().unwrap();
        assert!(matches!(
            instance_err,
            InstanceError::UndefinedDependentVariableID { id } if *id == undefined_var_id
        ));
    }

    #[test]
    fn test_builder_valid_variable_dependency() {
        use maplit::btreemap;

        let var_id = VariableID::from(1);
        let decision_variables = btreemap! {
            var_id => DecisionVariable::binary(var_id),
        };

        // Create a dependency for a variable that IS in decision_variables (this is valid)
        let dependency = AcyclicAssignments::new(btreemap! {
            var_id => Function::Zero,
        })
        .unwrap();

        // This should succeed because dependent variable must be in decision_variables
        // and is not used in objective/constraints
        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(decision_variables)
            .constraints(BTreeMap::new())
            .decision_variable_dependency(dependency)
            .build();

        assert!(instance.is_ok());
    }

    #[test]
    fn test_builder_dependent_variable_used_in_objective() {
        use crate::linear;
        use maplit::btreemap;

        let var_id = VariableID::from(1);
        let decision_variables = btreemap! {
            var_id => DecisionVariable::binary(var_id),
        };

        // Create a dependency for var_id
        let dependency = AcyclicAssignments::new(btreemap! {
            var_id => Function::Zero,
        })
        .unwrap();

        // Objective uses var_id, which is also a dependent variable - this should fail
        let err = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(1)))
            .decision_variables(decision_variables)
            .constraints(BTreeMap::new())
            .decision_variable_dependency(dependency)
            .build()
            .unwrap_err();

        let instance_err = err.downcast_ref::<InstanceError>().unwrap();
        assert!(matches!(
            instance_err,
            InstanceError::DependentVariableUsed { id } if *id == var_id
        ));
    }

    #[test]
    fn test_builder_dependent_variable_used_in_constraint() {
        use crate::linear;
        use maplit::btreemap;

        let var_id = VariableID::from(1);
        let constraint_id = ConstraintID::from(1);
        let decision_variables = btreemap! {
            var_id => DecisionVariable::binary(var_id),
        };

        // Create a dependency for var_id
        let dependency = AcyclicAssignments::new(btreemap! {
            var_id => Function::Zero,
        })
        .unwrap();

        // Constraint uses var_id, which is also a dependent variable - this should fail
        let constraint = Constraint::equal_to_zero(constraint_id, Function::from(linear!(1)));
        let err = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(decision_variables)
            .constraints(btreemap! { constraint_id => constraint })
            .decision_variable_dependency(dependency)
            .build()
            .unwrap_err();

        let instance_err = err.downcast_ref::<InstanceError>().unwrap();
        assert!(matches!(
            instance_err,
            InstanceError::DependentVariableUsed { id } if *id == var_id
        ));
    }

    #[test]
    fn test_builder_fixed_and_dependent_variable() {
        use maplit::btreemap;

        let var_id = VariableID::from(1);
        // Create a decision variable with substituted_value (fixed)
        let mut dv = DecisionVariable::binary(var_id);
        dv.substitute(1.0, crate::ATol::default()).unwrap();
        let decision_variables = btreemap! {
            var_id => dv,
        };

        // Create a dependency for the same var_id (now both fixed and dependent)
        let dependency = AcyclicAssignments::new(btreemap! {
            var_id => Function::Zero,
        })
        .unwrap();

        let err = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(decision_variables)
            .constraints(BTreeMap::new())
            .decision_variable_dependency(dependency)
            .build()
            .unwrap_err();

        let instance_err = err.downcast_ref::<InstanceError>().unwrap();
        assert!(matches!(
            instance_err,
            InstanceError::FixedAndDependentVariable { id } if *id == var_id
        ));
    }

    #[test]
    fn test_builder_undefined_variable_in_removed_constraint() {
        use crate::RemovedConstraint;
        use maplit::btreemap;

        let constraint_id = ConstraintID::from(1);
        // Create a removed constraint that references undefined variable ID 999
        let constraint =
            Constraint::equal_to_zero(constraint_id, Function::from(linear!(999) + coeff!(1.0)));
        let removed_constraint = RemovedConstraint {
            id: constraint.id,
            equality: constraint.equality,
            metadata: constraint.metadata,
            stage: crate::constraint::RemovedData {
                function: constraint.stage.function,
                removed_reason: crate::constraint::RemovedReason {
                    reason: "test".to_string(),
                    parameters: Default::default(),
                },
            },
        };

        let err = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .constraints(BTreeMap::new())
            .removed_constraints(btreemap! { constraint_id => removed_constraint })
            .build()
            .unwrap_err();

        let instance_err = err.downcast_ref::<InstanceError>().unwrap();
        assert!(matches!(
            instance_err,
            InstanceError::UndefinedVariableID { id } if *id == VariableID::from(999)
        ));
    }

    #[test]
    fn test_builder_fixed_variable_used_in_objective() {
        use maplit::btreemap;

        let var_id = VariableID::from(1);
        // Create a decision variable with substituted_value (fixed)
        let mut dv = DecisionVariable::binary(var_id);
        dv.substitute(1.0, crate::ATol::default()).unwrap();
        let decision_variables = btreemap! {
            var_id => dv,
        };

        // Objective uses var_id, which is fixed - this should fail
        let err = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(1)))
            .decision_variables(decision_variables)
            .constraints(BTreeMap::new())
            .build()
            .unwrap_err();

        let instance_err = err.downcast_ref::<InstanceError>().unwrap();
        assert!(matches!(
            instance_err,
            InstanceError::FixedVariableUsed { id } if *id == var_id
        ));
    }

    #[test]
    fn test_builder_inconsistent_named_function_id() {
        use crate::{NamedFunction, NamedFunctionID};
        use maplit::btreemap;

        // Create a named function with id=1 but use key=2 in the map
        let named_function = NamedFunction {
            id: NamedFunctionID::from(1),
            function: Function::Zero,
            name: Some("f".to_string()),
            subscripts: vec![],
            parameters: Default::default(),
            description: None,
        };

        let err = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .constraints(BTreeMap::new())
            .named_functions(btreemap! {
                NamedFunctionID::from(2) => named_function,  // key=2 but id=1
            })
            .build()
            .unwrap_err();

        let instance_err = err.downcast_ref::<InstanceError>().unwrap();
        assert!(matches!(
            instance_err,
            InstanceError::InconsistentNamedFunctionID { key, id }
                if *key == NamedFunctionID::from(2) && *id == NamedFunctionID::from(1)
        ));
    }

    #[test]
    fn test_builder_undefined_variable_in_named_function() {
        use crate::{NamedFunction, NamedFunctionID};
        use maplit::btreemap;

        // Create a named function that references undefined variable ID 999
        let named_function = NamedFunction {
            id: NamedFunctionID::from(1),
            function: Function::from(linear!(999) + coeff!(1.0)),
            name: Some("f".to_string()),
            subscripts: vec![],
            parameters: Default::default(),
            description: None,
        };

        let err = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .constraints(BTreeMap::new())
            .named_functions(btreemap! {
                NamedFunctionID::from(1) => named_function,
            })
            .build()
            .unwrap_err();

        let instance_err = err.downcast_ref::<InstanceError>().unwrap();
        assert!(matches!(
            instance_err,
            InstanceError::UndefinedVariableID { id } if *id == VariableID::from(999)
        ));
    }
}
