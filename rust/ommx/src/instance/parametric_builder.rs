use super::*;
use crate::constraint_type::ConstraintCollection;

/// Builder for creating [`ParametricInstance`] with validation.
///
/// # Example
/// ```
/// use ommx::{ParametricInstance, Sense, Function};
/// use std::collections::BTreeMap;
///
/// let instance = ParametricInstance::builder()
///     .sense(Sense::Minimize)
///     .objective(Function::Zero)
///     .decision_variables(BTreeMap::new())
///     .parameters(BTreeMap::new())
///     .constraints(BTreeMap::new())
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone, Default)]
pub struct ParametricInstanceBuilder {
    sense: Option<Sense>,
    objective: Option<Function>,
    decision_variables: Option<BTreeMap<VariableID, DecisionVariable>>,
    parameters: Option<BTreeMap<VariableID, v1::Parameter>>,
    constraints: Option<BTreeMap<ConstraintID, Constraint>>,
    named_functions: BTreeMap<NamedFunctionID, NamedFunction>,
    removed_constraints: BTreeMap<ConstraintID, (Constraint, crate::constraint::RemovedReason)>,
    decision_variable_dependency: AcyclicAssignments,
    description: Option<v1::instance::Description>,
}

impl ParametricInstanceBuilder {
    /// Creates a new `ParametricInstanceBuilder` with all fields unset.
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

    /// Sets the parameters.
    pub fn parameters(mut self, parameters: BTreeMap<VariableID, v1::Parameter>) -> Self {
        self.parameters = Some(parameters);
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
        removed_constraints: BTreeMap<ConstraintID, (Constraint, crate::constraint::RemovedReason)>,
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

    /// Sets the description.
    pub fn description(mut self, description: v1::instance::Description) -> Self {
        self.description = Some(description);
        self
    }

    /// Builds the `ParametricInstance` with validation.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Required fields (`sense`, `objective`, `decision_variables`, `parameters`, `constraints`) are not set
    /// - Decision variable IDs and parameter IDs overlap
    /// - The objective function or constraints reference undefined variable IDs
    /// - The keys of `constraints` and `removed_constraints` are not disjoint
    /// - The keys of `decision_variable_dependency` are not in `decision_variables`
    /// - `used`, `fixed`, and `dependent` are not pairwise disjoint (see [`DecisionVariableAnalysis`])
    pub fn build(self) -> crate::Result<ParametricInstance> {
        let sense = self
            .sense
            .ok_or_else(|| crate::error!("Required field is missing: sense"))?;
        let objective = self
            .objective
            .ok_or_else(|| crate::error!("Required field is missing: objective"))?;
        let decision_variables = self
            .decision_variables
            .ok_or_else(|| crate::error!("Required field is missing: decision_variables"))?;
        let parameters = self
            .parameters
            .ok_or_else(|| crate::error!("Required field is missing: parameters"))?;
        let constraints = self
            .constraints
            .ok_or_else(|| crate::error!("Required field is missing: constraints"))?;

        // Validate that decision variable map keys match their value's id
        for (key, value) in &decision_variables {
            if *key != value.id() {
                let value_id = value.id();
                crate::bail!(
                    { ?key, ?value_id },
                    "Decision variable map key {key:?} does not match value's id {value_id:?}",
                );
            }
        }

        // Validate that parameter map keys match their value's id
        for (key, value) in &parameters {
            if key.into_inner() != value.id {
                let value_id = value.id;
                crate::bail!(
                    { ?key, value_id },
                    "Parameter map key {key:?} does not match value's id {value_id}",
                );
            }
        }

        // Check that decision variable IDs and parameter IDs are disjoint
        let decision_variable_ids: VariableIDSet = decision_variables.keys().cloned().collect();
        let parameter_ids: VariableIDSet = parameters.keys().cloned().collect();

        let intersection: VariableIDSet = decision_variable_ids
            .intersection(&parameter_ids)
            .cloned()
            .collect();
        if !intersection.is_empty() {
            let id = *intersection.iter().next().unwrap();
            crate::bail!(
                { ?id },
                "Duplicated variable ID is found in definition: {id:?}",
            );
        }

        // Combine decision variables and parameters for validation
        let all_variable_ids: VariableIDSet = decision_variable_ids
            .union(&parameter_ids)
            .cloned()
            .collect();

        // Validate that all variable IDs in objective and constraints are defined
        for id in objective.required_ids() {
            if !all_variable_ids.contains(&id) {
                crate::bail!({ ?id }, "Undefined variable ID is used: {id:?}");
            }
        }
        for constraint in constraints.values() {
            for id in constraint.required_ids() {
                if !all_variable_ids.contains(&id) {
                    crate::bail!({ ?id }, "Undefined variable ID is used: {id:?}");
                }
            }
        }
        // Validate that all variable IDs in removed_constraints are defined
        // (removed_constraints may contain fixed or dependent variable IDs)
        for (removed, _reason) in self.removed_constraints.values() {
            for id in removed.required_ids() {
                if !all_variable_ids.contains(&id) {
                    crate::bail!({ ?id }, "Undefined variable ID is used: {id:?}");
                }
            }
        }

        // Validate named_functions: key must match value's id, and all variable IDs must exist
        for (key, nf) in &self.named_functions {
            if *key != nf.id {
                let id = nf.id;
                crate::bail!(
                    { ?key, ?id },
                    "Named function map key {key:?} does not match value's id {id:?}",
                );
            }
            for id in nf.function.required_ids() {
                if !all_variable_ids.contains(&id) {
                    crate::bail!({ ?id }, "Undefined variable ID is used: {id:?}");
                }
            }
        }

        // Validate that constraints and removed_constraints keys are disjoint
        for id in self.removed_constraints.keys() {
            if constraints.contains_key(id) {
                crate::bail!(
                    { ?id },
                    "Constraint ID {id:?} is in both constraints and removed_constraints, but they must be disjoint",
                );
            }
        }

        // Validate that decision_variable_dependency keys are in decision_variables
        // (dependent variables must exist as decision variables to get kind/bound info)
        for id in self.decision_variable_dependency.keys() {
            if !decision_variable_ids.contains(&id) {
                crate::bail!(
                    { ?id },
                    "Variable ID {id:?} in decision_variable_dependency is not in decision_variables",
                );
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
        let fixed: VariableIDSet = decision_variables
            .values()
            .filter(|dv| dv.substituted_value().is_some())
            .map(|dv| dv.id())
            .collect();
        let dependent: VariableIDSet = self.decision_variable_dependency.keys().collect();

        // Check used ∩ dependent = ∅
        if let Some(id) = used.intersection(&dependent).next() {
            crate::bail!(
                { ?id },
                "Dependent variable cannot be used in objectives or constraints: {id:?}",
            );
        }

        // Check used ∩ fixed = ∅
        if let Some(id) = used.intersection(&fixed).next() {
            crate::bail!(
                { ?id },
                "Fixed variable {id:?} (substituted_value set) cannot be used in objectives or constraints",
            );
        }

        // Check fixed ∩ dependent = ∅
        if let Some(id) = fixed.intersection(&dependent).next() {
            crate::bail!(
                { ?id },
                "Variable {id:?} cannot be both fixed (substituted_value set) and dependent",
            );
        }

        Ok(ParametricInstance {
            sense,
            objective,
            decision_variables,
            parameters,
            variable_metadata: Default::default(),
            constraint_collection: ConstraintCollection::new(constraints, self.removed_constraints),
            indicator_constraint_collection: Default::default(),
            one_hot_constraint_collection: Default::default(),
            sos1_constraint_collection: Default::default(),
            named_functions: self.named_functions,
            decision_variable_dependency: self.decision_variable_dependency,
            description: self.description,
        })
    }
}

impl ParametricInstance {
    /// Creates a new `ParametricInstanceBuilder`.
    pub fn builder() -> ParametricInstanceBuilder {
        ParametricInstanceBuilder::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parametric_builder_basic() {
        let instance = ParametricInstance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .parameters(BTreeMap::new())
            .constraints(BTreeMap::new())
            .build()
            .unwrap();

        assert_eq!(*instance.sense(), Sense::Minimize);
        assert!(instance.decision_variables().is_empty());
        assert!(instance.parameters().is_empty());
        assert!(instance.constraints().is_empty());
    }

    #[test]
    fn test_parametric_builder_missing_required_field() {
        // Missing parameters
        let err = ParametricInstance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .constraints(BTreeMap::new())
            .build()
            .unwrap_err();
        assert!(
            err.to_string().contains("missing: parameters"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_parametric_builder_overlapping_ids() {
        use maplit::btreemap;

        let var_id = VariableID::from(1);

        // Same ID in both decision_variables and parameters
        let err = ParametricInstance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(btreemap! {
                var_id => DecisionVariable::binary(var_id),
            })
            .parameters(btreemap! {
                var_id => v1::Parameter { id: 1, ..Default::default() },
            })
            .constraints(BTreeMap::new())
            .build()
            .unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("Duplicated variable ID") && msg.contains(&format!("{:?}", var_id)),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn test_parametric_builder_inconsistent_named_function_id() {
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

        let err = ParametricInstance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .parameters(BTreeMap::new())
            .constraints(BTreeMap::new())
            .named_functions(btreemap! {
                NamedFunctionID::from(2) => named_function,  // key=2 but id=1
            })
            .build()
            .unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("Named function map key")
                && msg.contains("NamedFunctionID(2)")
                && msg.contains("NamedFunctionID(1)"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn test_parametric_builder_undefined_variable_in_named_function() {
        use crate::{coeff, linear, NamedFunction, NamedFunctionID};
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

        let err = ParametricInstance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .parameters(BTreeMap::new())
            .constraints(BTreeMap::new())
            .named_functions(btreemap! {
                NamedFunctionID::from(1) => named_function,
            })
            .build()
            .unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("Undefined variable ID") && msg.contains("999"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn test_parametric_builder_named_function_with_parameter() {
        use crate::{linear, NamedFunction, NamedFunctionID};
        use maplit::btreemap;

        // Named functions can reference parameters (not just decision variables)
        let var_id = VariableID::from(1);
        let param_id = VariableID::from(2);

        let named_function = NamedFunction {
            id: NamedFunctionID::from(1),
            function: Function::from(linear!(1) + linear!(2)), // uses both decision var and param
            name: Some("f".to_string()),
            subscripts: vec![],
            parameters: Default::default(),
            description: None,
        };

        let instance = ParametricInstance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(btreemap! {
                var_id => DecisionVariable::binary(var_id),
            })
            .parameters(btreemap! {
                param_id => v1::Parameter { id: 2, ..Default::default() },
            })
            .constraints(BTreeMap::new())
            .named_functions(btreemap! {
                NamedFunctionID::from(1) => named_function,
            })
            .build()
            .unwrap();

        // Should succeed since both var_id and param_id are defined
        assert_eq!(instance.named_functions().len(), 1);
    }
}
