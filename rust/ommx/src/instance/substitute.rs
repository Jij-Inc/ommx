use super::*;
use crate::{
    substitute_acyclic, substitute_one_via_acyclic, Function, IndicatorConstraintID,
    NamedFunctionID, Substitute, SubstitutionError, VariableID,
};
use std::collections::{BTreeMap, BTreeSet};

/// All fallible results of substituting an [`Instance`].
///
/// The root object prepares this plan without mutation, then commits only
/// table-local replacements. This keeps substitution atomic without granting
/// crate-wide access to a partially mutating `Instance` operation.
pub(super) struct InstanceSubstitutionPlan {
    objective: Option<Function>,
    constraint_replacements: BTreeMap<ConstraintID, Constraint>,
    indicator_replacements: BTreeMap<IndicatorConstraintID, IndicatorConstraint>,
    named_function_replacements: BTreeMap<NamedFunctionID, NamedFunction>,
    decision_variable_dependency: crate::AcyclicAssignments,
}

impl Instance {
    /// Prepare every fallible rewrite required by an acyclic substitution.
    pub(super) fn plan_substitution(
        &self,
        acyclic: &crate::AcyclicAssignments,
    ) -> Result<InstanceSubstitutionPlan, crate::SubstitutionError> {
        debug_assert!(!acyclic.is_empty());
        let substituted_variables: BTreeSet<VariableID> =
            acyclic.iter().map(|(var_id, _)| *var_id).collect();

        // Structural special-constraint variables cannot be substituted. Check
        // every family while the operation is still read-only.
        for (&constraint_id, constraint) in self.indicator_constraint_collection.active() {
            if substituted_variables.contains(&constraint.indicator_variable) {
                return Err(SubstitutionError::IndicatorVariableSubstitution {
                    indicator_variable: constraint.indicator_variable,
                    constraint_id,
                });
            }
        }
        for (&constraint_id, constraint) in self.one_hot_constraint_collection.active() {
            for &variable in &constraint.variables {
                if substituted_variables.contains(&variable) {
                    return Err(SubstitutionError::OneHotVariableSubstitution {
                        variable,
                        constraint_id,
                    });
                }
            }
        }
        for (&constraint_id, constraint) in self.sos1_constraint_collection.active() {
            for &variable in &constraint.variables {
                if substituted_variables.contains(&variable) {
                    return Err(SubstitutionError::Sos1VariableSubstitution {
                        variable,
                        constraint_id,
                    });
                }
            }
        }

        let objective = if self
            .objective
            .required_ids()
            .is_disjoint(&substituted_variables)
        {
            None
        } else {
            Some(self.objective.clone().substitute_acyclic(acyclic)?)
        };

        // Removed constraints are intentionally not rewritten here; they are
        // normalized through `restore_constraint` when reactivated.
        let mut constraint_replacements = BTreeMap::new();
        for (&constraint_id, constraint) in self.constraint_collection.active() {
            if !constraint
                .required_ids()
                .is_disjoint(&substituted_variables)
            {
                let mut replacement = constraint.clone();
                replacement.stage.function =
                    replacement.stage.function.substitute_acyclic(acyclic)?;
                constraint_replacements.insert(constraint_id, replacement);
            }
        }

        let mut indicator_replacements = BTreeMap::new();
        for (&constraint_id, constraint) in self.indicator_constraint_collection.active() {
            if !constraint
                .stage
                .function
                .required_ids()
                .is_disjoint(&substituted_variables)
            {
                let mut replacement = constraint.clone();
                replacement.stage.function =
                    replacement.stage.function.substitute_acyclic(acyclic)?;
                indicator_replacements.insert(constraint_id, replacement);
            }
        }

        let mut named_function_replacements = BTreeMap::new();
        for (&id, named_function) in self.named_functions.iter() {
            if !named_function
                .function
                .required_ids()
                .is_disjoint(&substituted_variables)
            {
                named_function_replacements
                    .insert(id, named_function.clone().substitute_acyclic(acyclic)?);
            }
        }

        let decision_variable_dependency = self
            .decision_variable_dependency
            .clone()
            .substitute_acyclic(acyclic)?;

        Ok(InstanceSubstitutionPlan {
            objective,
            constraint_replacements,
            indicator_replacements,
            named_function_replacements,
            decision_variable_dependency,
        })
    }

    /// Commit a fully validated substitution plan using table-local effects.
    pub(super) fn commit_substitution(&mut self, plan: InstanceSubstitutionPlan) {
        if let Some(objective) = plan.objective {
            self.objective = objective;
        }
        self.constraint_collection
            .replace_active_rows(plan.constraint_replacements)
            .expect("replacement IDs were read from active constraints");
        self.indicator_constraint_collection
            .replace_active_rows(plan.indicator_replacements)
            .expect("replacement IDs were read from active indicator constraints");
        self.named_functions
            .replace_rows(plan.named_function_replacements)
            .expect("replacement IDs were read from named functions");
        self.decision_variable_dependency = plan.decision_variable_dependency;
    }
}

impl Substitute for Instance {
    type Output = Self;

    fn substitute_acyclic(
        mut self,
        acyclic: &crate::AcyclicAssignments,
    ) -> Result<Self::Output, crate::SubstitutionError> {
        if acyclic.is_empty() {
            return Ok(self);
        }
        let plan = self.plan_substitution(acyclic)?;
        self.commit_substitution(plan);
        Ok(self)
    }

    fn substitute_one(
        self,
        assigned: VariableID,
        f: &Function,
    ) -> Result<Self::Output, SubstitutionError> {
        substitute_one_via_acyclic(self, assigned, f)
    }
}

impl Substitute for ParametricInstance {
    type Output = Self;

    fn substitute_acyclic(
        mut self,
        acyclic: &crate::AcyclicAssignments,
    ) -> Result<Self::Output, crate::SubstitutionError> {
        if acyclic.is_empty() {
            return Ok(self);
        }

        let substituted_variables: std::collections::BTreeSet<VariableID> =
            acyclic.iter().map(|(var_id, _)| *var_id).collect();

        for (var_id, function) in acyclic.iter() {
            if self.parameters.contains_key(var_id) {
                return Err(SubstitutionError::ParameterSubstitution { parameter: *var_id });
            }
            if !self.decision_variables.contains_key(var_id) {
                return Err(SubstitutionError::UndefinedSubstitutionVariable { variable: *var_id });
            }
            for required_id in function.required_ids() {
                if !self.decision_variables.contains_key(&required_id)
                    && !self.parameters.contains_key(&required_id)
                {
                    return Err(SubstitutionError::UndefinedSubstitutionVariable {
                        variable: required_id,
                    });
                }
            }
        }

        let mut affected_constraint_ids = std::collections::BTreeSet::new();
        for (constraint_id, constraint) in self.constraint_collection.active() {
            let required_ids = constraint.required_ids();
            if !required_ids.is_disjoint(&substituted_variables) {
                affected_constraint_ids.insert(*constraint_id);
            }
        }

        substitute_acyclic(&mut self.objective, acyclic)?;

        let mut constraint_replacements = BTreeMap::new();
        for (&constraint_id, constraint) in self.constraint_collection.active() {
            if affected_constraint_ids.contains(&constraint_id) {
                let mut constraint = constraint.clone();
                substitute_acyclic(&mut constraint.stage.function, acyclic)?;
                constraint_replacements.insert(constraint_id, constraint);
            }
        }
        self.constraint_collection
            .replace_active_rows(constraint_replacements)
            .expect("replacement IDs were read from active constraints");

        for (&cid, ic) in self.indicator_constraint_collection.active().iter() {
            if substituted_variables.contains(&ic.indicator_variable) {
                return Err(SubstitutionError::IndicatorVariableSubstitution {
                    indicator_variable: ic.indicator_variable,
                    constraint_id: cid,
                });
            }
        }
        let mut indicator_replacements = BTreeMap::new();
        for (&constraint_id, ic) in self.indicator_constraint_collection.active() {
            let required_ids = ic.stage.function.required_ids();
            if !required_ids.is_disjoint(&substituted_variables) {
                let mut ic = ic.clone();
                substitute_acyclic(&mut ic.stage.function, acyclic)?;
                indicator_replacements.insert(constraint_id, ic);
            }
        }
        self.indicator_constraint_collection
            .replace_active_rows(indicator_replacements)
            .expect("replacement IDs were read from active indicator constraints");

        for (&cid, oh) in self.one_hot_constraint_collection.active().iter() {
            for var_id in &oh.variables {
                if substituted_variables.contains(var_id) {
                    return Err(SubstitutionError::OneHotVariableSubstitution {
                        variable: *var_id,
                        constraint_id: cid,
                    });
                }
            }
        }
        for (&cid, sos1) in self.sos1_constraint_collection.active().iter() {
            for var_id in &sos1.variables {
                if substituted_variables.contains(var_id) {
                    return Err(SubstitutionError::Sos1VariableSubstitution {
                        variable: *var_id,
                        constraint_id: cid,
                    });
                }
            }
        }

        // Apply substitution to named functions and existing dependencies.
        substitute_acyclic(&mut self.named_functions, acyclic)?;
        substitute_acyclic(&mut self.decision_variable_dependency, acyclic)?;

        Ok(self)
    }

    fn substitute_one(
        self,
        assigned: VariableID,
        f: &Function,
    ) -> Result<Self::Output, SubstitutionError> {
        substitute_one_via_acyclic(self, assigned, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, constraint::Equality, linear, DecisionVariable, Evaluate, Sense};
    use std::collections::BTreeMap;

    #[test]
    fn test_instance_substitute() {
        // Create decision variables
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(VariableID::from(1), DecisionVariable::continuous());
        decision_variables.insert(VariableID::from(2), DecisionVariable::continuous());

        // Create a simple instance: minimize x1 + 2*x2, subject to x1 + x2 <= 10
        let objective = Function::from((linear!(1) + coeff!(2.0) * linear!(2)).unwrap());
        let constraint_function =
            Function::from(((linear!(1) + linear!(2)).unwrap() + coeff!(-10.0)).unwrap());

        let mut constraints = BTreeMap::new();
        let constraint = Constraint {
            equality: Equality::LessThanOrEqualToZero,
            stage: crate::constraint::CreatedData {
                function: constraint_function,
            },
        };
        constraints.insert(ConstraintID::from(1), constraint);

        let mut instance =
            Instance::new(Sense::Minimize, objective, decision_variables, constraints).unwrap();
        let named_function_id = instance
            .new_named_function(
                Function::from((linear!(1) + linear!(2)).unwrap()),
                Some("tracked".to_string()),
                vec![],
                Default::default(),
                None,
            )
            .unwrap();

        // Substitute x1 with x3 + 1
        let substitution = Function::from(linear!(3) + coeff!(1.0));
        let result = instance
            .substitute_one(VariableID::from(1), &substitution)
            .unwrap();

        // Check that the decision_variable_dependency contains the assignment x1 <- x3 + 1
        assert_eq!(result.decision_variable_dependency.len(), 1);
        assert!(result
            .decision_variable_dependency
            .get(&VariableID::from(1))
            .is_some());

        let named_function = result.named_functions().get(&named_function_id).unwrap();
        let expected_ids: std::collections::BTreeSet<_> =
            [VariableID::from(2), VariableID::from(3)]
                .into_iter()
                .collect();
        assert_eq!(named_function.required_ids(), expected_ids);
    }

    #[test]
    fn test_parametric_instance_substitute_parameterized_rhs() {
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(VariableID::from(0), DecisionVariable::continuous());
        decision_variables.insert(VariableID::from(1), DecisionVariable::continuous());

        let parameters = ParameterTable::from_ids([VariableID::from(100)].into_iter().collect());

        let objective = Function::from(linear!(0) + linear!(100));
        let parametric = ParametricInstance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            parameters,
            BTreeMap::new(),
        )
        .unwrap();

        let substituted = parametric
            .substitute_one(
                VariableID::from(0),
                &Function::from(linear!(1) + linear!(100)),
            )
            .unwrap();

        assert_eq!(substituted.decision_variable_dependency.len(), 1);
        assert!(substituted
            .decision_variable_dependency
            .get(&VariableID::from(0))
            .is_some());

        let mut parameter_values = crate::v1::Parameters::default();
        parameter_values.entries.insert(100, 2.0);
        let instance = substituted.with_parameters(parameter_values).unwrap();
        let state = crate::v1::State::from_iter([(1, 3.0)]);
        let value = instance
            .objective()
            .evaluate(&state, crate::ATol::default())
            .unwrap();
        assert_eq!(value, 7.0);
    }

    #[test]
    fn test_parametric_instance_substitute_parameter_target_fails() {
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(VariableID::from(0), DecisionVariable::continuous());

        let parameters = ParameterTable::from_ids([VariableID::from(100)].into_iter().collect());

        let parametric = ParametricInstance::new(
            Sense::Minimize,
            Function::from(linear!(0) + linear!(100)),
            decision_variables,
            parameters,
            BTreeMap::new(),
        )
        .unwrap();

        let err = parametric
            .substitute_one(VariableID::from(100), &Function::from(linear!(0)))
            .unwrap_err();
        assert!(matches!(
            err,
            SubstitutionError::ParameterSubstitution { parameter }
                if parameter == VariableID::from(100)
        ));
    }

    #[test]
    fn test_parametric_instance_substitute_undefined_rhs_fails() {
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(VariableID::from(0), DecisionVariable::continuous());

        let parametric = ParametricInstance::new(
            Sense::Minimize,
            Function::from(linear!(0)),
            decision_variables,
            ParameterTable::default(),
            BTreeMap::new(),
        )
        .unwrap();

        let err = parametric
            .substitute_one(VariableID::from(0), &Function::from(linear!(999)))
            .unwrap_err();
        assert!(matches!(
            err,
            SubstitutionError::UndefinedSubstitutionVariable { variable }
                if variable == VariableID::from(999)
        ));
    }

    #[test]
    fn test_substitute_indicator_function() {
        // Substituting a variable in the indicator's function should work
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(VariableID::from(1), DecisionVariable::continuous());
        decision_variables.insert(VariableID::from(2), DecisionVariable::continuous());
        decision_variables.insert(VariableID::from(10), DecisionVariable::binary());

        let objective = Function::from(linear!(1));

        let mut indicator_constraints = BTreeMap::new();
        indicator_constraints.insert(
            crate::IndicatorConstraintID::from(1),
            crate::IndicatorConstraint::new(
                VariableID::from(10),
                Equality::LessThanOrEqualToZero,
                Function::from(linear!(1) + coeff!(-5.0)),
            ),
        );

        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(objective)
            .decision_variables(decision_variables)
            .constraints(BTreeMap::new())
            .indicator_constraints(indicator_constraints)
            .build()
            .unwrap();

        // Substitute x1 = x2 + 1
        let assignments = crate::AcyclicAssignments::new(vec![(
            VariableID::from(1),
            Function::from(linear!(2) + coeff!(1.0)),
        )])
        .unwrap();

        let result = instance.substitute_acyclic(&assignments).unwrap();

        // Indicator constraint should still exist with substituted function
        assert_eq!(result.indicator_constraints().len(), 1);
    }

    #[test]
    fn test_substitute_indicator_variable_fails() {
        // Substituting the indicator variable itself should fail
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(VariableID::from(1), DecisionVariable::continuous());
        decision_variables.insert(VariableID::from(10), DecisionVariable::binary());

        let objective = Function::from(linear!(1));

        let mut indicator_constraints = BTreeMap::new();
        indicator_constraints.insert(
            crate::IndicatorConstraintID::from(1),
            crate::IndicatorConstraint::new(
                VariableID::from(10),
                Equality::LessThanOrEqualToZero,
                Function::from(linear!(1) + coeff!(-5.0)),
            ),
        );

        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(objective)
            .decision_variables(decision_variables)
            .constraints(BTreeMap::new())
            .indicator_constraints(indicator_constraints)
            .build()
            .unwrap();

        // Try to substitute the indicator variable x10
        let assignments = crate::AcyclicAssignments::new(vec![(
            VariableID::from(10),
            Function::from(coeff!(1.0)),
        )])
        .unwrap();

        let result = instance.substitute_acyclic(&assignments);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SubstitutionError::IndicatorVariableSubstitution {
                indicator_variable,
                constraint_id,
            } if indicator_variable == VariableID::from(10)
                && constraint_id == crate::IndicatorConstraintID::from(1)
        ));
    }
}
