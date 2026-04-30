use super::*;
use crate::constraint_type::ConstraintCollection;
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
            variable_metadata,
            constraint_collection,
            indicator_constraint_collection,
            one_hot_constraint_collection,
            sos1_constraint_collection,
            decision_variable_dependency,
            description,
            named_functions,
            named_function_metadata,
            ..
        }: Instance,
    ) -> Self {
        ParametricInstance {
            sense,
            objective,
            decision_variables,
            parameters: BTreeMap::default(),
            variable_metadata,
            constraint_collection,
            indicator_constraint_collection,
            one_hot_constraint_collection,
            sos1_constraint_collection,
            decision_variable_dependency,
            description,
            named_functions,
            named_function_metadata,
        }
    }
}

impl ParametricInstance {
    pub fn with_parameters(self, parameters: crate::v1::Parameters) -> crate::Result<Instance> {
        use crate::ATol;
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
            let missing_ids: Vec<VariableID> =
                required_ids.difference(&given_ids).cloned().collect();
            crate::bail!(
                { ?missing_ids },
                "Missing parameters: required IDs {required_ids:?}, got {given_ids:?}",
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

        // Both active and removed regular constraint bodies need the parameter
        // substitution applied — otherwise the resulting `Instance` would
        // carry dangling parameter IDs in `removed_constraints`, violating
        // its own invariants.
        let (mut constraints, mut removed_constraints, constraint_metadata) =
            self.constraint_collection.into_parts();
        for (_, constraint) in constraints.iter_mut() {
            constraint.stage.function.partial_evaluate(&state, atol)?;
        }
        for (_, (constraint, _reason)) in removed_constraints.iter_mut() {
            constraint.stage.function.partial_evaluate(&state, atol)?;
        }

        // Indicator constraint function bodies may also reference parameter
        // IDs (the structural indicator variable does not, by construction).
        // Apply the same substitution to active and removed maps.
        let (mut indicator_active, mut indicator_removed, indicator_metadata) =
            self.indicator_constraint_collection.into_parts();
        for (_, ic) in indicator_active.iter_mut() {
            ic.stage.function.partial_evaluate(&state, atol)?;
        }
        for (_, (ic, _reason)) in indicator_removed.iter_mut() {
            ic.stage.function.partial_evaluate(&state, atol)?;
        }

        let mut named_functions = self.named_functions;
        for (_, named_function) in named_functions.iter_mut() {
            named_function.partial_evaluate(&state, atol)?;
        }

        // Decision-variable dependency RHS expressions can also reference
        // parameter IDs. Without substitution, dependent-variable
        // expressions in the resulting `Instance` would carry dangling
        // parameter references (the parametric builder doesn't restrict
        // `decision_variable_dependency` RHS bodies to decision-variable
        // IDs only).
        let mut decision_variable_dependency = self.decision_variable_dependency;
        decision_variable_dependency.partial_evaluate(&state, atol)?;

        Ok(Instance {
            sense: self.sense,
            objective,
            decision_variables: self.decision_variables,
            variable_metadata: self.variable_metadata,
            constraint_collection: ConstraintCollection::with_metadata(
                constraints,
                removed_constraints,
                constraint_metadata,
            ),
            indicator_constraint_collection: ConstraintCollection::with_metadata(
                indicator_active,
                indicator_removed,
                indicator_metadata,
            ),
            // OneHot / SOS1 constraints are purely structural — their
            // variable sets are always real decision variables (the
            // parametric builder rejects parameter IDs there), so there is
            // nothing to substitute and the collections pass through
            // unchanged.
            one_hot_constraint_collection: self.one_hot_constraint_collection,
            sos1_constraint_collection: self.sos1_constraint_collection,
            named_functions,
            named_function_metadata: self.named_function_metadata,
            decision_variable_dependency,
            parameters: Some(parameters),
            description: self.description,
        })
    }
}

#[cfg(test)]
mod with_parameters_tests {
    use super::*;
    use crate::{coeff, linear, Equality, Function};
    use maplit::btreemap;

    /// Parameter substitution must apply to the right-hand-side of
    /// `decision_variable_dependency` entries. The RHS is a `Function`
    /// over arbitrary IDs; the parametric builder doesn't restrict it to
    /// decision-variable IDs only, so a parameter reference there would
    /// dangle in the resulting `Instance` without explicit substitution.
    #[test]
    fn decision_variable_dependency_rhs_is_substituted() {
        use crate::AcyclicAssignments;
        let x = VariableID::from(1);
        let dep = VariableID::from(2);
        let p = VariableID::from(100);
        // Dependency: dep_var = x + p (RHS references a parameter).
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

        let mut params = crate::v1::Parameters::default();
        params.entries = std::collections::HashMap::from([(100, 1.0)]);
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

    /// Parameter substitution must apply to *removed* regular constraints
    /// as well. `ParametricInstance` permits removed-constraint bodies to
    /// reference parameters (function bodies are unrestricted), but the
    /// resulting `Instance` has no parameters at all — so any parameter id
    /// left in a removed body would dangle.
    #[test]
    fn removed_regular_constraint_body_is_substituted() {
        let x = VariableID::from(1);
        let p = VariableID::from(100);
        let c_active = Constraint::equal_to_zero(Function::from(linear!(1)));
        let c_removed = Constraint::equal_to_zero(Function::from(linear!(1) + linear!(100)));

        let parametric = ParametricInstance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(btreemap! {
                x => DecisionVariable::binary(x),
            })
            .parameters(btreemap! {
                p => crate::v1::Parameter { id: 100, ..Default::default() },
            })
            .constraints(btreemap! {
                ConstraintID::from(0) => c_active,
            })
            .removed_constraints(btreemap! {
                ConstraintID::from(1) => (
                    c_removed,
                    crate::constraint::RemovedReason {
                        reason: "test".to_string(),
                        parameters: Default::default(),
                    },
                ),
            })
            .build()
            .unwrap();

        let mut params = crate::v1::Parameters::default();
        params.entries = std::collections::HashMap::from([(100, 1.0)]);
        let instance = parametric.with_parameters(params).unwrap();

        let (rc, _r) = instance
            .removed_constraints()
            .get(&ConstraintID::from(1))
            .unwrap();
        let body_required: VariableIDSet = rc.stage.function.required_ids();
        assert!(
            !body_required.contains(&p),
            "parameter id {p:?} survived in removed-constraint body: {body_required:?}",
        );
    }

    /// Parameter substitution must apply to *removed* indicator constraints
    /// too — the parametric builder accepts a removed-indicator map and the
    /// `convert_*` paths can populate it. Without substitution, a
    /// parameter id in a removed indicator body would dangle in the
    /// materialized `Instance`.
    #[test]
    fn removed_indicator_function_body_is_substituted() {
        let y = VariableID::from(1);
        let x = VariableID::from(2);
        let p = VariableID::from(100);
        let indicator = crate::IndicatorConstraint::new(
            y,
            Equality::EqualToZero,
            Function::from(linear!(2) + linear!(100)),
        );

        let parametric = ParametricInstance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(btreemap! {
                y => DecisionVariable::binary(y),
                x => DecisionVariable::binary(x),
            })
            .parameters(btreemap! {
                p => crate::v1::Parameter { id: 100, ..Default::default() },
            })
            .constraints(BTreeMap::new())
            .removed_indicator_constraints(btreemap! {
                crate::IndicatorConstraintID::from(0) => (
                    indicator,
                    crate::constraint::RemovedReason {
                        reason: "test".to_string(),
                        parameters: Default::default(),
                    },
                ),
            })
            .build()
            .unwrap();

        let mut params = crate::v1::Parameters::default();
        params.entries = std::collections::HashMap::from([(100, 1.0)]);
        let instance = parametric.with_parameters(params).unwrap();

        let (ic, _r) = instance
            .removed_indicator_constraints()
            .get(&crate::IndicatorConstraintID::from(0))
            .expect("removed indicator survives materialization");
        let body_required: VariableIDSet = ic.stage.function.required_ids();
        assert!(
            !body_required.contains(&p),
            "parameter id {p:?} survived in removed-indicator body: {body_required:?}",
        );
    }

    /// `ParametricInstance::with_parameters` must substitute parameter IDs
    /// inside *indicator* function bodies, not just the objective and
    /// regular constraint bodies. Otherwise the resulting `Instance`
    /// carries dangling parameter IDs in its active indicator collection
    /// and breaks its own invariants.
    #[test]
    fn indicator_function_body_is_substituted() {
        // Indicator: y = 1 ⇒ (x + p - 1) == 0, where p is a parameter.
        // After substituting p = 1, the body should read x + 0 = x.
        let y = VariableID::from(1);
        let x = VariableID::from(2);
        let p = VariableID::from(100);
        let indicator = crate::IndicatorConstraint::new(
            y,
            Equality::EqualToZero,
            Function::from(linear!(2) + linear!(100) + coeff!(-1.0)),
        );

        let parametric = ParametricInstance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(btreemap! {
                y => DecisionVariable::binary(y),
                x => DecisionVariable::binary(x),
            })
            .parameters(btreemap! {
                p => crate::v1::Parameter { id: 100, ..Default::default() },
            })
            .constraints(BTreeMap::new())
            .indicator_constraints(btreemap! {
                crate::IndicatorConstraintID::from(0) => indicator,
            })
            .build()
            .unwrap();

        let mut params = crate::v1::Parameters::default();
        params.entries = std::collections::HashMap::from([(100, 1.0)]);
        let instance = parametric.with_parameters(params).unwrap();

        // After substitution, the indicator body must no longer reference
        // the parameter id 100.
        let materialized = instance
            .indicator_constraints()
            .get(&crate::IndicatorConstraintID::from(0))
            .unwrap();
        let body_required: VariableIDSet = materialized.stage.function.required_ids();
        assert!(
            !body_required.contains(&p),
            "parameter id {p:?} survived in indicator body after with_parameters: {body_required:?}",
        );
        assert!(
            body_required.contains(&x),
            "decision variable id {x:?} should remain in indicator body: {body_required:?}",
        );
    }
}
