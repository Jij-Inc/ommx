use super::*;
use crate::{message_io, parse::RawParseError, v1, v2, ConstraintType, Message, Parse};
use anyhow::{Context, Result};

pub(super) fn ensure_root_function_depths(
    action: &'static str,
    root: &'static str,
    objective: &Function,
    constraints: &ConstraintCollection<Constraint>,
    indicator_constraints: &ConstraintCollection<IndicatorConstraint>,
    decision_variable_dependency: &AcyclicAssignments,
    named_functions: &NamedFunctionTable<NamedFunction>,
) -> Result<()> {
    objective
        .ensure_wire_expression_depth()
        .with_context(|| format!("{action} {root} objective"))?;

    for (id, constraint) in constraints.active() {
        constraint
            .function()
            .ensure_wire_expression_depth()
            .with_context(|| format!("{action} active regular constraint {id}"))?;
    }
    for (id, (constraint, _)) in constraints.removed() {
        constraint
            .function()
            .ensure_wire_expression_depth()
            .with_context(|| format!("{action} removed regular constraint {id}"))?;
    }
    for (id, constraint) in indicator_constraints.active() {
        constraint
            .function()
            .ensure_wire_expression_depth()
            .with_context(|| format!("{action} active indicator constraint {id}"))?;
    }
    for (id, (constraint, _)) in indicator_constraints.removed() {
        constraint
            .function()
            .ensure_wire_expression_depth()
            .with_context(|| format!("{action} removed indicator constraint {id}"))?;
    }
    for (id, function) in decision_variable_dependency.iter() {
        function
            .ensure_wire_expression_depth()
            .with_context(|| format!("{action} decision variable dependency {id}"))?;
    }
    for (id, named_function) in named_functions {
        named_function
            .function
            .ensure_wire_expression_depth()
            .with_context(|| format!("{action} named function {id}"))?;
    }
    Ok(())
}

impl Instance {
    /// Serialize this instance as `ommx.v1.Instance` protobuf bytes.
    ///
    /// Returns an error if an embedded function expression exceeds the
    /// protobuf-safe serialization depth or cannot be represented in v1.
    pub fn to_v1_bytes(&self) -> Result<Vec<u8>> {
        // Validate before cloning: cloning a recursively nested Function walks
        // the expression tree recursively and must not precede the wire-depth
        // guard that makes this API fallible.
        ensure_root_function_depths(
            "Cannot serialize",
            "Instance",
            &self.objective,
            &self.constraint_collection,
            &self.indicator_constraint_collection,
            &self.decision_variable_dependency,
            &self.named_functions,
        )?;
        let v1_instance = v1::Instance::try_from(self.clone())?;
        Ok(v1_instance.encode_to_vec())
    }

    /// Serialize this instance as `ommx.v2.Instance` protobuf bytes.
    ///
    /// Returns an error if an embedded function expression exceeds the
    /// protobuf-safe serialization depth.
    pub fn to_v2_bytes(&self) -> Result<Vec<u8>> {
        ensure_root_function_depths(
            "Cannot serialize",
            "Instance",
            &self.objective,
            &self.constraint_collection,
            &self.indicator_constraint_collection,
            &self.decision_variable_dependency,
            &self.named_functions,
        )?;
        let v2_instance = v2::Instance::from(self.clone());
        Ok(v2_instance.encode_to_vec())
    }

    /// Deserialize an `ommx.v1.Instance` from protobuf bytes.
    ///
    /// Malformed or semantically invalid messages, including embedded
    /// functions beyond the managed wire-depth limit, preserve
    /// [`crate::ParseError`] in the returned error chain.
    pub fn from_v1_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = message_io::decode::<v1::Instance>(bytes, "ommx.v1.Instance")?;
        let instance = Parse::parse(inner, &())?;
        ensure_root_function_depths(
            "Cannot deserialize",
            "Instance",
            &instance.objective,
            &instance.constraint_collection,
            &instance.indicator_constraint_collection,
            &instance.decision_variable_dependency,
            &instance.named_functions,
        )
        .map_err(|error| {
            RawParseError::InvalidFunction(format!("{error:#}"))
                .context("ommx.v1.Instance", "bytes")
        })?;
        Ok(instance)
    }

    /// Deserialize an `ommx.v2.Instance` from protobuf bytes.
    ///
    /// Malformed or semantically invalid messages, including embedded
    /// functions beyond the managed wire-depth limit, preserve
    /// [`crate::ParseError`] in the returned error chain.
    pub fn from_v2_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = message_io::decode::<v2::Instance>(bytes, "ommx.v2.Instance")?;
        let instance = Parse::parse(inner, &())?;
        ensure_root_function_depths(
            "Cannot deserialize",
            "Instance",
            &instance.objective,
            &instance.constraint_collection,
            &instance.indicator_constraint_collection,
            &instance.decision_variable_dependency,
            &instance.named_functions,
        )
        .map_err(|error| {
            RawParseError::InvalidFunction(format!("{error:#}"))
                .context("ommx.v2.Instance", "bytes")
        })?;
        Ok(instance)
    }
}

impl ParametricInstance {
    /// Serialize this parametric instance as `ommx.v1.ParametricInstance`
    /// protobuf bytes.
    ///
    /// Returns an error if an embedded function expression exceeds the
    /// protobuf-safe serialization depth or cannot be represented in v1.
    pub fn to_v1_bytes(&self) -> Result<Vec<u8>> {
        // Keep the depth guard ahead of the recursive clone for the same reason
        // as Instance::to_v1_bytes.
        ensure_root_function_depths(
            "Cannot serialize",
            "ParametricInstance",
            &self.objective,
            &self.constraint_collection,
            &self.indicator_constraint_collection,
            &self.decision_variable_dependency,
            &self.named_functions,
        )?;
        let v1_instance = v1::ParametricInstance::try_from(self.clone())?;
        Ok(v1_instance.encode_to_vec())
    }

    /// Serialize this parametric instance as `ommx.v2.ParametricInstance`
    /// protobuf bytes.
    ///
    /// Returns an error if an embedded function expression exceeds the
    /// protobuf-safe serialization depth.
    pub fn to_v2_bytes(&self) -> Result<Vec<u8>> {
        ensure_root_function_depths(
            "Cannot serialize",
            "ParametricInstance",
            &self.objective,
            &self.constraint_collection,
            &self.indicator_constraint_collection,
            &self.decision_variable_dependency,
            &self.named_functions,
        )?;
        let v2_instance = v2::ParametricInstance::from(self.clone());
        Ok(v2_instance.encode_to_vec())
    }

    /// Deserialize an `ommx.v1.ParametricInstance` from protobuf bytes.
    ///
    /// Malformed or semantically invalid messages, including embedded
    /// functions beyond the managed wire-depth limit, preserve
    /// [`crate::ParseError`] in the returned error chain.
    pub fn from_v1_bytes(bytes: &[u8]) -> Result<Self> {
        let inner =
            message_io::decode::<v1::ParametricInstance>(bytes, "ommx.v1.ParametricInstance")?;
        let instance = Parse::parse(inner, &())?;
        ensure_root_function_depths(
            "Cannot deserialize",
            "ParametricInstance",
            &instance.objective,
            &instance.constraint_collection,
            &instance.indicator_constraint_collection,
            &instance.decision_variable_dependency,
            &instance.named_functions,
        )
        .map_err(|error| {
            RawParseError::InvalidFunction(format!("{error:#}"))
                .context("ommx.v1.ParametricInstance", "bytes")
        })?;
        Ok(instance)
    }

    /// Deserialize an `ommx.v2.ParametricInstance` from protobuf bytes.
    ///
    /// Malformed or semantically invalid messages, including embedded
    /// functions beyond the managed wire-depth limit, preserve
    /// [`crate::ParseError`] in the returned error chain.
    pub fn from_v2_bytes(bytes: &[u8]) -> Result<Self> {
        let inner =
            message_io::decode::<v2::ParametricInstance>(bytes, "ommx.v2.ParametricInstance")?;
        let instance = Parse::parse(inner, &())?;
        ensure_root_function_depths(
            "Cannot deserialize",
            "ParametricInstance",
            &instance.objective,
            &instance.constraint_collection,
            &instance.indicator_constraint_collection,
            &instance.decision_variable_dependency,
            &instance.named_functions,
        )
        .map_err(|error| {
            RawParseError::InvalidFunction(format!("{error:#}"))
                .context("ommx.v2.ParametricInstance", "bytes")
        })?;
        Ok(instance)
    }
}

impl From<Instance> for v2::Instance {
    fn from(value: Instance) -> Self {
        let required_features = crate::v2_io::required_features(
            created_collection_has_payload(&value.indicator_constraint_collection),
            created_collection_has_payload(&value.one_hot_constraint_collection),
            created_collection_has_payload(&value.sos1_constraint_collection),
        );

        let Instance {
            sense,
            objective,
            decision_variables,
            constraint_collection,
            indicator_constraint_collection,
            one_hot_constraint_collection,
            sos1_constraint_collection,
            decision_variable_dependency,
            named_functions,
            parameters,
            description,
            annotations,
        } = value;

        Self {
            required_features,
            description,
            decision_variables: Some(decision_variables.into()),
            objective: Some(objective.into()),
            regular_constraints: Some(constraint_collection.into()),
            sense: sense.into(),
            parameters,
            indicator_constraints: Some(indicator_constraint_collection.into()),
            one_hot_constraints: Some(one_hot_constraint_collection.into()),
            sos1_constraints: Some(sos1_constraint_collection.into()),
            decision_variable_dependency: decision_variable_dependency_to_v2_map(
                decision_variable_dependency,
            ),
            named_functions: Some(named_functions.into()),
            annotations: crate::v2_io::extension_annotations_to_v2_map(annotations),
        }
    }
}

impl From<ParametricInstance> for v2::ParametricInstance {
    fn from(value: ParametricInstance) -> Self {
        let required_features = crate::v2_io::required_features(
            created_collection_has_payload(&value.indicator_constraint_collection),
            created_collection_has_payload(&value.one_hot_constraint_collection),
            created_collection_has_payload(&value.sos1_constraint_collection),
        );

        let ParametricInstance {
            sense,
            objective,
            decision_variables,
            parameters,
            constraint_collection,
            indicator_constraint_collection,
            one_hot_constraint_collection,
            sos1_constraint_collection,
            decision_variable_dependency,
            named_functions,
            description,
            annotations,
        } = value;

        Self {
            required_features,
            description,
            decision_variables: Some(decision_variables.into()),
            parameters: Some(parameters.into()),
            objective: Some(objective.into()),
            regular_constraints: Some(constraint_collection.into()),
            sense: sense.into(),
            indicator_constraints: Some(indicator_constraint_collection.into()),
            one_hot_constraints: Some(one_hot_constraint_collection.into()),
            sos1_constraints: Some(sos1_constraint_collection.into()),
            decision_variable_dependency: decision_variable_dependency_to_v2_map(
                decision_variable_dependency,
            ),
            named_functions: Some(named_functions.into()),
            annotations: crate::v2_io::extension_annotations_to_v2_map(annotations),
        }
    }
}

fn created_collection_has_payload<T: ConstraintType>(collection: &ConstraintCollection<T>) -> bool {
    !collection.active().is_empty() || !collection.removed().is_empty()
}

fn decision_variable_dependency_to_v2_map(
    dependency: AcyclicAssignments,
) -> std::collections::BTreeMap<u64, v1::Function> {
    dependency
        .into_iter()
        .map(|(id, function)| (id.into_inner(), function.into()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        linear, v2, ATol, DecisionVariable, Equality, Evaluate, Function, IndicatorConstraint,
        IndicatorConstraintID, OneHotConstraint, OneHotConstraintID, ParameterLabelStore,
        ParameterTable, Sampled, Sos1Constraint, Sos1ConstraintID, VariableID,
    };
    use std::collections::{BTreeMap, BTreeSet, HashMap};

    fn function_at_depth(depth: usize) -> Function {
        (0..depth).fold(Function::from(linear!(1)), |function, level| {
            if level % 2 == 0 {
                function.abs()
            } else {
                function.signum()
            }
        })
    }

    fn instance_with_objective_at_depth(depth: usize) -> Instance {
        Instance::builder()
            .sense(Sense::Minimize)
            .objective(function_at_depth(depth))
            .decision_variables(BTreeMap::from([(
                VariableID::from(1),
                DecisionVariable::continuous(),
            )]))
            .constraints(BTreeMap::new())
            .build()
            .unwrap()
    }

    fn instance_with_named_function_at_depth(depth: usize) -> Instance {
        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([(
                VariableID::from(1),
                DecisionVariable::continuous(),
            )]))
            .constraints(BTreeMap::new())
            .named_functions(BTreeMap::from([(
                NamedFunctionID::from(1),
                NamedFunction {
                    function: function_at_depth(depth),
                },
            )]))
            .build()
            .unwrap()
    }

    #[test]
    fn instance_protobuf_bytes_enforce_embedded_expression_depth_boundary() {
        let boundary = instance_with_objective_at_depth(Function::MAX_WIRE_EXPRESSION_DEPTH);
        let v1_bytes = boundary.to_v1_bytes().unwrap();
        assert_eq!(Instance::from_v1_bytes(&v1_bytes).unwrap(), boundary);
        let v2_bytes = boundary.to_v2_bytes().unwrap();
        assert_eq!(Instance::from_v2_bytes(&v2_bytes).unwrap(), boundary);

        // Named functions sit below a table and a protobuf map entry, making
        // this one of the deepest supported Function containers in v2.
        let deepest_container =
            instance_with_named_function_at_depth(Function::MAX_WIRE_EXPRESSION_DEPTH);
        let v2_bytes = deepest_container.to_v2_bytes().unwrap();
        assert_eq!(
            Instance::from_v2_bytes(&v2_bytes).unwrap(),
            deepest_container,
        );

        let too_deep = instance_with_objective_at_depth(Function::MAX_WIRE_EXPRESSION_DEPTH + 1);
        let direct_v1_error = v1::Instance::try_from(too_deep.clone()).unwrap_err();
        assert!(
            format!("{direct_v1_error:#}").contains("protobuf serialization limit of 32"),
            "unexpected error: {direct_v1_error:#}",
        );
        for error in [
            too_deep.to_v1_bytes().unwrap_err(),
            too_deep.to_v2_bytes().unwrap_err(),
        ] {
            let message = format!("{error:#}");
            assert!(
                message.contains("protobuf serialization limit of 32"),
                "unexpected error: {error:#}",
            );
        }

        let parametric: ParametricInstance = too_deep.into();
        let direct_v1_error = v1::ParametricInstance::try_from(parametric.clone()).unwrap_err();
        assert!(
            format!("{direct_v1_error:#}").contains("protobuf serialization limit of 32"),
            "unexpected error: {direct_v1_error:#}",
        );
        let error = parametric.to_v1_bytes().unwrap_err();
        assert!(
            format!("{error:#}").contains("protobuf serialization limit of 32"),
            "unexpected error: {error:#}",
        );
        assert!(parametric.to_v2_bytes().is_err());

        // Raw protobuf construction remains available to low-level callers,
        // but managed decoders must reject roots that cannot subsequently be
        // encoded by the corresponding managed API.
        let raw_function = function_at_depth(Function::MAX_WIRE_EXPRESSION_DEPTH + 1);

        let mut raw_v1 = v1::Instance::try_from(instance_with_objective_at_depth(0)).unwrap();
        raw_v1.objective = Some(raw_function.clone().into());
        let error = Instance::from_v1_bytes(&raw_v1.encode_to_vec()).unwrap_err();
        assert!(error.downcast_ref::<crate::ParseError>().is_some());
        assert!(
            format!("{error:#}").contains("protobuf serialization limit of 32"),
            "unexpected error: {error:#}",
        );

        let mut raw_v2 = v2::Instance::from(instance_with_objective_at_depth(0));
        raw_v2.objective = Some(raw_function.clone().into());
        let error = Instance::from_v2_bytes(&raw_v2.encode_to_vec()).unwrap_err();
        assert!(error.downcast_ref::<crate::ParseError>().is_some());
        assert!(
            format!("{error:#}").contains("protobuf serialization limit of 32"),
            "unexpected error: {error:#}",
        );

        let base_parametric: ParametricInstance = instance_with_objective_at_depth(0).into();
        let mut raw_v1 = v1::ParametricInstance::try_from(base_parametric.clone()).unwrap();
        raw_v1.objective = Some(raw_function.clone().into());
        let error = ParametricInstance::from_v1_bytes(&raw_v1.encode_to_vec()).unwrap_err();
        assert!(error.downcast_ref::<crate::ParseError>().is_some());
        assert!(
            format!("{error:#}").contains("protobuf serialization limit of 32"),
            "unexpected error: {error:#}",
        );

        let mut raw_v2 = v2::ParametricInstance::from(base_parametric);
        raw_v2.objective = Some(raw_function.into());
        let error = ParametricInstance::from_v2_bytes(&raw_v2.encode_to_vec()).unwrap_err();
        assert!(error.downcast_ref::<crate::ParseError>().is_some());
        assert!(
            format!("{error:#}").contains("protobuf serialization limit of 32"),
            "unexpected error: {error:#}",
        );
    }

    fn instance_with_special_constraints() -> Instance {
        let variable_1 = VariableID::from(1);
        let variable_2 = VariableID::from(2);

        let indicator_id = IndicatorConstraintID::from(10);
        let one_hot_id = OneHotConstraintID::from(20);
        let sos1_id = Sos1ConstraintID::from(30);

        let mut indicator_context = ConstraintContextStore::default();
        indicator_context.set_name(indicator_id, "indicator");
        let mut one_hot_context = ConstraintContextStore::default();
        one_hot_context.set_name(one_hot_id, "one_hot");
        let mut sos1_context = ConstraintContextStore::default();
        sos1_context.set_name(sos1_id, "sos1");

        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([
                (variable_1, DecisionVariable::binary()),
                (variable_2, DecisionVariable::binary()),
            ]))
            .constraints(BTreeMap::new())
            .indicator_constraints(BTreeMap::from([(
                indicator_id,
                IndicatorConstraint::new(
                    variable_1,
                    Equality::LessThanOrEqualToZero,
                    Function::Zero,
                ),
            )]))
            .indicator_constraint_context(indicator_context)
            .one_hot_constraints(BTreeMap::from([(
                one_hot_id,
                OneHotConstraint::new(BTreeSet::from([variable_1, variable_2])).unwrap(),
            )]))
            .one_hot_constraint_context(one_hot_context)
            .sos1_constraints(BTreeMap::from([(
                sos1_id,
                Sos1Constraint::new(BTreeSet::from([variable_1, variable_2])).unwrap(),
            )]))
            .sos1_constraint_context(sos1_context)
            .build()
            .unwrap()
    }

    fn expected_special_features() -> Vec<i32> {
        vec![
            v2::Feature::ConstraintIndicator as i32,
            v2::Feature::ConstraintOneHot as i32,
            v2::Feature::ConstraintSos1 as i32,
        ]
    }

    fn assert_btree_map<K: Ord, V>(_: &BTreeMap<K, V>) {}

    #[test]
    fn v1_instance_serialization_rejects_special_constraints() {
        let err = instance_with_special_constraints()
            .to_v1_bytes()
            .unwrap_err();

        assert!(
            err.to_string().contains("to_v2_bytes"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn v1_parametric_instance_serialization_rejects_special_constraints() {
        let instance: ParametricInstance = instance_with_special_constraints().into();
        let err = instance.to_v1_bytes().unwrap_err();

        assert!(
            err.to_string().contains("to_v2_bytes"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn v2_instance_serializes_special_constraint_collections() {
        let proto = v2::Instance::from(instance_with_special_constraints());

        assert_eq!(proto.required_features, expected_special_features());
        let indicator_constraints = proto.indicator_constraints.unwrap();
        assert!(indicator_constraints.active.contains_key(&10));
        assert_eq!(
            indicator_constraints
                .contexts
                .get(&10)
                .and_then(|context| context.label.as_ref())
                .and_then(|label| label.name.as_deref()),
            Some("indicator")
        );

        let one_hot_constraints = proto.one_hot_constraints.unwrap();
        assert_eq!(
            one_hot_constraints.active.get(&20).unwrap().variables,
            vec![1, 2]
        );

        let sos1_constraints = proto.sos1_constraints.unwrap();
        assert_eq!(
            sos1_constraints.active.get(&30).unwrap().variables,
            vec![1, 2]
        );
    }

    #[test]
    fn v2_instance_deserializes_special_constraint_collections() {
        let instance = instance_with_special_constraints();
        let restored = Instance::from_v2_bytes(&instance.to_v2_bytes().unwrap()).unwrap();

        assert_eq!(restored, instance);
        assert_eq!(
            restored
                .indicator_constraint_context()
                .name(IndicatorConstraintID::from(10)),
            Some("indicator")
        );
        assert_eq!(
            restored
                .one_hot_constraint_context()
                .name(OneHotConstraintID::from(20)),
            Some("one_hot")
        );
        assert_eq!(
            restored
                .sos1_constraint_context()
                .name(Sos1ConstraintID::from(30)),
            Some("sos1")
        );
    }

    #[test]
    fn v2_instance_deserialization_rejects_missing_required_feature() {
        let mut proto = v2::Instance::from(instance_with_special_constraints());
        proto.required_features = vec![
            v2::Feature::ConstraintOneHot as i32,
            v2::Feature::ConstraintSos1 as i32,
        ];

        let err = Instance::try_from(proto).unwrap_err();

        assert!(
            err.to_string().contains("required_features")
                && err.to_string().contains("ConstraintIndicator"),
            "unexpected error: {err}",
        );
    }

    #[test]
    fn v2_solution_serializes_evaluated_special_constraint_collections() {
        let instance = instance_with_special_constraints();
        let solution = instance
            .evaluate(
                &v1::State {
                    entries: HashMap::from([(1, 1.0), (2, 0.0)]),
                },
                ATol::default(),
            )
            .unwrap();

        let proto = v2::Solution::from(solution);

        assert_eq!(proto.required_features, expected_special_features());
        assert!(proto
            .evaluated_indicator_constraints
            .unwrap()
            .entries
            .contains_key(&10));
        assert!(proto
            .evaluated_one_hot_constraints
            .unwrap()
            .entries
            .contains_key(&20));
        assert!(proto
            .evaluated_sos1_constraints
            .unwrap()
            .entries
            .contains_key(&30));
    }

    #[test]
    fn v2_solution_deserializes_evaluated_special_constraint_collections() {
        let instance = instance_with_special_constraints();
        let solution = instance
            .evaluate(
                &v1::State {
                    entries: HashMap::from([(1, 1.0), (2, 0.0)]),
                },
                ATol::default(),
            )
            .unwrap();

        let restored = crate::Solution::from_v2_bytes(&solution.to_v2_bytes()).unwrap();

        assert_eq!(restored, solution);
        assert_eq!(
            restored
                .evaluated_indicator_constraints()
                .context()
                .name(IndicatorConstraintID::from(10)),
            Some("indicator")
        );
        assert!(restored
            .evaluated_one_hot_constraints()
            .contains_key(&OneHotConstraintID::from(20)));
        assert!(restored
            .evaluated_sos1_constraints()
            .contains_key(&Sos1ConstraintID::from(30)));
    }

    #[test]
    fn v2_solution_deserialization_rejects_unknown_structural_special_variable() {
        let instance = instance_with_special_constraints();
        let solution = instance
            .evaluate(
                &v1::State {
                    entries: HashMap::from([(1, 1.0), (2, 0.0)]),
                },
                ATol::default(),
            )
            .unwrap();
        let mut proto = v2::Solution::from(solution);
        let one_hot = proto
            .evaluated_one_hot_constraints
            .as_mut()
            .unwrap()
            .entries
            .get_mut(&20)
            .unwrap();
        one_hot.variables = vec![999];
        one_hot.active_variable = Some(999);
        one_hot.used_decision_variable_ids.clear();

        let err = crate::Solution::try_from(proto).unwrap_err();

        assert!(
            err.to_string().contains("One-hot variable")
                && err.to_string().contains("decision_variables"),
            "unexpected error: {err}",
        );
    }

    #[test]
    fn v2_sample_set_serializes_sampled_special_constraint_collections() {
        let instance = instance_with_special_constraints();
        let samples = Sampled::from(v1::State {
            entries: HashMap::from([(1, 1.0), (2, 0.0)]),
        });
        let sample_set = instance
            .evaluate_samples(&samples, ATol::default())
            .unwrap();

        let proto = v2::SampleSet::from(sample_set);

        assert_eq!(proto.required_features, expected_special_features());
        assert!(proto
            .sampled_indicator_constraints
            .unwrap()
            .entries
            .contains_key(&10));
        assert!(proto
            .sampled_one_hot_constraints
            .unwrap()
            .entries
            .contains_key(&20));
        assert!(proto
            .sampled_sos1_constraints
            .unwrap()
            .entries
            .contains_key(&30));
    }

    #[test]
    fn v2_sample_set_deserializes_sampled_special_constraint_collections() {
        let instance = instance_with_special_constraints();
        let samples = Sampled::from(v1::State {
            entries: HashMap::from([(1, 1.0), (2, 0.0)]),
        });
        let sample_set = instance
            .evaluate_samples(&samples, ATol::default())
            .unwrap();

        let restored = crate::SampleSet::from_v2_bytes(&sample_set.to_v2_bytes()).unwrap();

        assert_eq!(restored.feasible(), sample_set.feasible());
        assert_eq!(restored.feasible_relaxed(), sample_set.feasible_relaxed());
        assert_eq!(restored.indicator_constraints().len(), 1);
        assert_eq!(restored.one_hot_constraints().len(), 1);
        assert_eq!(restored.sos1_constraints().len(), 1);
        assert_eq!(
            restored
                .indicator_constraints()
                .context()
                .name(IndicatorConstraintID::from(10)),
            Some("indicator")
        );
    }

    #[test]
    fn v2_sample_set_deserialization_rejects_missing_feasible_sample_id() {
        let instance = instance_with_special_constraints();
        let samples = Sampled::from(v1::State {
            entries: HashMap::from([(1, 1.0), (2, 0.0)]),
        });
        let sample_set = instance
            .evaluate_samples(&samples, ATol::default())
            .unwrap();
        let mut proto = v2::SampleSet::from(sample_set);
        let sample_id = *proto.feasible.keys().next().unwrap();
        proto.feasible.remove(&sample_id);

        let err = crate::SampleSet::try_from(proto).unwrap_err();

        assert!(
            err.to_string().contains("feasible")
                && err.to_string().contains("Inconsistent sample IDs"),
            "unexpected error: {err}",
        );
    }

    #[test]
    fn to_v2_bytes_encodes_v2_instance() {
        let bytes = instance_with_special_constraints().to_v2_bytes().unwrap();
        let proto = v2::Instance::decode(bytes.as_slice()).unwrap();

        assert_eq!(proto.required_features, expected_special_features());
    }

    #[test]
    fn v2_generated_maps_are_ordered_for_deterministic_encoding() {
        let proto = v2::Instance::from(instance_with_special_constraints());

        assert_btree_map(&proto.annotations);
        let decision_variables = proto.decision_variables.as_ref().unwrap();
        assert_btree_map(&decision_variables.entries);
        assert_btree_map(&decision_variables.labels);
        let indicator_constraints = proto.indicator_constraints.as_ref().unwrap();
        assert_btree_map(&indicator_constraints.active);
        assert_btree_map(&indicator_constraints.contexts);
    }

    #[test]
    fn v2_parametric_instance_serializes_parameter_table() {
        let decision_variable_id = VariableID::from(1);
        let parameter_id = VariableID::from(100);
        let mut parameter_labels = ParameterLabelStore::default();
        parameter_labels.set_name(parameter_id, "p");

        let instance = ParametricInstance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([(
                decision_variable_id,
                DecisionVariable::binary(),
            )]))
            .parameters(
                ParameterTable::new(BTreeSet::from([parameter_id]), parameter_labels).unwrap(),
            )
            .constraints(BTreeMap::new())
            .build()
            .unwrap();

        let proto = v2::ParametricInstance::from(instance);
        let parameters = proto.parameters.unwrap();

        assert_eq!(parameters.ids, vec![100]);
        assert_eq!(
            parameters
                .labels
                .get(&100)
                .and_then(|label| label.name.as_deref()),
            Some("p")
        );
    }

    #[test]
    fn v2_parametric_instance_deserializes_parameter_table() {
        let decision_variable_id = VariableID::from(1);
        let parameter_id = VariableID::from(100);
        let mut parameter_labels = ParameterLabelStore::default();
        parameter_labels.set_name(parameter_id, "p");

        let instance = ParametricInstance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([(
                decision_variable_id,
                DecisionVariable::binary(),
            )]))
            .parameters(
                ParameterTable::new(BTreeSet::from([parameter_id]), parameter_labels).unwrap(),
            )
            .constraints(BTreeMap::new())
            .build()
            .unwrap();

        let restored = ParametricInstance::from_v2_bytes(&instance.to_v2_bytes().unwrap()).unwrap();

        assert_eq!(restored, instance);
        assert_eq!(restored.parameters().labels().name(parameter_id), Some("p"));
    }
}
