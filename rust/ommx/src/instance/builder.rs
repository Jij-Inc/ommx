use super::*;
use crate::constraint_type::ConstraintCollection;

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
    variable_labels: VariableLabelStore,
    fixed_decision_variable_values: BTreeMap<VariableID, f64>,
    constraints: Option<BTreeMap<ConstraintID, Constraint>>,
    constraint_context: ConstraintContextStore<ConstraintID>,
    named_functions: BTreeMap<NamedFunctionID, NamedFunction>,
    named_function_labels: crate::named_function::NamedFunctionLabelStore,
    removed_constraints: BTreeMap<ConstraintID, (Constraint, crate::constraint::RemovedReason)>,
    indicator_constraints: BTreeMap<crate::IndicatorConstraintID, crate::IndicatorConstraint>,
    indicator_constraint_context: ConstraintContextStore<crate::IndicatorConstraintID>,
    removed_indicator_constraints: BTreeMap<
        crate::IndicatorConstraintID,
        (crate::IndicatorConstraint, crate::constraint::RemovedReason),
    >,
    one_hot_constraints: BTreeMap<crate::OneHotConstraintID, crate::OneHotConstraint>,
    one_hot_constraint_context: ConstraintContextStore<crate::OneHotConstraintID>,
    sos1_constraints: BTreeMap<crate::Sos1ConstraintID, crate::Sos1Constraint>,
    sos1_constraint_context: ConstraintContextStore<crate::Sos1ConstraintID>,
    decision_variable_dependency: AcyclicAssignments,
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

    /// Sets the per-variable modeling labels.
    pub fn variable_labels(mut self, variable_labels: VariableLabelStore) -> Self {
        self.variable_labels = variable_labels;
        self
    }

    /// Sets root-owned fixed decision-variable values.
    pub fn fixed_decision_variable_values(
        mut self,
        fixed_decision_variable_values: BTreeMap<VariableID, f64>,
    ) -> Self {
        self.fixed_decision_variable_values = fixed_decision_variable_values;
        self
    }

    /// Sets the constraints.
    pub fn constraints(mut self, constraints: BTreeMap<ConstraintID, Constraint>) -> Self {
        self.constraints = Some(constraints);
        self
    }

    /// Sets the per-regular-constraint context.
    pub fn constraint_context(
        mut self,
        constraint_context: ConstraintContextStore<ConstraintID>,
    ) -> Self {
        self.constraint_context = constraint_context;
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

    /// Sets the per-named-function modeling labels.
    pub fn named_function_labels(
        mut self,
        named_function_labels: crate::named_function::NamedFunctionLabelStore,
    ) -> Self {
        self.named_function_labels = named_function_labels;
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

    /// Sets the indicator constraints.
    pub fn indicator_constraints(
        mut self,
        indicator_constraints: BTreeMap<crate::IndicatorConstraintID, crate::IndicatorConstraint>,
    ) -> Self {
        self.indicator_constraints = indicator_constraints;
        self
    }

    /// Sets the per-indicator-constraint context.
    pub fn indicator_constraint_context(
        mut self,
        indicator_constraint_context: ConstraintContextStore<crate::IndicatorConstraintID>,
    ) -> Self {
        self.indicator_constraint_context = indicator_constraint_context;
        self
    }

    /// Sets the removed indicator constraints.
    pub fn removed_indicator_constraints(
        mut self,
        removed_indicator_constraints: BTreeMap<
            crate::IndicatorConstraintID,
            (crate::IndicatorConstraint, crate::constraint::RemovedReason),
        >,
    ) -> Self {
        self.removed_indicator_constraints = removed_indicator_constraints;
        self
    }

    /// Sets the one-hot constraints.
    pub fn one_hot_constraints(
        mut self,
        one_hot_constraints: BTreeMap<crate::OneHotConstraintID, crate::OneHotConstraint>,
    ) -> Self {
        self.one_hot_constraints = one_hot_constraints;
        self
    }

    /// Sets the per-one-hot-constraint context.
    pub fn one_hot_constraint_context(
        mut self,
        one_hot_constraint_context: ConstraintContextStore<crate::OneHotConstraintID>,
    ) -> Self {
        self.one_hot_constraint_context = one_hot_constraint_context;
        self
    }

    /// Sets the SOS1 constraints.
    pub fn sos1_constraints(
        mut self,
        sos1_constraints: BTreeMap<crate::Sos1ConstraintID, crate::Sos1Constraint>,
    ) -> Self {
        self.sos1_constraints = sos1_constraints;
        self
    }

    /// Sets the per-SOS1-constraint context.
    pub fn sos1_constraint_context(
        mut self,
        sos1_constraint_context: ConstraintContextStore<crate::Sos1ConstraintID>,
    ) -> Self {
        self.sos1_constraint_context = sos1_constraint_context;
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
    /// - Named-function labels reference IDs not present in the named-function table
    /// - The objective function or constraints reference undefined variable IDs
    /// - The keys of `constraints` and `removed_constraints` are not disjoint
    /// - Label/context stores contain IDs that are not owned by the
    ///   corresponding decision-variable, named-function, or constraint collection
    /// - The keys of `decision_variable_dependency` are not in `decision_variables`
    /// - Construction-time `used`, `fixed`, and `dependent` sets are not pairwise disjoint
    pub fn build(self) -> crate::Result<Instance> {
        let sense = self
            .sense
            .ok_or_else(|| crate::error!("Required field is missing: sense"))?;
        let objective = self
            .objective
            .ok_or_else(|| crate::error!("Required field is missing: objective"))?;
        let decision_variables = self
            .decision_variables
            .ok_or_else(|| crate::error!("Required field is missing: decision_variables"))?;
        let constraints = self
            .constraints
            .ok_or_else(|| crate::error!("Required field is missing: constraints"))?;

        // Collect all variable IDs for validation
        let variable_ids: VariableIDSet = decision_variables.keys().cloned().collect();
        crate::modeling_label::validate_modeling_label_ids(
            &self.variable_labels,
            &variable_ids,
            "decision variable",
        )?;

        let fixed_decision_variable_values = self.fixed_decision_variable_values;
        for (id, value) in &fixed_decision_variable_values {
            let Some(dv) = decision_variables.get(id) else {
                crate::bail!(
                    { ?id },
                    "Fixed decision-variable value references unknown decision variable ID {id:?}",
                );
            };
            dv.check_value_consistency(*id, *value, crate::ATol::default())?;
        }

        // Validate that all variable IDs in objective and constraints are defined
        for id in objective.required_ids() {
            if !variable_ids.contains(&id) {
                crate::bail!({ ?id }, "Undefined variable ID is used: {id:?}");
            }
        }
        for constraint in constraints.values() {
            for id in constraint.required_ids() {
                if !variable_ids.contains(&id) {
                    crate::bail!({ ?id }, "Undefined variable ID is used: {id:?}");
                }
            }
        }
        // Validate that all variable IDs in removed_constraints are defined
        // (removed_constraints may contain fixed or dependent variable IDs)
        for (constraint, _reason) in self.removed_constraints.values() {
            for id in constraint.required_ids() {
                if !variable_ids.contains(&id) {
                    crate::bail!({ ?id }, "Undefined variable ID is used: {id:?}");
                }
            }
        }

        // Validate named_functions: map keys own IDs, and all referenced variable IDs must exist.
        for nf in self.named_functions.values() {
            for id in nf.function.required_ids() {
                if !variable_ids.contains(&id) {
                    crate::bail!({ ?id }, "Undefined variable ID is used: {id:?}");
                }
            }
        }
        let named_functions =
            NamedFunctionTable::new(self.named_functions, self.named_function_labels)?;

        // Validate indicator constraints
        for value in self.indicator_constraints.values() {
            // Check that indicator_variable exists and is binary
            let indicator_id = value.indicator_variable;
            let Some(dv) = decision_variables.get(&indicator_id) else {
                crate::bail!(
                    { ?indicator_id },
                    "Indicator variable {indicator_id:?} is not defined in decision_variables",
                );
            };
            if dv.kind() != crate::decision_variable::Kind::Binary {
                crate::bail!(
                    { ?indicator_id },
                    "Indicator variable {indicator_id:?} must be binary",
                );
            }
            // Check that all variable IDs in function are defined
            for id in value.required_ids() {
                if !variable_ids.contains(&id) {
                    crate::bail!({ ?id }, "Undefined variable ID is used: {id:?}");
                }
            }
        }
        for (ic, _reason) in self.removed_indicator_constraints.values() {
            // Check that indicator_variable exists and is binary
            let indicator_id = ic.indicator_variable;
            let Some(dv) = decision_variables.get(&indicator_id) else {
                crate::bail!(
                    { ?indicator_id },
                    "Indicator variable {indicator_id:?} is not defined in decision_variables",
                );
            };
            if dv.kind() != crate::decision_variable::Kind::Binary {
                crate::bail!(
                    { ?indicator_id },
                    "Indicator variable {indicator_id:?} must be binary",
                );
            }
            for id in ic.required_ids() {
                if !variable_ids.contains(&id) {
                    crate::bail!({ ?id }, "Undefined variable ID is used: {id:?}");
                }
            }
        }
        // Validate disjointness of indicator active/removed
        for id in self.removed_indicator_constraints.keys() {
            if self.indicator_constraints.contains_key(id) {
                crate::bail!(
                    { ?id },
                    "Indicator constraint ID {id:?} is in both indicator_constraints and removed_indicator_constraints, but they must be disjoint",
                );
            }
        }

        // Validate one-hot constraints
        for (id, value) in &self.one_hot_constraints {
            if value.variables.is_empty() {
                crate::bail!(
                    { ?id },
                    "One-hot constraint {id:?} has no variables; one-hot constraints must contain at least one variable",
                );
            }
            for var_id in &value.variables {
                let Some(dv) = decision_variables.get(var_id) else {
                    crate::bail!(
                        { ?var_id },
                        "One-hot variable {var_id:?} is not defined in decision_variables",
                    );
                };
                if dv.kind() != crate::decision_variable::Kind::Binary {
                    crate::bail!({ ?var_id }, "One-hot variable {var_id:?} must be binary");
                }
            }
        }

        // Validate SOS1 constraints
        for (id, value) in &self.sos1_constraints {
            if value.variables.is_empty() {
                crate::bail!(
                    { ?id },
                    "SOS1 constraint {id:?} has no variables; SOS1 constraints must contain at least one variable",
                );
            }
            for var_id in &value.variables {
                if !variable_ids.contains(var_id) {
                    crate::bail!(
                        { ?var_id },
                        "SOS1 variable {var_id:?} is not defined in decision_variables",
                    );
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
            if !variable_ids.contains(&id) {
                crate::bail!(
                    { ?id },
                    "Variable ID {id:?} in decision_variable_dependency is not in decision_variables",
                );
            }
        }

        // Construction invariant: raw used, fixed, and dependent sets must be pairwise disjoint.
        // - used: IDs appearing in objective or constraints
        // - fixed: IDs in the root-owned fixed_decision_variable_values table
        // - dependent: keys of decision_variable_dependency
        let mut used: VariableIDSet = objective.required_ids().into_iter().collect();
        for constraint in constraints.values() {
            used.extend(constraint.required_ids());
        }
        for ic in self.indicator_constraints.values() {
            used.extend(ic.required_ids());
        }
        for oh in self.one_hot_constraints.values() {
            used.extend(oh.required_ids());
        }
        for sos1 in self.sos1_constraints.values() {
            used.extend(sos1.required_ids());
        }
        let fixed: VariableIDSet = fixed_decision_variable_values.keys().copied().collect();
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
                "Fixed variable {id:?} cannot be used in objectives or constraints",
            );
        }

        // Check fixed ∩ dependent = ∅
        if let Some(id) = fixed.intersection(&dependent).next() {
            crate::bail!(
                { ?id },
                "Variable {id:?} cannot be both fixed and dependent",
            );
        }

        Ok(Instance {
            sense,
            objective,
            decision_variables,
            variable_labels: self.variable_labels,
            fixed_decision_variable_values,
            constraint_collection: ConstraintCollection::with_context(
                constraints,
                self.removed_constraints,
                self.constraint_context,
            )?,
            indicator_constraint_collection: ConstraintCollection::with_context(
                self.indicator_constraints,
                self.removed_indicator_constraints,
                self.indicator_constraint_context,
            )?,
            one_hot_constraint_collection: ConstraintCollection::with_context(
                self.one_hot_constraints,
                BTreeMap::new(),
                self.one_hot_constraint_context,
            )?,
            sos1_constraint_collection: ConstraintCollection::with_context(
                self.sos1_constraints,
                BTreeMap::new(),
                self.sos1_constraint_context,
            )?,
            named_functions,
            decision_variable_dependency: self.decision_variable_dependency,
            parameters: self.parameters,
            description: self.description,
            annotations: Default::default(),
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
    use std::collections::BTreeSet;

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
        assert!(
            err.to_string().contains("missing: sense"),
            "unexpected error: {err}"
        );

        // Missing objective
        let err = Instance::builder()
            .sense(Sense::Minimize)
            .decision_variables(BTreeMap::new())
            .constraints(BTreeMap::new())
            .build()
            .unwrap_err();
        assert!(
            err.to_string().contains("missing: objective"),
            "unexpected error: {err}"
        );
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
    fn test_builder_preserves_labels_and_context() {
        let var_id = VariableID::from(1);
        let constraint_id = ConstraintID::from(2);
        let mut variable_labels = VariableLabelStore::default();
        variable_labels.set_name(var_id, "x");
        variable_labels.set_subscripts(var_id, vec![0]);
        let mut constraint_context = ConstraintContextStore::default();
        constraint_context.set_name(constraint_id, "balance");

        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([(var_id, DecisionVariable::binary())]))
            .variable_labels(variable_labels)
            .constraints(BTreeMap::from([(
                constraint_id,
                Constraint::equal_to_zero(Function::Zero),
            )]))
            .constraint_context(constraint_context)
            .build()
            .unwrap();

        assert_eq!(instance.variable_labels().name(var_id), Some("x"));
        assert_eq!(instance.variable_labels().subscripts(var_id), &[0]);
        assert_eq!(
            instance.constraint_context().name(constraint_id),
            Some("balance")
        );
    }

    #[test]
    fn test_builder_preserves_special_constraint_contexts() {
        let var_id = VariableID::from(1);
        let indicator_id = crate::IndicatorConstraintID::from(10);
        let one_hot_id = crate::OneHotConstraintID::from(11);
        let sos1_id = crate::Sos1ConstraintID::from(12);

        let mut indicator_context = ConstraintContextStore::default();
        indicator_context.set_name(indicator_id, "activation");
        let mut one_hot_context = ConstraintContextStore::default();
        one_hot_context.set_name(one_hot_id, "choice");
        let mut sos1_context = ConstraintContextStore::default();
        sos1_context.set_name(sos1_id, "exclusive");

        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([(var_id, DecisionVariable::binary())]))
            .constraints(BTreeMap::new())
            .indicator_constraints(BTreeMap::from([(
                indicator_id,
                crate::IndicatorConstraint::new(
                    var_id,
                    crate::Equality::LessThanOrEqualToZero,
                    Function::Zero,
                ),
            )]))
            .indicator_constraint_context(indicator_context)
            .one_hot_constraints(BTreeMap::from([(
                one_hot_id,
                crate::OneHotConstraint::new(BTreeSet::from([var_id])).unwrap(),
            )]))
            .one_hot_constraint_context(one_hot_context)
            .sos1_constraints(BTreeMap::from([(
                sos1_id,
                crate::Sos1Constraint::new(BTreeSet::from([var_id])).unwrap(),
            )]))
            .sos1_constraint_context(sos1_context)
            .build()
            .unwrap();

        assert_eq!(
            instance.indicator_constraint_context().name(indicator_id),
            Some("activation")
        );
        assert_eq!(
            instance.one_hot_constraint_context().name(one_hot_id),
            Some("choice")
        );
        assert_eq!(
            instance.sos1_constraint_context().name(sos1_id),
            Some("exclusive")
        );
    }

    #[test]
    fn test_builder_rejects_orphan_variable_labels() {
        let mut variable_labels = VariableLabelStore::default();
        variable_labels.set_name(VariableID::from(99), "orphan");

        let err = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .variable_labels(variable_labels)
            .constraints(BTreeMap::new())
            .build()
            .unwrap_err();

        assert!(
            err.to_string().contains("unknown decision variable ID")
                && err.to_string().contains("VariableID(99)"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_builder_rejects_orphan_named_function_labels() {
        let mut named_function_labels = crate::named_function::NamedFunctionLabelStore::default();
        named_function_labels.set_name(NamedFunctionID::from(99), "orphan");

        let err = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .constraints(BTreeMap::new())
            .named_function_labels(named_function_labels)
            .build()
            .unwrap_err();

        assert!(
            err.to_string().contains("unknown named function ID")
                && err.to_string().contains("NamedFunctionID(99)"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_builder_rejects_orphan_constraint_context() {
        let mut constraint_context = ConstraintContextStore::default();
        constraint_context.set_name(ConstraintID::from(99), "orphan");

        let err = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .constraints(BTreeMap::new())
            .constraint_context(constraint_context)
            .build()
            .unwrap_err();

        assert!(
            err.to_string().contains("unknown constraint ID")
                && err.to_string().contains("ConstraintID(99)"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_builder_rejects_orphan_special_constraint_contexts() {
        let mut indicator_context = ConstraintContextStore::default();
        indicator_context.set_name(crate::IndicatorConstraintID::from(99), "orphan");
        let err = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .constraints(BTreeMap::new())
            .indicator_constraint_context(indicator_context)
            .build()
            .unwrap_err();
        assert!(
            err.to_string().contains("unknown constraint ID")
                && err.to_string().contains("IndicatorConstraintID(99)"),
            "unexpected error: {err}"
        );

        let mut one_hot_context = ConstraintContextStore::default();
        one_hot_context.set_name(crate::OneHotConstraintID::from(99), "orphan");
        let err = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .constraints(BTreeMap::new())
            .one_hot_constraint_context(one_hot_context)
            .build()
            .unwrap_err();
        assert!(
            err.to_string().contains("unknown constraint ID")
                && err.to_string().contains("OneHotConstraintID(99)"),
            "unexpected error: {err}"
        );

        let mut sos1_context = ConstraintContextStore::default();
        sos1_context.set_name(crate::Sos1ConstraintID::from(99), "orphan");
        let err = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .constraints(BTreeMap::new())
            .sos1_constraint_context(sos1_context)
            .build()
            .unwrap_err();
        assert!(
            err.to_string().contains("unknown constraint ID")
                && err.to_string().contains("Sos1ConstraintID(99)"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_sidecar_setters_reject_unknown_ids() {
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .constraints(BTreeMap::new())
            .build()
            .unwrap();

        let err = instance
            .set_variable_label(VariableID::from(99), ModelingLabel::default())
            .unwrap_err();
        assert!(err.to_string().contains("unknown decision variable ID"));

        let err = instance
            .set_constraint_context(ConstraintID::from(99), ConstraintContext::default())
            .unwrap_err();
        assert!(err.to_string().contains("unknown constraint ID"));

        let err = instance
            .set_one_hot_constraint_context(
                crate::OneHotConstraintID::from(99),
                ConstraintContext::default(),
            )
            .unwrap_err();
        assert!(err.to_string().contains("unknown one-hot constraint ID"));
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

        let msg = err.to_string();
        assert!(
            msg.contains("Undefined variable ID") && msg.contains("999"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn test_builder_overlapping_constraint_ids() {
        use crate::Constraint;
        use maplit::btreemap;

        let constraint_id = ConstraintID::from(1);
        let constraint = Constraint::equal_to_zero(Function::Zero);
        let removed_constraint = Constraint::equal_to_zero(Function::Zero);
        let _ = constraint_id;
        let removed_reason = crate::constraint::RemovedReason {
            reason: "test".to_string(),
            parameters: Default::default(),
        };

        let err = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .constraints(btreemap! { constraint_id => constraint })
            .removed_constraints(
                btreemap! { constraint_id => (removed_constraint, removed_reason) },
            )
            .build()
            .unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("both constraints and removed_constraints")
                && msg.contains(&format!("{:?}", constraint_id)),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn test_builder_rejects_empty_one_hot() {
        use maplit::btreemap;

        let var_id = VariableID::from(1);
        let empty_one_hot = crate::OneHotConstraint {
            variables: std::collections::BTreeSet::new(),
            stage: crate::OneHotCreatedData,
        };

        let err = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(btreemap! {
                var_id => DecisionVariable::binary(),
            })
            .constraints(BTreeMap::new())
            .one_hot_constraints(btreemap! {
                crate::OneHotConstraintID::from(42) => empty_one_hot,
            })
            .build()
            .unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("no variables") && msg.contains("42"),
            "expected empty one-hot error mentioning the id, got: {msg}"
        );
    }

    #[test]
    fn test_builder_undefined_variable_dependency() {
        use maplit::btreemap;

        let var_id = VariableID::from(1);
        let undefined_var_id = VariableID::from(999);
        let decision_variables = btreemap! {
            var_id => DecisionVariable::binary(),
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

        let msg = err.to_string();
        assert!(
            msg.contains("decision_variable_dependency is not in decision_variables")
                && msg.contains(&format!("{:?}", undefined_var_id)),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn test_builder_valid_variable_dependency() {
        use maplit::btreemap;

        let var_id = VariableID::from(1);
        let decision_variables = btreemap! {
            var_id => DecisionVariable::binary(),
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
            var_id => DecisionVariable::binary(),
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

        let msg = err.to_string();
        assert!(
            msg.contains("Dependent variable cannot be used")
                && msg.contains(&format!("{:?}", var_id)),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn test_builder_dependent_variable_used_in_constraint() {
        use crate::linear;
        use maplit::btreemap;

        let var_id = VariableID::from(1);
        let constraint_id = ConstraintID::from(1);
        let decision_variables = btreemap! {
            var_id => DecisionVariable::binary(),
        };

        // Create a dependency for var_id
        let dependency = AcyclicAssignments::new(btreemap! {
            var_id => Function::Zero,
        })
        .unwrap();

        // Constraint uses var_id, which is also a dependent variable - this should fail
        let constraint = Constraint::equal_to_zero(Function::from(linear!(1)));
        let err = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(decision_variables)
            .constraints(btreemap! { constraint_id => constraint })
            .decision_variable_dependency(dependency)
            .build()
            .unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("Dependent variable cannot be used")
                && msg.contains(&format!("{:?}", var_id)),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn test_builder_fixed_and_dependent_variable() {
        use maplit::btreemap;

        let var_id = VariableID::from(1);
        let dv = DecisionVariable::binary();
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
            .fixed_decision_variable_values(btreemap! {
                var_id => 1.0,
            })
            .constraints(BTreeMap::new())
            .decision_variable_dependency(dependency)
            .build()
            .unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("cannot be both fixed") && msg.contains(&format!("{:?}", var_id)),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn test_builder_undefined_variable_in_removed_constraint() {
        use maplit::btreemap;

        let constraint_id = ConstraintID::from(1);
        // Create a removed constraint that references undefined variable ID 999
        let removed_constraint =
            Constraint::equal_to_zero(Function::from(linear!(999) + coeff!(1.0)));
        let removed_reason = crate::constraint::RemovedReason {
            reason: "test".to_string(),
            parameters: Default::default(),
        };

        let err = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .constraints(BTreeMap::new())
            .removed_constraints(
                btreemap! { constraint_id => (removed_constraint, removed_reason) },
            )
            .build()
            .unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("Undefined variable ID") && msg.contains("999"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn test_builder_fixed_variable_used_in_objective() {
        use maplit::btreemap;

        let var_id = VariableID::from(1);
        let dv = DecisionVariable::binary();
        let decision_variables = btreemap! {
            var_id => dv,
        };

        // Objective uses var_id, which is fixed - this should fail
        let err = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(1)))
            .decision_variables(decision_variables)
            .fixed_decision_variable_values(btreemap! {
                var_id => 1.0,
            })
            .constraints(BTreeMap::new())
            .build()
            .unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("Fixed variable") && msg.contains(&format!("{:?}", var_id)),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn test_builder_undefined_variable_in_named_function() {
        use crate::{NamedFunction, NamedFunctionID};
        use maplit::btreemap;

        // Create a named function that references undefined variable ID 999
        let named_function = NamedFunction {
            function: Function::from(linear!(999) + coeff!(1.0)),
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

        let msg = err.to_string();
        assert!(
            msg.contains("Undefined variable ID") && msg.contains("999"),
            "unexpected error: {msg}"
        );
    }
}
