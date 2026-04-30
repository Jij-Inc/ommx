use super::*;

impl Instance {
    /// Internal helper to validate required IDs against precomputed sets.
    ///
    /// Mirrors the `used / fixed / dependent` disjointness invariant the
    /// builder enforces (`builder.rs`): a constraint or objective cannot
    /// reference a variable whose value has been pinned via
    /// [`DecisionVariable::substituted_value`] (`fixed`), nor a variable
    /// used as a substitution-dependency key (`dependent`).
    fn validate_required_ids_with_sets(
        required_ids: &VariableIDSet,
        variable_ids: &VariableIDSet,
        dependency_keys: &VariableIDSet,
        fixed_ids: &VariableIDSet,
    ) -> crate::Result<()> {
        // Check if all required IDs are defined
        if !required_ids.is_subset(variable_ids) {
            let id = *required_ids.difference(variable_ids).next().unwrap();
            crate::bail!({ ?id }, "Undefined variable ID is used: {id:?}");
        }

        // Check if any required ID is a dependent variable (used as a key in decision_variable_dependency)
        if let Some(&id) = required_ids.intersection(dependency_keys).next() {
            crate::bail!(
                { ?id },
                "Dependent variable cannot be used in objectives or constraints: {id:?}",
            );
        }

        // Check if any required ID is a fixed (substituted) variable.
        if let Some(&id) = required_ids.intersection(fixed_ids).next() {
            crate::bail!(
                { ?id },
                "Fixed variable {id:?} (substituted_value set) cannot be used in objectives or constraints",
            );
        }

        Ok(())
    }

    /// Validate that all required variable IDs are defined in the instance
    /// and are not dependent variables (i.e., not used as keys in
    /// decision_variable_dependency) and are not fixed variables
    /// (substituted_value set).
    fn validate_required_ids(&self, required_ids: VariableIDSet) -> crate::Result<()> {
        let variable_ids: VariableIDSet = self.decision_variables.keys().cloned().collect();
        let dependency_keys: VariableIDSet = self.decision_variable_dependency.keys().collect();
        let fixed_ids: VariableIDSet = self
            .decision_variables
            .values()
            .filter(|dv| dv.substituted_value().is_some())
            .map(|dv| dv.id())
            .collect();
        Self::validate_required_ids_with_sets(
            &required_ids,
            &variable_ids,
            &dependency_keys,
            &fixed_ids,
        )
    }

    /// Set the objective function
    pub fn set_objective(&mut self, objective: Function) -> crate::Result<()> {
        // Validate that all variables in the objective are defined
        self.validate_required_ids(objective.required_ids())?;
        self.objective = objective;
        Ok(())
    }

    /// Insert a new constraint with its metadata, picking an unused id.
    ///
    /// Returns the newly assigned [`ConstraintID`]. The metadata is
    /// drained into the per-constraint [`ConstraintMetadataStore`]; pass
    /// `ConstraintMetadata::default()` for an unannotated constraint.
    ///
    /// All variable IDs referenced by the constraint must already be
    /// present in `decision_variables` and must not be substitution-
    /// dependency keys, matching the validation enforced by
    /// [`Self::insert_constraint`].
    pub fn add_constraint(
        &mut self,
        constraint: Constraint,
        metadata: crate::ConstraintMetadata,
    ) -> crate::Result<ConstraintID> {
        self.validate_required_ids(constraint.required_ids())?;
        let id = self.constraint_collection.unused_id();
        self.constraint_collection
            .insert_with(id, constraint, metadata);
        Ok(id)
    }

    /// Verify that the given id is a binary decision variable.
    ///
    /// Used at the structural positions of indicator and one-hot constraints,
    /// where the [`Instance`] builder enforces `Kind::Binary` and the same
    /// invariant must hold for the post-construction `add_*` setters.
    fn require_binary_variable(&self, id: VariableID) -> crate::Result<()> {
        let dv = self
            .decision_variables
            .get(&id)
            .ok_or_else(|| crate::error!("Variable {id:?} is not defined in decision_variables"))?;
        if dv.kind() != crate::decision_variable::Kind::Binary {
            crate::bail!({ ?id }, "Variable {id:?} must be binary");
        }
        Ok(())
    }

    /// Insert a new indicator constraint with its metadata, picking an unused id.
    ///
    /// Returns the newly assigned [`crate::IndicatorConstraintID`].
    /// Enforces the same invariants as the [`Instance`] builder:
    /// - All variable IDs referenced by the constraint (function plus the
    ///   indicator variable) must be present in `decision_variables` and must
    ///   not be substitution-dependency keys.
    /// - The indicator variable must have [`Kind::Binary`](crate::decision_variable::Kind).
    pub fn add_indicator_constraint(
        &mut self,
        constraint: crate::IndicatorConstraint,
        metadata: crate::ConstraintMetadata,
    ) -> crate::Result<crate::IndicatorConstraintID> {
        self.validate_required_ids(constraint.required_ids())?;
        self.require_binary_variable(constraint.indicator_variable)?;
        let id = self.indicator_constraint_collection.unused_id();
        self.indicator_constraint_collection
            .insert_with(id, constraint, metadata);
        Ok(id)
    }

    /// Insert a new one-hot constraint with its metadata, picking an unused id.
    ///
    /// Returns the newly assigned [`crate::OneHotConstraintID`]. Enforces
    /// the [`Instance`] builder's invariants: every variable in the one-hot
    /// set must be defined and have [`Kind::Binary`](crate::decision_variable::Kind).
    pub fn add_one_hot_constraint(
        &mut self,
        constraint: crate::OneHotConstraint,
        metadata: crate::ConstraintMetadata,
    ) -> crate::Result<crate::OneHotConstraintID> {
        self.validate_required_ids(constraint.required_ids())?;
        for var_id in &constraint.variables {
            self.require_binary_variable(*var_id)?;
        }
        let id = self.one_hot_constraint_collection.unused_id();
        self.one_hot_constraint_collection
            .insert_with(id, constraint, metadata);
        Ok(id)
    }

    /// Insert a new SOS1 constraint with its metadata, picking an unused id.
    ///
    /// Returns the newly assigned [`crate::Sos1ConstraintID`]. Enforces the
    /// [`Instance`] builder's invariants: the variable set must be non-empty
    /// and every variable must be defined in `decision_variables`.
    pub fn add_sos1_constraint(
        &mut self,
        constraint: crate::Sos1Constraint,
        metadata: crate::ConstraintMetadata,
    ) -> crate::Result<crate::Sos1ConstraintID> {
        if constraint.variables.is_empty() {
            crate::bail!("SOS1 constraint must contain at least one variable");
        }
        self.validate_required_ids(constraint.required_ids())?;
        let id = self.sos1_constraint_collection.unused_id();
        self.sos1_constraint_collection
            .insert_with(id, constraint, metadata);
        Ok(id)
    }

    /// Insert a decision variable with its metadata.
    ///
    /// The decision variable's `id()` must not collide with any existing
    /// variable, and must not be a substitution-dependency key. Returns the
    /// inserted variable's id for symmetry with `add_constraint`.
    pub fn add_decision_variable(
        &mut self,
        variable: crate::DecisionVariable,
        metadata: crate::DecisionVariableMetadata,
    ) -> crate::Result<crate::VariableID> {
        let id = variable.id();
        if self.decision_variables.contains_key(&id) {
            crate::bail!({ ?id }, "Duplicate decision variable ID: {id:?}");
        }
        if self.decision_variable_dependency.keys().any(|k| k == id) {
            crate::bail!(
                { ?id },
                "Variable id {id:?} is currently used as a substitution-dependency key",
            );
        }
        self.decision_variables.insert(id, variable);
        self.variable_metadata.insert(id, metadata);
        Ok(id)
    }

    /// Insert a constraint into the instance under the given [`ConstraintID`].
    ///
    /// - If the constraint already exists, it will be replaced.
    /// - If the ID is in the removed constraints, it replaces it.
    /// - Otherwise, it adds the constraint to the instance.
    ///
    pub fn insert_constraint(
        &mut self,
        id: ConstraintID,
        constraint: Constraint,
    ) -> crate::Result<Option<Constraint>> {
        // Validate that all variables in the constraints are defined
        self.validate_required_ids(constraint.required_ids())?;
        use std::collections::btree_map::Entry;
        if let Entry::Occupied(mut o) = self.constraint_collection.removed_mut().entry(id) {
            let (rc, _reason) = o.get_mut();
            let old_function = std::mem::replace(&mut rc.stage.function, constraint.stage.function);
            let old_equality = std::mem::replace(&mut rc.equality, constraint.equality);
            let removed = Constraint {
                equality: old_equality,
                stage: crate::constraint::CreatedData {
                    function: old_function,
                },
            };
            return Ok(Some(removed));
        }
        Ok(self
            .constraint_collection
            .active_mut()
            .insert(id, constraint))
    }

    /// Insert multiple `(id, constraint)` pairs into the instance with a single validation pass.
    ///
    /// This is more efficient than calling [`Self::insert_constraint`] multiple times
    /// because it validates all required variable IDs once, rather than
    /// rebuilding the validation sets for each constraint.
    ///
    /// The behavior for each constraint follows the same rules as [`Self::insert_constraint`]:
    /// - If the constraint already exists, it will be replaced.
    /// - If the ID is in the removed constraints, it replaces it.
    /// - Otherwise, it adds the constraint to the instance.
    ///
    /// # Atomicity
    ///
    /// This method is atomic: all constraints are validated before any insertion occurs.
    /// If any constraint fails validation, no constraints are inserted and an error is returned.
    ///
    pub fn insert_constraints(
        &mut self,
        constraints: Vec<(ConstraintID, Constraint)>,
    ) -> crate::Result<BTreeMap<ConstraintID, Constraint>> {
        // Build validation sets once
        let variable_ids: VariableIDSet = self.decision_variables.keys().cloned().collect();
        let dependency_keys: VariableIDSet = self.decision_variable_dependency.keys().collect();
        let fixed_ids: VariableIDSet = self
            .decision_variables
            .values()
            .filter(|dv| dv.substituted_value().is_some())
            .map(|dv| dv.id())
            .collect();

        // Validate all constraints first (atomic: fail before any insertion)
        for (_, constraint) in &constraints {
            let required_ids = constraint.required_ids();
            Self::validate_required_ids_with_sets(
                &required_ids,
                &variable_ids,
                &dependency_keys,
                &fixed_ids,
            )?;
        }

        // Insert all constraints (validation already done)
        let mut replaced = BTreeMap::new();
        for (id, constraint) in constraints {
            use std::collections::btree_map::Entry;
            let old = if let Entry::Occupied(mut o) =
                self.constraint_collection.removed_mut().entry(id)
            {
                let (rc, _reason) = o.get_mut();
                let old_function =
                    std::mem::replace(&mut rc.stage.function, constraint.stage.function);
                let old_equality = std::mem::replace(&mut rc.equality, constraint.equality);
                Some(Constraint {
                    equality: old_equality,
                    stage: crate::constraint::CreatedData {
                        function: old_function,
                    },
                })
            } else {
                self.constraint_collection
                    .active_mut()
                    .insert(id, constraint)
            };
            if let Some(old_constraint) = old {
                replaced.insert(id, old_constraint);
            }
        }

        Ok(replaced)
    }

    /// Returns the next available ConstraintID.
    ///
    /// Finds the maximum ID from both active constraints and removed constraints,
    /// then adds 1. If there are no constraints, returns ConstraintID(0).
    ///
    /// Note: This method does not track which IDs have been allocated.
    /// Consecutive calls will return the same ID until a constraint is actually added.
    pub fn next_constraint_id(&self) -> ConstraintID {
        let max_in_constraints = self
            .constraints()
            .last_key_value()
            .map(|(id, _)| id.into_inner());
        let max_in_removed = self
            .removed_constraints()
            .last_key_value()
            .map(|(id, _)| id.into_inner());

        max_in_constraints
            .max(max_in_removed)
            .map(|max| ConstraintID::from(max + 1))
            .unwrap_or(ConstraintID::from(0))
    }
}

impl ParametricInstance {
    /// Validate that all required IDs are defined either as decision variables
    /// or as parameters, and are not currently used as substitution-dependency
    /// keys.
    ///
    /// `ParametricInstance` validation differs from
    /// [`Instance::validate_required_ids`](Instance) by also accepting
    /// parameter IDs — constraints in a parametric instance may reference
    /// parameters that will be substituted later via
    /// [`ParametricInstance::with_parameters`].
    fn validate_required_ids(&self, required_ids: VariableIDSet) -> crate::Result<()> {
        let variable_ids: VariableIDSet = self.decision_variables().keys().cloned().collect();
        let parameter_ids: VariableIDSet = self.parameters().keys().cloned().collect();
        let known_ids: VariableIDSet = variable_ids.union(&parameter_ids).cloned().collect();
        let dependency_keys: VariableIDSet = self.decision_variable_dependency().keys().collect();
        let fixed_ids: VariableIDSet = self
            .decision_variables()
            .values()
            .filter(|dv| dv.substituted_value().is_some())
            .map(|dv| dv.id())
            .collect();

        if !required_ids.is_subset(&known_ids) {
            let id = *required_ids.difference(&known_ids).next().unwrap();
            crate::bail!({ ?id }, "Undefined variable ID is used: {id:?}");
        }
        if let Some(&id) = required_ids.intersection(&dependency_keys).next() {
            crate::bail!(
                { ?id },
                "Dependent variable cannot be used in objectives or constraints: {id:?}",
            );
        }
        if let Some(&id) = required_ids.intersection(&fixed_ids).next() {
            crate::bail!(
                { ?id },
                "Fixed variable {id:?} (substituted_value set) cannot be used in objectives or constraints",
            );
        }
        Ok(())
    }

    /// Insert a new constraint with its metadata, picking an unused id.
    ///
    /// Mirrors [`Instance::add_constraint`] for parametric instances.
    /// Returns the newly assigned [`ConstraintID`]. The metadata is drained
    /// into the per-constraint [`ConstraintMetadataStore`]; pass
    /// [`ConstraintMetadata::default`](crate::ConstraintMetadata) for an
    /// unannotated constraint.
    ///
    /// All IDs referenced by the constraint must already be present in either
    /// `decision_variables` or `parameters`, and must not be substitution-
    /// dependency keys.
    pub fn add_constraint(
        &mut self,
        constraint: Constraint,
        metadata: crate::ConstraintMetadata,
    ) -> crate::Result<ConstraintID> {
        self.validate_required_ids(constraint.required_ids())?;
        let id = self.constraint_collection.unused_id();
        self.constraint_collection
            .insert_with(id, constraint, metadata);
        Ok(id)
    }

    /// Validate that the given ids are real decision variables (not parameters).
    ///
    /// Used for *structural* positions in special constraints — the indicator
    /// variable and the variable sets of one-hot / SOS1 — where parameter ids
    /// would not be substitutable in a way that preserves the constraint's
    /// semantics. Function-body ids continue to be validated through
    /// [`Self::validate_required_ids`], which permits parameters.
    fn require_decision_variables(&self, ids: VariableIDSet) -> crate::Result<()> {
        let variable_ids: VariableIDSet = self.decision_variables().keys().cloned().collect();
        if !ids.is_subset(&variable_ids) {
            let id = *ids.difference(&variable_ids).next().unwrap();
            if self.parameters().contains_key(&id) {
                crate::bail!(
                    { ?id },
                    "Parameter id {id:?} cannot occupy a structural variable position; \
                     structural variables in indicator / one-hot / SOS1 constraints \
                     must be decision variables",
                );
            }
            crate::bail!({ ?id }, "Undefined variable ID is used: {id:?}");
        }
        Ok(())
    }

    /// Verify that the given id is a binary decision variable. Mirrors
    /// [`Instance::require_binary_variable`](Instance) for the parametric
    /// host. Parameter ids are rejected because they are not decision
    /// variables in the first place — the matching error message points
    /// at the structural-position rule from
    /// [`Self::require_decision_variables`].
    fn require_binary_variable(&self, id: VariableID) -> crate::Result<()> {
        let dv = self.decision_variables().get(&id).ok_or_else(|| {
            if self.parameters().contains_key(&id) {
                crate::error!(
                    "Parameter id {id:?} cannot occupy a structural variable position; \
                     it must be a binary decision variable",
                )
            } else {
                crate::error!("Variable {id:?} is not defined in decision_variables")
            }
        })?;
        if dv.kind() != crate::decision_variable::Kind::Binary {
            crate::bail!({ ?id }, "Variable {id:?} must be binary");
        }
        Ok(())
    }

    /// Insert a new indicator constraint with its metadata, picking an unused id.
    ///
    /// Mirrors [`Instance::add_indicator_constraint`] for parametric
    /// instances. The function body may reference either decision variables
    /// or parameters, but the indicator variable itself must be a binary
    /// decision variable — substitution cannot replace a structural variable
    /// position, and the indicator semantics require `Kind::Binary`.
    pub fn add_indicator_constraint(
        &mut self,
        constraint: crate::IndicatorConstraint,
        metadata: crate::ConstraintMetadata,
    ) -> crate::Result<crate::IndicatorConstraintID> {
        // Structural position: the indicator variable must be a binary
        // decision variable, not a parameter or a non-binary variable.
        self.require_binary_variable(constraint.indicator_variable)?;
        // `validate_required_ids` (variables ∪ parameters minus dependency
        // keys) is allowed to see the indicator variable here too: the
        // variable-vs-parameter axis is already enforced above, so this
        // call's only added contribution for the indicator variable is the
        // dependency-key check.
        self.validate_required_ids(constraint.required_ids())?;
        let id = self.indicator_constraint_collection.unused_id();
        self.indicator_constraint_collection
            .insert_with(id, constraint, metadata);
        Ok(id)
    }

    /// Insert a new one-hot constraint with its metadata, picking an unused id.
    ///
    /// All variables in the one-hot set are structural and must be binary
    /// decision variables (parameter ids and non-binary kinds are rejected).
    /// Dependency keys are also rejected.
    pub fn add_one_hot_constraint(
        &mut self,
        constraint: crate::OneHotConstraint,
        metadata: crate::ConstraintMetadata,
    ) -> crate::Result<crate::OneHotConstraintID> {
        for var_id in &constraint.variables {
            self.require_binary_variable(*var_id)?;
        }
        self.validate_required_ids(constraint.required_ids())?;
        let id = self.one_hot_constraint_collection.unused_id();
        self.one_hot_constraint_collection
            .insert_with(id, constraint, metadata);
        Ok(id)
    }

    /// Insert a new SOS1 constraint with its metadata, picking an unused id.
    ///
    /// All variables in the SOS1 set are structural and must be decision
    /// variables (parameter ids are rejected). The set must be non-empty.
    /// Dependency keys are also rejected. Unlike one-hot, SOS1 does not
    /// require `Kind::Binary`.
    pub fn add_sos1_constraint(
        &mut self,
        constraint: crate::Sos1Constraint,
        metadata: crate::ConstraintMetadata,
    ) -> crate::Result<crate::Sos1ConstraintID> {
        if constraint.variables.is_empty() {
            crate::bail!("SOS1 constraint must contain at least one variable");
        }
        let required_ids = constraint.required_ids();
        self.require_decision_variables(required_ids.clone())?;
        self.validate_required_ids(required_ids)?;
        let id = self.sos1_constraint_collection.unused_id();
        self.sos1_constraint_collection
            .insert_with(id, constraint, metadata);
        Ok(id)
    }

    /// Insert a decision variable with its metadata.
    ///
    /// The variable's id must not collide with any existing decision
    /// variable, parameter, or substitution-dependency key.
    pub fn add_decision_variable(
        &mut self,
        variable: crate::DecisionVariable,
        metadata: crate::DecisionVariableMetadata,
    ) -> crate::Result<crate::VariableID> {
        let id = variable.id();
        if self.decision_variables().contains_key(&id) {
            crate::bail!({ ?id }, "Duplicate decision variable ID: {id:?}");
        }
        if self.parameters().contains_key(&id) {
            crate::bail!(
                { ?id },
                "Variable id {id:?} collides with an existing parameter id",
            );
        }
        if self.decision_variable_dependency().keys().any(|k| k == id) {
            crate::bail!(
                { ?id },
                "Variable id {id:?} is currently used as a substitution-dependency key",
            );
        }
        self.decision_variables.insert(id, variable);
        self.variable_metadata.insert(id, metadata);
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assign, coeff,
        constraint::{Constraint, ConstraintID},
        linear,
        polynomial_base::{Linear, LinearMonomial},
        DecisionVariable, Function, VariableID,
    };

    use maplit::btreemap;

    #[test]
    fn test_insert_constraint_success() {
        // Create a simple instance with two decision variables
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
        };

        let objective = linear!(1) + coeff!(1.0);

        let mut instance = Instance::new(
            Sense::Minimize,
            objective.into(),
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        // Insert a new constraint using variable 1
        let constraint = Constraint::equal_to_zero((linear!(1) + coeff!(2.0)).into());
        let result = instance
            .insert_constraint(ConstraintID::from(10), constraint.clone())
            .unwrap();

        // Should return None since no constraint with ID 10 existed before
        assert!(result.is_none());
        assert_eq!(instance.constraints().len(), 1);
        assert_eq!(
            instance.constraints().get(&ConstraintID::from(10)),
            Some(&constraint)
        );
    }

    #[test]
    fn test_insert_constraint_replace_existing() {
        // Create instance with one constraint
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(
            VariableID::from(1),
            DecisionVariable::binary(VariableID::from(1)),
        );
        decision_variables.insert(
            VariableID::from(2),
            DecisionVariable::binary(VariableID::from(2)),
        );

        let objective = Function::Linear(Linear::single_term(
            LinearMonomial::Variable(VariableID::from(1)),
            coeff!(1.0),
        ));

        let mut constraints = BTreeMap::new();
        let original_constraint = Constraint::equal_to_zero((linear!(1) + coeff!(1.0)).into());
        constraints.insert(ConstraintID::from(5), original_constraint.clone());

        let mut instance =
            Instance::new(Sense::Minimize, objective, decision_variables, constraints).unwrap();

        // Insert a new constraint with the same ID but using variable 2
        let new_constraint = Constraint::equal_to_zero((linear!(2) + coeff!(1.0)).into());
        let result = instance
            .insert_constraint(ConstraintID::from(5), new_constraint.clone())
            .unwrap();

        // Should return the old constraint that was replaced
        assert_eq!(result, Some(original_constraint));
        assert_eq!(instance.constraints().len(), 1);
        assert_eq!(
            instance.constraints().get(&ConstraintID::from(5)),
            Some(&new_constraint)
        );
    }

    #[test]
    fn test_insert_constraint_undefined_variable() {
        // Create instance with only variable 1 and 2
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(
            VariableID::from(1),
            DecisionVariable::binary(VariableID::from(1)),
        );
        decision_variables.insert(
            VariableID::from(2),
            DecisionVariable::binary(VariableID::from(2)),
        );

        let objective = Function::Linear(Linear::single_term(
            LinearMonomial::Variable(VariableID::from(1)),
            coeff!(1.0),
        ));

        let mut instance = Instance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        // Try to insert constraint using undefined variable 999
        let constraint = Constraint::equal_to_zero((linear!(999) + coeff!(1.0)).into());
        let result = instance.insert_constraint(ConstraintID::from(1), constraint);

        // Should fail with undefined variable error
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err.to_string(),
            "Undefined variable ID is used: VariableID(999)"
        );
        // Ensure no constraint was added
        assert_eq!(instance.constraints().len(), 0);
    }

    #[test]
    fn test_insert_constraint_multiple_operations() {
        // Test multiple insertions and replacements
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(
            VariableID::from(1),
            DecisionVariable::binary(VariableID::from(1)),
        );
        decision_variables.insert(
            VariableID::from(2),
            DecisionVariable::binary(VariableID::from(2)),
        );
        decision_variables.insert(
            VariableID::from(3),
            DecisionVariable::binary(VariableID::from(3)),
        );

        let objective = Function::Linear(Linear::single_term(
            LinearMonomial::Variable(VariableID::from(1)),
            coeff!(1.0),
        ));

        let mut instance = Instance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        // Insert multiple constraints
        let constraint1 = Constraint::equal_to_zero((linear!(1) + coeff!(1.0)).into());
        let constraint2 = Constraint::equal_to_zero((linear!(2) + coeff!(1.0)).into());
        let constraint3 = Constraint::equal_to_zero((linear!(3) + coeff!(1.0)).into());

        assert!(instance
            .insert_constraint(ConstraintID::from(1), constraint1.clone())
            .unwrap()
            .is_none());
        assert!(instance
            .insert_constraint(ConstraintID::from(2), constraint2.clone())
            .unwrap()
            .is_none());
        assert!(instance
            .insert_constraint(ConstraintID::from(3), constraint3.clone())
            .unwrap()
            .is_none());
        assert_eq!(instance.constraints().len(), 3);

        // Replace constraint 2 with new one
        let new_constraint2 = Constraint::equal_to_zero((linear!(1) + coeff!(1.0)).into());
        let replaced = instance
            .insert_constraint(ConstraintID::from(2), new_constraint2.clone())
            .unwrap();
        assert_eq!(replaced, Some(constraint2));
        assert_eq!(instance.constraints().len(), 3);
        assert_eq!(
            instance.constraints().get(&ConstraintID::from(2)),
            Some(&new_constraint2)
        );
    }

    #[test]
    fn test_insert_constraint_with_dependency_key() {
        // Create instance with decision variables and dependency
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
            VariableID::from(3) => DecisionVariable::binary(VariableID::from(3)),
        };
        let objective = linear!(1) + coeff!(1.0);
        let mut instance = Instance::new(
            Sense::Minimize,
            objective.into(),
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        // Add a dependency: x2 = x1 + 1
        instance.decision_variable_dependency = assign! {
            2 <- linear!(1) + coeff!(1.0)
        };

        // Try to insert constraint using variable 2 (which is in dependency keys)
        let constraint = Constraint::equal_to_zero((linear!(2) + coeff!(1.0)).into());
        let result = instance.insert_constraint(ConstraintID::from(1), constraint);
        assert_eq!(
            result.unwrap_err().to_string(),
            "Dependent variable cannot be used in objectives or constraints: VariableID(2)"
        );
        // Ensure no constraint was added
        assert_eq!(instance.constraints().len(), 0);
    }

    #[test]
    fn test_add_constraint_rejects_fixed_variable() {
        // Pin variable 2's value via substituted_value, then try to add a
        // constraint that references it. The setter must reject — same rule
        // the builder enforces (used ∩ fixed = ∅).
        let mut decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
        };
        decision_variables
            .get_mut(&VariableID::from(2))
            .unwrap()
            .substitute(0.0, crate::ATol::default())
            .unwrap();

        let objective = linear!(1) + coeff!(1.0);
        let mut instance = Instance::new(
            Sense::Minimize,
            objective.into(),
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        let bad = crate::Constraint::equal_to_zero((linear!(2) + coeff!(1.0)).into());
        let err = instance
            .add_constraint(bad, crate::ConstraintMetadata::default())
            .unwrap_err();
        assert!(
            err.to_string().contains("Fixed variable") && err.to_string().contains("VariableID(2)"),
            "unexpected error: {err}"
        );
        assert!(instance.constraints().is_empty());
    }

    #[test]
    fn test_set_objective_with_dependency_key() {
        // Create instance with decision variables and dependency
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
        };
        let objective = linear!(1) + coeff!(1.0);
        let mut instance = Instance::new(
            Sense::Minimize,
            objective.into(),
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        // Add a dependency: x2 = x1 + 1
        instance.decision_variable_dependency = assign! {
            2 <- linear!(1) + coeff!(1.0)
        };

        // Try to set objective using variable 2 (which is in dependency keys)
        let new_objective = linear!(2) + coeff!(1.0);
        let result = instance.set_objective(new_objective.into());

        // Should fail with DependentVariableUsed error
        assert_eq!(
            result.unwrap_err().to_string(),
            "Dependent variable cannot be used in objectives or constraints: VariableID(2)"
        );
        // Ensure objective was not changed
        assert_eq!(instance.objective, Function::from(linear!(1) + coeff!(1.0)));
    }

    #[test]
    fn test_insert_constraint_replace_removed_constraint() {
        // Create instance with one active constraint and one removed constraint
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
        };

        let objective = (linear!(1) + coeff!(1.0)).into();
        let constraints = btreemap! {
            ConstraintID::from(1) => Constraint::equal_to_zero((linear!(1) + coeff!(1.0)).into(),
            ),
            ConstraintID::from(2) => Constraint::equal_to_zero((linear!(2) + coeff!(2.0)).into(),
            ),
        };

        let mut instance =
            Instance::new(Sense::Minimize, objective, decision_variables, constraints).unwrap();
        instance
            .relax_constraint(ConstraintID::from(2), "test".to_string(), [])
            .unwrap();

        // Verify initial state
        assert_eq!(instance.constraints().len(), 1);
        assert_eq!(instance.removed_constraints().len(), 1);

        // Insert a new constraint with the same ID as the removed constraint
        let new_constraint =
            Constraint::equal_to_zero((linear!(1) + linear!(2) + coeff!(3.0)).into());
        let result = instance
            .insert_constraint(ConstraintID::from(2), new_constraint.clone())
            .unwrap();

        // Should return the old removed constraint
        assert_eq!(
            result,
            Some(Constraint::equal_to_zero((linear!(2) + coeff!(2.0)).into(),))
        );

        assert_eq!(instance.constraints().len(), 1);
        assert_eq!(instance.removed_constraints().len(), 1);
        let (removed, _reason) = instance
            .removed_constraints()
            .get(&ConstraintID::from(2))
            .unwrap();
        assert_eq!(removed.equality, new_constraint.equality);
        assert_eq!(removed.stage.function, new_constraint.stage.function);
    }

    #[test]
    fn test_insert_constraints_bulk() {
        // Create instance with decision variables
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
            VariableID::from(3) => DecisionVariable::binary(VariableID::from(3)),
        };
        let objective = linear!(1) + coeff!(1.0);
        let mut instance = Instance::new(
            Sense::Minimize,
            objective.into(),
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        // Insert multiple constraints at once
        let constraints = vec![
            (
                ConstraintID::from(1),
                Constraint::equal_to_zero((linear!(1) + coeff!(1.0)).into()),
            ),
            (
                ConstraintID::from(2),
                Constraint::equal_to_zero((linear!(2) + coeff!(2.0)).into()),
            ),
            (
                ConstraintID::from(3),
                Constraint::equal_to_zero((linear!(3) + coeff!(3.0)).into()),
            ),
        ];

        let replaced = instance.insert_constraints(constraints.clone()).unwrap();

        // No constraints were replaced since none existed before
        assert!(replaced.is_empty());
        assert_eq!(instance.constraints().len(), 3);

        // Verify constraints were inserted correctly
        for (id, constraint) in &constraints {
            assert_eq!(instance.constraints().get(id), Some(constraint));
        }
    }

    #[test]
    fn test_insert_constraints_bulk_with_undefined_variable() {
        // Create instance with only variables 1 and 2
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
        };
        let objective = linear!(1) + coeff!(1.0);
        let mut instance = Instance::new(
            Sense::Minimize,
            objective.into(),
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        // Try to insert constraints where one uses undefined variable 999
        let constraints = vec![
            (
                ConstraintID::from(1),
                Constraint::equal_to_zero((linear!(1) + coeff!(1.0)).into()),
            ),
            (
                ConstraintID::from(2),
                Constraint::equal_to_zero((linear!(999) + coeff!(2.0)).into()),
            ),
            (
                ConstraintID::from(3),
                Constraint::equal_to_zero((linear!(2) + coeff!(3.0)).into()),
            ),
        ];

        let result = instance.insert_constraints(constraints);

        // Should fail with undefined variable error
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Undefined variable ID is used: VariableID(999)"
        );
        // Ensure no constraints were added (atomic operation)
        assert_eq!(instance.constraints().len(), 0);
    }

    #[test]
    fn test_insert_constraints_bulk_replace_existing() {
        // Create instance with existing constraints
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
        };
        let objective = linear!(1) + coeff!(1.0);
        let constraints = btreemap! {
            ConstraintID::from(1) => Constraint::equal_to_zero((linear!(1) + coeff!(1.0)).into(),
            ),
            ConstraintID::from(2) => Constraint::equal_to_zero((linear!(2) + coeff!(2.0)).into(),
            ),
        };
        let mut instance = Instance::new(
            Sense::Minimize,
            objective.into(),
            decision_variables,
            constraints,
        )
        .unwrap();

        // Replace constraint 1, add constraint 3
        let new_constraints = vec![
            (
                ConstraintID::from(1),
                Constraint::equal_to_zero((linear!(2) + coeff!(10.0)).into()),
            ),
            (
                ConstraintID::from(3),
                Constraint::equal_to_zero((linear!(1) + coeff!(3.0)).into()),
            ),
        ];

        let replaced = instance
            .insert_constraints(new_constraints.clone())
            .unwrap();

        // Should have replaced constraint 1
        assert_eq!(replaced.len(), 1);
        assert!(replaced.contains_key(&ConstraintID::from(1)));
        assert_eq!(instance.constraints().len(), 3);
    }

    #[test]
    fn test_insert_constraints_bulk_replace_removed() {
        // Create instance with a removed constraint
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
        };
        let objective = linear!(1) + coeff!(1.0);
        let constraints = btreemap! {
            ConstraintID::from(1) => Constraint::equal_to_zero((linear!(1) + coeff!(1.0)).into(),
            ),
        };
        let mut instance = Instance::new(
            Sense::Minimize,
            objective.into(),
            decision_variables,
            constraints,
        )
        .unwrap();

        // Remove constraint 1
        instance
            .relax_constraint(ConstraintID::from(1), "test".to_string(), [])
            .unwrap();
        assert_eq!(instance.constraints().len(), 0);
        assert_eq!(instance.removed_constraints().len(), 1);

        // Replace the removed constraint
        let new_constraints = vec![(
            ConstraintID::from(1),
            Constraint::equal_to_zero((linear!(2) + coeff!(10.0)).into()),
        )];

        let replaced = instance.insert_constraints(new_constraints).unwrap();

        // Should have replaced the removed constraint
        assert_eq!(replaced.len(), 1);
        assert!(replaced.contains_key(&ConstraintID::from(1)));
        // Constraint is still in removed_constraints (with updated content)
        assert_eq!(instance.removed_constraints().len(), 1);
    }

    #[test]
    fn test_insert_constraints_bulk_with_dependent_variable() {
        // Create instance with decision variables and dependency
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
            VariableID::from(3) => DecisionVariable::binary(VariableID::from(3)),
        };
        let objective = linear!(1) + coeff!(1.0);
        let mut instance = Instance::new(
            Sense::Minimize,
            objective.into(),
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        // Add a dependency: x2 = x1 + 1
        instance.decision_variable_dependency = assign! {
            2 <- linear!(1) + coeff!(1.0)
        };

        // Try to insert constraints using variable 2 (which is in dependency keys)
        let constraints = vec![
            (
                ConstraintID::from(1),
                Constraint::equal_to_zero((linear!(1) + coeff!(1.0)).into()),
            ),
            (
                ConstraintID::from(2),
                Constraint::equal_to_zero((linear!(2) + coeff!(2.0)).into()),
            ),
        ];

        let result = instance.insert_constraints(constraints);

        // Should fail with DependentVariableUsed error
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Dependent variable cannot be used in objectives or constraints: VariableID(2)"
        );
        // Ensure no constraints were added (atomic operation)
        assert_eq!(instance.constraints().len(), 0);
    }

    #[test]
    fn test_next_constraint_id() {
        // Test basic case: empty instance
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
        };
        let objective = (linear!(1) + coeff!(1.0)).into();
        let instance = Instance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(instance.next_constraint_id(), ConstraintID::from(0));

        // Test considering both active and removed constraints
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
        };
        let objective = (linear!(1) + coeff!(1.0)).into();
        let constraints = btreemap! {
            ConstraintID::from(3) => Constraint::equal_to_zero((linear!(1) + coeff!(1.0)).into(),
            ),
            ConstraintID::from(15) => Constraint::equal_to_zero((linear!(1) + coeff!(2.0)).into(),
            ),
        };
        let mut instance =
            Instance::new(Sense::Minimize, objective, decision_variables, constraints).unwrap();
        instance
            .relax_constraint(ConstraintID::from(15), "test".to_string(), [])
            .unwrap();

        // Should return 16 (max(3, 15) + 1)
        assert_eq!(instance.next_constraint_id(), ConstraintID::from(16));
    }
}
