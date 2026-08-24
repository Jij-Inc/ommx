use super::*;
use crate::{
    constraint_type::{ConstraintCollection, ConstraintType},
    ATol, Evaluate,
};
use std::{collections::BTreeMap, ops::Neg};

fn convert_objective_pair(sense: &mut Sense, objective: &mut Function, target: Sense) -> bool {
    if *sense == target {
        false
    } else {
        *sense = target;
        *objective = std::mem::take(objective).neg();
        true
    }
}

impl Instance {
    /// Convert only the active, solver-facing objective to `target`.
    ///
    /// # Postconditions
    ///
    /// Only the active pair changes, while evaluation retains the entry output semantics.
    ///
    /// ```
    /// use ommx::{
    ///     linear, v1::State, ATol, DecisionVariable, Evaluate, Function, Instance,
    ///     Sampled, Sense, VariableID,
    /// };
    /// use std::collections::{BTreeMap, HashMap};
    ///
    /// let original = Function::from(linear!(1));
    /// let mut instance = Instance::builder()
    ///     .sense(Sense::Maximize)
    ///     .objective(original.clone())
    ///     .decision_variables(BTreeMap::from([(
    ///         VariableID::from(1),
    ///         DecisionVariable::binary(),
    ///     )]))
    ///     .constraints(BTreeMap::new())
    ///     .build()
    ///     .unwrap();
    /// let state = State::from(HashMap::from([(1, 1.0)]));
    ///
    /// assert!(instance.convert_active_objective(Sense::Minimize));
    /// assert_eq!(instance.sense(), Sense::Minimize);
    /// assert_eq!(instance.objective().evaluate(&state, ATol::default()).unwrap(), -1.0);
    /// assert!(!instance.convert_active_objective(Sense::Minimize));
    ///
    /// let solution = instance.evaluate(&state, ATol::default()).unwrap();
    /// let sample_set = instance
    ///     .evaluate_samples(&Sampled::from(state), ATol::default())
    ///     .unwrap();
    /// assert_eq!(*solution.sense(), Some(Sense::Maximize));
    /// assert_eq!(*solution.objective(), 1.0);
    /// assert_eq!(*sample_set.sense(), Sense::Maximize);
    /// let sample_id = sample_set.sample_ids().into_iter().next().unwrap();
    /// assert_eq!(sample_set.objectives().get(sample_id), Some(&1.0));
    /// ```
    pub fn convert_active_objective(&mut self, target: Sense) -> bool {
        if self.sense == target {
            return false;
        }
        let captured = self.capture_output_objective();
        let converted = convert_objective_pair(&mut self.sense, &mut self.objective, target);
        if !captured {
            self.remove_redundant_output_objective();
        }
        converted
    }

    /// Convert the complete instance objective semantics to minimization.
    ///
    /// # Postconditions
    ///
    /// Both active and output objective semantics become minimization semantics.
    ///
    /// ```
    /// use ommx::{
    ///     linear, v1::State, ATol, DecisionVariable, Evaluate, Function, Instance,
    ///     Sense, VariableID,
    /// };
    /// use std::collections::{BTreeMap, HashMap};
    ///
    /// let mut instance = Instance::builder()
    ///     .sense(Sense::Maximize)
    ///     .objective(Function::from(linear!(1)))
    ///     .decision_variables(BTreeMap::from([(
    ///         VariableID::from(1),
    ///         DecisionVariable::binary(),
    ///     )]))
    ///     .constraints(BTreeMap::new())
    ///     .build()
    ///     .unwrap();
    /// let state = State::from(HashMap::from([(1, 1.0)]));
    ///
    /// assert!(instance.as_minimization_problem());
    /// assert_eq!(instance.sense(), Sense::Minimize);
    /// assert_eq!(instance.objective().evaluate(&state, ATol::default()).unwrap(), -1.0);
    /// let solution = instance.evaluate(&state, ATol::default()).unwrap();
    /// assert_eq!(*solution.sense(), Some(Sense::Minimize));
    /// assert_eq!(*solution.objective(), -1.0);
    /// assert!(!instance.as_minimization_problem());
    /// ```
    pub fn as_minimization_problem(&mut self) -> bool {
        self.convert_problem_objective(Sense::Minimize)
    }

    /// Convert the complete instance objective semantics to maximization.
    ///
    /// # Postconditions
    ///
    /// Both active and output objective semantics become maximization semantics.
    ///
    /// ```
    /// use ommx::{
    ///     linear, v1::State, ATol, DecisionVariable, Evaluate, Function, Instance,
    ///     Sense, VariableID,
    /// };
    /// use std::collections::{BTreeMap, HashMap};
    ///
    /// let mut instance = Instance::builder()
    ///     .sense(Sense::Minimize)
    ///     .objective(Function::from(linear!(1)))
    ///     .decision_variables(BTreeMap::from([(
    ///         VariableID::from(1),
    ///         DecisionVariable::binary(),
    ///     )]))
    ///     .constraints(BTreeMap::new())
    ///     .build()
    ///     .unwrap();
    /// let state = State::from(HashMap::from([(1, 1.0)]));
    ///
    /// assert!(instance.as_maximization_problem());
    /// assert_eq!(instance.sense(), Sense::Maximize);
    /// assert_eq!(instance.objective().evaluate(&state, ATol::default()).unwrap(), -1.0);
    /// let solution = instance.evaluate(&state, ATol::default()).unwrap();
    /// assert_eq!(*solution.sense(), Some(Sense::Maximize));
    /// assert_eq!(*solution.objective(), -1.0);
    /// assert!(!instance.as_maximization_problem());
    /// ```
    pub fn as_maximization_problem(&mut self) -> bool {
        self.convert_problem_objective(Sense::Maximize)
    }

    fn convert_problem_objective(&mut self, target: Sense) -> bool {
        let active_converted = convert_objective_pair(&mut self.sense, &mut self.objective, target);
        let output_converted = if let Some(output) = &mut self.output_objective {
            convert_objective_pair(&mut output.sense, &mut output.function, target)
        } else {
            false
        };
        let converted = active_converted || output_converted;
        if converted {
            self.remove_redundant_output_objective();
        }
        converted
    }
}

impl From<Instance> for ParametricInstance {
    fn from(
        Instance {
            sense,
            objective,
            output_objective,
            decision_variables,
            constraint_collection,
            indicator_constraint_collection,
            one_hot_constraint_collection,
            sos1_constraint_collection,
            decision_variable_dependency,
            description,
            annotations,
            named_functions,
            ..
        }: Instance,
    ) -> Self {
        ParametricInstance {
            sense,
            objective,
            output_objective,
            decision_variables,
            parameters: ParameterTable::default(),
            constraint_collection,
            indicator_constraint_collection,
            one_hot_constraint_collection,
            sos1_constraint_collection,
            decision_variable_dependency,
            description,
            annotations,
            named_functions,
        }
    }
}

fn materialize_constraint_collection_parameters<T: ConstraintType>(
    collection: &mut ConstraintCollection<T>,
    state: &crate::v1::State,
    atol: ATol,
) -> crate::Result<()> {
    let mut active_replacements = BTreeMap::new();
    for (&id, constraint) in collection.active() {
        let mut constraint = constraint.clone();
        constraint.partial_evaluate(state, atol).inspect_err(|e| {
            tracing::error!(?id, error = %e, "failed to partial_evaluate active constraint");
        })?;
        active_replacements.insert(id, constraint);
    }

    let mut removed_replacements = BTreeMap::new();
    for (&id, (constraint, _reason)) in collection.removed() {
        let mut constraint = constraint.clone();
        constraint.partial_evaluate(state, atol).inspect_err(|e| {
            tracing::error!(?id, error = %e, "failed to partial_evaluate removed constraint");
        })?;
        removed_replacements.insert(id, constraint);
    }

    collection.replace_rows_preserving_lifecycle(active_replacements, removed_replacements)
}

impl ParametricInstance {
    /// Materialize every parameter into an [`Instance`].
    ///
    /// # Postconditions
    ///
    /// Materialization removes parameter IDs from both active and output objectives.
    ///
    /// ```
    /// use ommx::{
    ///     linear, v1::{Parameters, State}, ATol, Constraint, ConstraintID,
    ///     DecisionVariable, Evaluate, Function, Instance, Sense, VariableID,
    /// };
    /// use std::collections::{BTreeMap, HashMap};
    ///
    /// let variable = VariableID::from(1);
    /// let source = Instance::builder()
    ///     .sense(Sense::Minimize)
    ///     .objective(Function::from(linear!(1)))
    ///     .decision_variables(BTreeMap::from([(variable, DecisionVariable::binary())]))
    ///     .constraints(BTreeMap::from([(
    ///         ConstraintID::from(1),
    ///         Constraint::equal_to_zero(Function::from(linear!(1))),
    ///     )]))
    ///     .build()
    ///     .unwrap();
    /// let parametric = source.uniform_penalty_method().unwrap();
    /// let penalty = *parametric.parameters().keys().next().unwrap();
    /// let mut parameters = Parameters::default();
    /// parameters.entries.insert(penalty.into_inner(), 2.0);
    /// let instance = parametric.with_parameters(parameters).unwrap();
    ///
    /// assert!(instance.objective().required_ids().contains(&variable));
    /// assert!(!instance.objective().required_ids().contains(&penalty));
    /// assert_eq!(instance.output_objective().unwrap().sense(), Sense::Minimize);
    /// assert!(!instance.output_objective().unwrap().preserves_optimality());
    /// let solution = instance
    ///     .evaluate(&State::from(HashMap::from([(1, 0.0)])), ATol::default())
    ///     .unwrap();
    /// assert_eq!(*solution.objective(), 0.0);
    /// ```
    pub fn with_parameters(self, parameters: crate::v1::Parameters) -> crate::Result<Instance> {
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

        // Partially evaluate the active and output objectives, constraints,
        // and named functions.
        let mut objective = self.objective;
        objective.partial_evaluate(&state, atol)?;
        let mut output_objective = self.output_objective;
        if let Some(output_objective) = &mut output_objective {
            output_objective.function.partial_evaluate(&state, atol)?;
        }

        // Both active and removed regular constraint bodies need the parameter
        // substitution applied — otherwise the resulting `Instance` would
        // carry dangling parameter IDs in `removed_constraints`, violating
        // its own invariants.
        let mut constraint_collection = self.constraint_collection;
        materialize_constraint_collection_parameters(&mut constraint_collection, &state, atol)?;

        // Indicator constraint function bodies may also reference parameter
        // IDs (the structural indicator variable does not, by construction).
        // Apply the same substitution to active and removed maps.
        let mut indicator_constraint_collection = self.indicator_constraint_collection;
        materialize_constraint_collection_parameters(
            &mut indicator_constraint_collection,
            &state,
            atol,
        )?;

        let mut named_functions = self.named_functions;
        named_functions.partial_evaluate(&state, atol)?;

        // Decision-variable dependency RHS expressions can also reference
        // parameter IDs. Without substitution, dependent-variable
        // expressions in the resulting `Instance` would carry dangling
        // parameter references.
        let mut decision_variable_dependency = self.decision_variable_dependency;
        decision_variable_dependency.partial_evaluate(&state, atol)?;

        let mut instance = Instance {
            sense: self.sense,
            objective,
            output_objective,
            decision_variables: self.decision_variables,
            constraint_collection,
            indicator_constraint_collection,
            // OneHot / SOS1 constraints are purely structural — their
            // variable sets are always real decision variables (the
            // parametric builder rejects parameter IDs there), so there is
            // nothing to substitute and the collections pass through
            // unchanged.
            one_hot_constraint_collection: self.one_hot_constraint_collection,
            sos1_constraint_collection: self.sos1_constraint_collection,
            named_functions,
            decision_variable_dependency,
            parameters: Some(parameters),
            description: self.description,
            annotations: self.annotations,
        };
        instance.remove_redundant_output_objective();
        Ok(instance)
    }
}

#[cfg(test)]
mod output_objective_tests {
    use super::*;
    use crate::{linear, v1::State, ATol, DecisionVariable, Evaluate, Sampled};
    use std::collections::{BTreeMap, HashMap};

    fn maximizing_binary_instance() -> Instance {
        Instance::builder()
            .sense(Sense::Maximize)
            .objective(Function::from(linear!(1)))
            .decision_variables(BTreeMap::from([(
                VariableID::from(1),
                DecisionVariable::binary(),
            )]))
            .constraints(BTreeMap::new())
            .build()
            .unwrap()
    }

    fn assert_evaluation(instance: &Instance, sense: Sense, objective: f64) {
        let state = State::from(HashMap::from([(1, 1.0)]));
        let solution = instance.evaluate(&state, ATol::default()).unwrap();
        assert_eq!(*solution.sense(), Some(sense));
        assert_eq!(*solution.objective(), objective);

        let sample_set = instance
            .evaluate_samples(&Sampled::from(state), ATol::default())
            .unwrap();
        assert_eq!(*sample_set.sense(), sense);
        let sample_id = sample_set.sample_ids().into_iter().next().unwrap();
        assert_eq!(sample_set.objectives().get(sample_id), Some(&objective));
    }

    #[test]
    fn conversions_preserve_false_optimality_transport() {
        let mut instance = maximizing_binary_instance();
        let original_objective = instance.objective().clone();
        instance.output_objective = Some(OutputObjective::new(
            Sense::Maximize,
            original_objective.clone(),
            false,
        ));

        assert!(instance.convert_active_objective(Sense::Minimize));
        let output = instance.output_objective().unwrap();
        assert_eq!(output.sense(), Sense::Maximize);
        assert_eq!(output.function(), &original_objective);
        assert!(!output.preserves_optimality());
        assert_evaluation(&instance, Sense::Maximize, 1.0);

        // The false flag is independent output semantics, so the sidecar must
        // remain present even when the active and output pairs become identical.
        assert!(instance.as_minimization_problem());
        let output = instance.output_objective().unwrap();
        assert_eq!(output.sense(), Sense::Minimize);
        assert_eq!(output.function(), instance.objective());
        assert!(!output.preserves_optimality());
        assert_evaluation(&instance, Sense::Minimize, -1.0);

        assert!(instance.as_maximization_problem());
        let output = instance.output_objective().unwrap();
        assert_eq!(output.sense(), Sense::Maximize);
        assert_eq!(output.function(), instance.objective());
        assert!(!output.preserves_optimality());
        assert_evaluation(&instance, Sense::Maximize, 1.0);
    }

    #[test]
    fn with_parameters_specializes_parameterized_output_objective() {
        let output_function = Function::from((linear!(1) + linear!(100)).unwrap());
        let mut parametric = ParametricInstance::new(
            Sense::Minimize,
            output_function.clone().neg(),
            BTreeMap::from([(VariableID::from(1), DecisionVariable::continuous())]),
            ParameterTable::from_ids([VariableID::from(100)].into_iter().collect()),
            BTreeMap::new(),
        )
        .unwrap();
        parametric.output_objective =
            Some(OutputObjective::new(Sense::Maximize, output_function, true));

        let materialized = parametric
            .with_parameters(crate::v1::Parameters {
                entries: HashMap::from([(100, 2.0)]),
            })
            .unwrap();

        let output = materialized.output_objective().unwrap();
        assert_eq!(
            output.function(),
            &Function::from((linear!(1) + crate::coeff!(2.0)).unwrap())
        );
        assert_eq!(
            output.function().required_ids(),
            VariableIDSet::from([VariableID::from(1)])
        );
        assert!(output.preserves_optimality());
        let solution = materialized
            .evaluate(&State::from(HashMap::from([(1, 3.0)])), ATol::default())
            .unwrap();
        assert_eq!(*solution.sense(), Some(Sense::Maximize));
        assert_eq!(*solution.objective(), 5.0);
    }

    #[test]
    fn with_parameters_removes_redundant_output_objective() {
        let mut parametric = ParametricInstance::new(
            Sense::Minimize,
            Function::from(linear!(1)),
            BTreeMap::from([(VariableID::from(1), DecisionVariable::continuous())]),
            ParameterTable::from_ids([VariableID::from(100)].into_iter().collect()),
            BTreeMap::new(),
        )
        .unwrap();
        parametric.output_objective = Some(OutputObjective::new(
            Sense::Minimize,
            Function::from((linear!(1) + linear!(100)).unwrap()),
            true,
        ));

        let materialized = parametric
            .with_parameters(crate::v1::Parameters {
                entries: HashMap::from([(100, 0.0)]),
            })
            .unwrap();

        assert!(materialized.output_objective().is_none());
        assert!(materialized.to_v1_bytes().is_ok());
    }
}

#[cfg(test)]
mod with_parameters_tests {
    use super::*;
    use crate::{coeff, linear, Equality, Function};
    use maplit::btreemap;

    fn parameters(ids: impl IntoIterator<Item = VariableID>) -> ParameterTable {
        ParameterTable::from_ids(ids.into_iter().collect())
    }

    /// Parameter substitution must apply to the right-hand-side of
    /// `decision_variable_dependency` entries. The RHS is a `Function`
    /// over defined decision-variable or parameter IDs, so a parameter
    /// reference there would dangle in the resulting `Instance` without
    /// explicit substitution.
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
                x => DecisionVariable::binary(),
                dep => DecisionVariable::binary(),
            })
            .parameters(parameters([p]))
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
                x => DecisionVariable::binary(),
            })
            .parameters(parameters([p]))
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

        let params = crate::v1::Parameters {
            entries: std::collections::HashMap::from([(100, 1.0)]),
        };
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
                y => DecisionVariable::binary(),
                x => DecisionVariable::binary(),
            })
            .parameters(parameters([p]))
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

        let params = crate::v1::Parameters {
            entries: std::collections::HashMap::from([(100, 1.0)]),
        };
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
            Function::from(((linear!(2) + linear!(100)).unwrap() + coeff!(-1.0)).unwrap()),
        );

        let parametric = ParametricInstance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(btreemap! {
                y => DecisionVariable::binary(),
                x => DecisionVariable::binary(),
            })
            .parameters(parameters([p]))
            .constraints(BTreeMap::new())
            .indicator_constraints(btreemap! {
                crate::IndicatorConstraintID::from(0) => indicator,
            })
            .build()
            .unwrap();

        let params = crate::v1::Parameters {
            entries: std::collections::HashMap::from([(100, 1.0)]),
        };
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
