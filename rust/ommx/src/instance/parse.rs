use super::*;
use crate::{
    constraint::{ConstraintContext, RemovedReason},
    constraint_hints::ConstraintHints,
    constraint_type::ConstraintCollection,
    parse::{as_variable_id, Parse, ParseError, RawParseError},
    v1::{self},
    Constraint, ConstraintID, VariableID,
};

type ConvertedConstraintHints = (
    BTreeMap<crate::OneHotConstraintID, crate::OneHotConstraint>,
    BTreeMap<crate::Sos1ConstraintID, crate::Sos1Constraint>,
    std::collections::BTreeSet<crate::ConstraintID>,
);

/// Convert parsed `ConstraintHints` to first-class OneHot/SOS1 constraint collections,
/// and return the set of regular constraint IDs that should be removed from the
/// constraint collection (they are subsumed by the new first-class constraints).
fn convert_hints_to_collections(
    hints: &ConstraintHints,
) -> crate::Result<ConvertedConstraintHints> {
    let mut one_hot_active = BTreeMap::new();
    let mut absorbed_constraint_ids = std::collections::BTreeSet::new();
    for hint in &hints.one_hot_constraints {
        let id = crate::OneHotConstraintID::from(*hint.id);
        one_hot_active.insert(id, crate::OneHotConstraint::new(hint.variables.clone())?);
        absorbed_constraint_ids.insert(hint.id);
    }
    let mut sos1_active = BTreeMap::new();
    for hint in &hints.sos1_constraints {
        let id = crate::Sos1ConstraintID::from(*hint.binary_constraint_id);
        sos1_active.insert(id, crate::Sos1Constraint::new(hint.variables.clone())?);
        absorbed_constraint_ids.insert(hint.binary_constraint_id);
        absorbed_constraint_ids.extend(&hint.big_m_constraint_ids);
    }
    Ok((one_hot_active, sos1_active, absorbed_constraint_ids))
}

fn drain_absorbed_hint_context(
    hints: &ConstraintHints,
    absorbed_ids: &std::collections::BTreeSet<ConstraintID>,
    regular_context: &mut crate::ConstraintContextStore<ConstraintID>,
) -> (
    crate::ConstraintContextStore<crate::OneHotConstraintID>,
    crate::ConstraintContextStore<crate::Sos1ConstraintID>,
) {
    let mut one_hot_context = crate::ConstraintContextStore::default();
    let mut sos1_context = crate::ConstraintContextStore::default();

    for hint in &hints.one_hot_constraints {
        one_hot_context.insert(
            crate::OneHotConstraintID::from(*hint.id),
            regular_context.remove(hint.id),
        );
    }
    for hint in &hints.sos1_constraints {
        sos1_context.insert(
            crate::Sos1ConstraintID::from(*hint.binary_constraint_id),
            regular_context.remove(hint.binary_constraint_id),
        );
    }

    for id in absorbed_ids {
        regular_context.remove(*id);
    }

    (one_hot_context, sos1_context)
}

fn validate_fixed_decision_variable_partition(
    message: &'static str,
    objective: &Function,
    constraints: &BTreeMap<ConstraintID, Constraint>,
    one_hot_constraints: &BTreeMap<crate::OneHotConstraintID, crate::OneHotConstraint>,
    sos1_constraints: &BTreeMap<crate::Sos1ConstraintID, crate::Sos1Constraint>,
    decision_variable_dependency: &AcyclicAssignments,
    fixed_decision_variable_values: &BTreeMap<VariableID, f64>,
) -> Result<(), ParseError> {
    let mut used: VariableIDSet = objective.required_ids();
    for constraint in constraints.values() {
        used.extend(constraint.required_ids());
    }
    for one_hot in one_hot_constraints.values() {
        used.extend(one_hot.required_ids());
    }
    for sos1 in sos1_constraints.values() {
        used.extend(sos1.required_ids());
    }

    let fixed: VariableIDSet = fixed_decision_variable_values.keys().copied().collect();
    let dependent: VariableIDSet = decision_variable_dependency.keys().collect();

    if let Some(id) = used.intersection(&dependent).next() {
        return Err(RawParseError::InvalidInstance(format!(
            "Dependent variable cannot be used in objectives or constraints: {id:?}"
        ))
        .context(message, "decision_variables"));
    }
    if let Some(id) = used.intersection(&fixed).next() {
        return Err(RawParseError::InvalidInstance(format!(
            "Fixed variable {id:?} cannot be used in objectives or constraints"
        ))
        .context(message, "decision_variables"));
    }
    if let Some(id) = fixed.intersection(&dependent).next() {
        return Err(RawParseError::InvalidInstance(format!(
            "Variable {id:?} cannot be both fixed and dependent"
        ))
        .context(message, "decision_variables"));
    }

    Ok(())
}

impl Parse for v1::instance::Sense {
    type Output = Sense;
    type Context = ();
    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        match self {
            v1::instance::Sense::Minimize => Ok(Sense::Minimize),
            v1::instance::Sense::Maximize => Ok(Sense::Maximize),
            v1::instance::Sense::Unspecified => {
                tracing::warn!("Unspecified ommx.v1.instance.Sense found, defaulting to Minimize");
                Ok(Sense::Minimize)
            }
        }
    }
}

impl TryFrom<v1::instance::Sense> for Sense {
    type Error = ParseError;
    fn try_from(value: v1::instance::Sense) -> Result<Self, Self::Error> {
        value.parse(&())
    }
}

impl TryFrom<i32> for Sense {
    type Error = anyhow::Error;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        let v1_sense = v1::instance::Sense::try_from(value).map_err(|_| {
            anyhow::anyhow!("Invalid integer for ommx.v1.instance.Sense: {}", value)
        })?;
        Ok(v1_sense.try_into()?)
    }
}

impl From<Sense> for v1::instance::Sense {
    fn from(value: Sense) -> Self {
        match value {
            Sense::Minimize => v1::instance::Sense::Minimize,
            Sense::Maximize => v1::instance::Sense::Maximize,
        }
    }
}

impl From<Sense> for i32 {
    fn from(value: Sense) -> Self {
        v1::instance::Sense::from(value).into()
    }
}

impl Parse for v1::Instance {
    type Output = Instance;
    type Context = ();
    fn parse(self, _context: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.Instance";
        crate::parse::check_format_version(self.format_version, message)?;
        crate::parse::validate_extension_annotations(&self.annotations, message)?;
        let sense = self.sense().parse_as(&(), message, "sense")?;

        let (decision_variables, variable_labels, fixed_decision_variable_values): (
            BTreeMap<VariableID, DecisionVariable>,
            crate::VariableLabelStore,
            BTreeMap<VariableID, f64>,
        ) = self
            .decision_variables
            .parse_as(&(), message, "decision_variables")?;

        let objective = self
            .objective
            .ok_or(RawParseError::MissingField {
                message,
                field: "objective",
            })?
            .parse_as(&(), message, "objective")?;

        // Validate that all variables used in objective are defined as decision variables
        let decision_variable_ids: VariableIDSet = decision_variables.keys().cloned().collect();
        for id in objective.required_ids() {
            if !decision_variable_ids.contains(&id) {
                return Err(RawParseError::InvalidInstance(format!(
                    "Undefined variable ID is used: {id:?}"
                ))
                .context(message, "objective"));
            }
        }

        let (constraints, mut constraint_context): (
            BTreeMap<ConstraintID, Constraint>,
            crate::ConstraintContextStore<ConstraintID>,
        ) = self.constraints.parse_as(&(), message, "constraints")?;

        // Validate that all variables used in constraints are defined as decision variables
        for constraint in constraints.values() {
            for id in constraint.required_ids() {
                if !decision_variable_ids.contains(&id) {
                    return Err(RawParseError::InvalidInstance(format!(
                        "Undefined variable ID is used: {id:?}"
                    ))
                    .context(message, "constraints"));
                }
            }
        }
        // `parse_as` for `Vec<v1::RemovedConstraint>` returns the active constraints
        // joined with context + removed reason. Strip context into the SoA store
        // so the collection-level `(active, removed)` shape stays uniform.
        let removed_constraints_with_context: BTreeMap<
            ConstraintID,
            (Constraint, ConstraintContext, RemovedReason),
        > = self
            .removed_constraints
            .parse_as(&constraints, message, "removed_constraints")?;
        let mut removed_constraints: BTreeMap<ConstraintID, (Constraint, RemovedReason)> =
            BTreeMap::new();
        for (id, (c, context, reason)) in removed_constraints_with_context {
            constraint_context.insert(id, context);
            removed_constraints.insert(id, (c, reason));
        }

        let (named_functions, named_function_labels): (
            BTreeMap<NamedFunctionID, NamedFunction>,
            crate::named_function::NamedFunctionLabelStore,
        ) = self
            .named_functions
            .parse_as(&(), message, "named_functions")?;

        // Validate that all variables used in named functions are defined as decision variables
        for named_function in named_functions.values() {
            for id in named_function.function.required_ids() {
                if !decision_variable_ids.contains(&id) {
                    return Err(RawParseError::InvalidInstance(format!(
                        "Undefined variable ID is used: {id:?}"
                    ))
                    .context(message, "named_functions"));
                }
            }
        }

        let mut decision_variable_dependency = BTreeMap::default();
        for (id, f) in self.decision_variable_dependency {
            decision_variable_dependency.insert(
                as_variable_id(&decision_variables, id)
                    .map_err(|e| e.context(message, "decision_variable_dependency"))?,
                f.parse_as(&(), message, "decision_variable_dependency")?,
            );
        }
        let decision_variable_dependency = AcyclicAssignments::new(decision_variable_dependency)
            .map_err(|e| RawParseError::from(e).context(message, "decision_variable_dependency"))?;

        let context = (decision_variables, constraints, removed_constraints);
        let constraint_hints = if let Some(hints) = self.constraint_hints {
            hints.parse_as(&context, message, "constraint_hints")?
        } else {
            Default::default()
        };
        let (decision_variables, mut constraints, removed_constraints) = context;

        let (one_hot_active, sos1_active, absorbed_ids) =
            convert_hints_to_collections(&constraint_hints).map_err(|e| {
                RawParseError::InvalidInstance(e.to_string()).context(message, "constraint_hints")
            })?;
        let (one_hot_context, sos1_context) =
            drain_absorbed_hint_context(&constraint_hints, &absorbed_ids, &mut constraint_context);
        // Remove regular constraints that are absorbed by OneHot/SOS1
        for id in &absorbed_ids {
            constraints.remove(id);
        }
        validate_fixed_decision_variable_partition(
            message,
            &objective,
            &constraints,
            &one_hot_active,
            &sos1_active,
            &decision_variable_dependency,
            &fixed_decision_variable_values,
        )?;
        let decision_variables = DecisionVariableTable::with_fixed_values(
            decision_variables,
            variable_labels,
            fixed_decision_variable_values,
            crate::ATol::default(),
        )
        .map_err(|e| {
            RawParseError::InvalidInstance(e.to_string()).context(message, "decision_variables")
        })?;

        Ok(Instance {
            sense,
            objective,
            decision_variables,
            constraint_collection: ConstraintCollection::with_context(
                constraints,
                removed_constraints,
                constraint_context,
            )
            .map_err(|e| {
                RawParseError::InvalidInstance(e.to_string()).context(message, "constraints")
            })?,
            indicator_constraint_collection: Default::default(),
            one_hot_constraint_collection: ConstraintCollection::with_context(
                one_hot_active,
                BTreeMap::new(),
                one_hot_context,
            )
            .map_err(|e| {
                RawParseError::InvalidInstance(e.to_string()).context(message, "constraint_hints")
            })?,
            sos1_constraint_collection: ConstraintCollection::with_context(
                sos1_active,
                BTreeMap::new(),
                sos1_context,
            )
            .map_err(|e| {
                RawParseError::InvalidInstance(e.to_string()).context(message, "constraint_hints")
            })?,
            decision_variable_dependency,
            parameters: self.parameters,
            description: self.description,
            annotations: self.annotations,
            named_functions: crate::NamedFunctionTable::new(named_functions, named_function_labels)
                .map_err(|e| {
                    RawParseError::InvalidInstance(e.to_string())
                        .context(message, "named_functions")
                })?,
        })
    }
}

impl TryFrom<v1::Instance> for Instance {
    type Error = ParseError;
    fn try_from(value: v1::Instance) -> Result<Self, Self::Error> {
        value.parse(&())
    }
}

impl From<Instance> for v1::Instance {
    fn from(value: Instance) -> Self {
        let decision_variables: Vec<v1::DecisionVariable> = (&value.decision_variables).into();
        let (constraints, removed_constraints): (Vec<v1::Constraint>, Vec<v1::RemovedConstraint>) =
            value.constraint_collection.into();
        let named_functions: Vec<v1::NamedFunction> = value.named_functions.into();
        let decision_variable_dependency = value
            .decision_variable_dependency
            .into_iter()
            .map(|(id, dep)| (id.into(), dep.into()))
            .collect();
        // Special constraint types do not have a v1 proto representation yet.
        // Serialization is not supported when these collections are non-empty.
        if !value.indicator_constraint_collection.active().is_empty()
            || !value.indicator_constraint_collection.removed().is_empty()
        {
            unimplemented!("Serialization of IndicatorConstraint to v1 proto is not yet supported");
        }
        if !value.one_hot_constraint_collection.active().is_empty()
            || !value.one_hot_constraint_collection.removed().is_empty()
        {
            unimplemented!("Serialization of OneHotConstraint to v1 proto is not yet supported");
        }
        if !value.sos1_constraint_collection.active().is_empty()
            || !value.sos1_constraint_collection.removed().is_empty()
        {
            unimplemented!("Serialization of Sos1Constraint to v1 proto is not yet supported");
        }
        Self {
            sense: v1::instance::Sense::from(value.sense).into(),
            decision_variables,
            objective: Some(value.objective.into()),
            constraints,
            named_functions,
            removed_constraints,
            decision_variable_dependency,
            parameters: value.parameters,
            description: value.description,
            constraint_hints: None,
            format_version: crate::CURRENT_FORMAT_VERSION,
            annotations: crate::protobuf_extension_annotations(value.annotations),
        }
    }
}

impl Parse for v1::ParametricInstance {
    type Output = ParametricInstance;
    type Context = ();
    fn parse(self, _context: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.ParametricInstance";
        crate::parse::check_format_version(self.format_version, message)?;
        crate::parse::validate_extension_annotations(&self.annotations, message)?;
        let sense = self.sense().parse_as(&(), message, "sense")?;

        let (decision_variables, variable_labels, fixed_decision_variable_values): (
            BTreeMap<VariableID, DecisionVariable>,
            crate::VariableLabelStore,
            BTreeMap<VariableID, f64>,
        ) = self
            .decision_variables
            .parse_as(&(), message, "decision_variables")?;

        let parameters = ParameterTable::from_v1_parameters(self.parameters).map_err(|e| {
            RawParseError::InvalidInstance(e.to_string()).context(message, "parameters")
        })?;

        let decision_variable_ids: VariableIDSet = decision_variables.keys().cloned().collect();
        let parameter_ids: VariableIDSet = parameters.keys().cloned().collect();
        let intersection: VariableIDSet = decision_variable_ids
            .intersection(&parameter_ids)
            .cloned()
            .collect();
        if !intersection.is_empty() {
            let id = *intersection.iter().next().unwrap();
            return Err(RawParseError::InvalidInstance(format!(
                "Duplicated variable ID is found in definition: {id:?}"
            ))
            .context(message, "parameters"));
        }

        let objective = self
            .objective
            .ok_or(RawParseError::MissingField {
                message,
                field: "objective",
            })?
            .parse_as(&(), message, "objective")?;

        // Validate that all variables used in objective are defined (either as decision variables or parameters)
        let all_variable_ids: VariableIDSet = decision_variable_ids
            .union(&parameter_ids)
            .cloned()
            .collect();
        for id in objective.required_ids() {
            if !all_variable_ids.contains(&id) {
                return Err(RawParseError::InvalidInstance(format!(
                    "Undefined variable ID is used: {id:?}"
                ))
                .context(message, "objective"));
            }
        }

        let (constraints, mut constraint_context): (
            BTreeMap<ConstraintID, Constraint>,
            crate::ConstraintContextStore<ConstraintID>,
        ) = self.constraints.parse_as(&(), message, "constraints")?;

        // Validate that all variables used in constraints are defined (either as decision variables or parameters)
        for constraint in constraints.values() {
            for id in constraint.required_ids() {
                if !all_variable_ids.contains(&id) {
                    return Err(RawParseError::InvalidInstance(format!(
                        "Undefined variable ID is used: {id:?}"
                    ))
                    .context(message, "constraints"));
                }
            }
        }

        // Drain removed-constraint context into the SoA store, then keep
        // (active, removed) tuples without context for the collection.
        let removed_constraints_with_context: BTreeMap<
            ConstraintID,
            (Constraint, ConstraintContext, RemovedReason),
        > = self
            .removed_constraints
            .parse_as(&constraints, message, "removed_constraints")?;
        let mut removed_constraints: BTreeMap<ConstraintID, (Constraint, RemovedReason)> =
            BTreeMap::new();
        for (id, (c, context, reason)) in removed_constraints_with_context {
            constraint_context.insert(id, context);
            removed_constraints.insert(id, (c, reason));
        }

        let (named_functions, named_function_labels): (
            BTreeMap<NamedFunctionID, NamedFunction>,
            crate::named_function::NamedFunctionLabelStore,
        ) = self
            .named_functions
            .parse_as(&(), message, "named_functions")?;

        // Validate that all variables used in named functions are defined (either as decision variables or parameters)
        for named_function in named_functions.values() {
            for id in named_function.function.required_ids() {
                if !all_variable_ids.contains(&id) {
                    return Err(RawParseError::InvalidInstance(format!(
                        "Undefined variable ID is used: {id:?}"
                    ))
                    .context(message, "named_functions"));
                }
            }
        }

        let mut decision_variable_dependency = BTreeMap::default();
        for (id, f) in self.decision_variable_dependency {
            decision_variable_dependency.insert(
                as_variable_id(&decision_variables, id)
                    .map_err(|e| e.context(message, "decision_variable_dependency"))?,
                f.parse_as(&(), message, "decision_variable_dependency")?,
            );
        }
        let decision_variable_dependency = AcyclicAssignments::new(decision_variable_dependency)
            .map_err(|e| RawParseError::from(e).context(message, "decision_variable_dependency"))?;

        let context = (decision_variables, constraints, removed_constraints);
        let constraint_hints = if let Some(hints) = self.constraint_hints {
            hints.parse_as(&context, message, "constraint_hints")?
        } else {
            Default::default()
        };
        let (decision_variables, mut constraints, removed_constraints) = context;

        let (one_hot_active, sos1_active, absorbed_ids) =
            convert_hints_to_collections(&constraint_hints).map_err(|e| {
                RawParseError::InvalidInstance(e.to_string()).context(message, "constraint_hints")
            })?;
        let (one_hot_context, sos1_context) =
            drain_absorbed_hint_context(&constraint_hints, &absorbed_ids, &mut constraint_context);
        // Remove regular constraints that are absorbed by OneHot/SOS1
        for id in &absorbed_ids {
            constraints.remove(id);
        }
        validate_fixed_decision_variable_partition(
            message,
            &objective,
            &constraints,
            &one_hot_active,
            &sos1_active,
            &decision_variable_dependency,
            &fixed_decision_variable_values,
        )?;
        let decision_variables = DecisionVariableTable::with_fixed_values(
            decision_variables,
            variable_labels,
            fixed_decision_variable_values,
            crate::ATol::default(),
        )
        .map_err(|e| {
            RawParseError::InvalidInstance(e.to_string()).context(message, "decision_variables")
        })?;

        Ok(ParametricInstance {
            sense,
            objective,
            decision_variables,
            parameters,
            constraint_collection: ConstraintCollection::with_context(
                constraints,
                removed_constraints,
                constraint_context,
            )
            .map_err(|e| {
                RawParseError::InvalidInstance(e.to_string()).context(message, "constraints")
            })?,
            indicator_constraint_collection: Default::default(),
            one_hot_constraint_collection: ConstraintCollection::with_context(
                one_hot_active,
                BTreeMap::new(),
                one_hot_context,
            )
            .map_err(|e| {
                RawParseError::InvalidInstance(e.to_string()).context(message, "constraint_hints")
            })?,
            sos1_constraint_collection: ConstraintCollection::with_context(
                sos1_active,
                BTreeMap::new(),
                sos1_context,
            )
            .map_err(|e| {
                RawParseError::InvalidInstance(e.to_string()).context(message, "constraint_hints")
            })?,
            named_functions: crate::NamedFunctionTable::new(named_functions, named_function_labels)
                .map_err(|e| {
                    RawParseError::InvalidInstance(e.to_string())
                        .context(message, "named_functions")
                })?,
            decision_variable_dependency,
            description: self.description,
            annotations: self.annotations,
        })
    }
}

impl From<ParametricInstance> for v1::ParametricInstance {
    fn from(
        ParametricInstance {
            sense,
            objective,
            decision_variables,
            parameters,
            constraint_collection,
            indicator_constraint_collection,
            one_hot_constraint_collection,
            sos1_constraint_collection,
            decision_variable_dependency,
            description,
            named_functions,
            annotations,
        }: ParametricInstance,
    ) -> Self {
        // Special constraint types do not have a v1 proto representation yet.
        if !indicator_constraint_collection.active().is_empty()
            || !indicator_constraint_collection.removed().is_empty()
        {
            unimplemented!("Serialization of IndicatorConstraint to v1 proto is not yet supported");
        }
        if !one_hot_constraint_collection.active().is_empty()
            || !one_hot_constraint_collection.removed().is_empty()
        {
            unimplemented!("Serialization of OneHotConstraint to v1 proto is not yet supported");
        }
        if !sos1_constraint_collection.active().is_empty()
            || !sos1_constraint_collection.removed().is_empty()
        {
            unimplemented!("Serialization of Sos1Constraint to v1 proto is not yet supported");
        }
        let v1_decision_variables: Vec<v1::DecisionVariable> = (&decision_variables).into();
        let (v1_constraints, v1_removed_constraints): (
            Vec<v1::Constraint>,
            Vec<v1::RemovedConstraint>,
        ) = constraint_collection.into();
        let v1_named_functions: Vec<v1::NamedFunction> = named_functions.into();
        Self {
            description,
            sense: v1::instance::Sense::from(sense) as i32,
            objective: Some(objective.into()),
            decision_variables: v1_decision_variables,
            parameters: parameters.into_v1_parameters(),
            constraints: v1_constraints,
            named_functions: v1_named_functions,
            removed_constraints: v1_removed_constraints,
            decision_variable_dependency: decision_variable_dependency
                .into_iter()
                .map(|(id, dep)| (id.into(), dep.into()))
                .collect(),
            constraint_hints: None,
            format_version: crate::CURRENT_FORMAT_VERSION,
            annotations: crate::protobuf_extension_annotations(annotations),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance::Instance;
    use proptest::prelude::*;
    use std::collections::HashMap;

    fn binary_decision_variables() -> Vec<v1::DecisionVariable> {
        [0, 1]
            .into_iter()
            .map(|id| {
                decision_variable_to_v1(
                    crate::VariableID::from(id),
                    crate::DecisionVariable::binary(),
                    Default::default(),
                )
            })
            .collect()
    }

    fn decision_variable_to_v1(
        id: VariableID,
        decision_variable: DecisionVariable,
        label: crate::ModelingLabel,
    ) -> v1::DecisionVariable {
        v1::DecisionVariable {
            id: id.into_inner(),
            kind: decision_variable.kind().into(),
            bound: Some(decision_variable.bound().into()),
            substituted_value: None,
            name: label.name,
            subscripts: label.subscripts,
            parameters: label.parameters.into_iter().collect(),
            description: label.description,
        }
    }

    fn constraint_to_v1(
        id: ConstraintID,
        value: Constraint,
        context: ConstraintContext,
    ) -> v1::Constraint {
        let label = context.label;
        v1::Constraint {
            id: id.into_inner(),
            equality: value.equality.into(),
            function: Some(value.stage.function.into()),
            name: label.name,
            subscripts: label.subscripts,
            parameters: label.parameters.into_iter().collect(),
            description: label.description,
        }
    }

    fn removed_constraint_to_v1(
        id: ConstraintID,
        constraint: Constraint,
        context: ConstraintContext,
        removed_reason: RemovedReason,
    ) -> v1::RemovedConstraint {
        v1::RemovedConstraint {
            constraint: Some(constraint_to_v1(id, constraint, context)),
            removed_reason: removed_reason.reason,
            removed_reason_parameters: removed_reason.parameters.into_iter().collect(),
        }
    }

    fn labeled_constraint(id: u64, name: &str) -> v1::Constraint {
        constraint_to_v1(
            ConstraintID::from(id),
            Constraint::equal_to_zero(crate::Function::Zero),
            ConstraintContext {
                label: crate::ModelingLabel {
                    name: Some(name.to_string()),
                    subscripts: vec![id as i64],
                    ..Default::default()
                },
                ..Default::default()
            },
        )
    }

    proptest! {
        #[test]
        fn instance_roundtrip(original_instance in Instance::arbitrary()) {
            let v1_instance: v1::Instance = original_instance.clone().into();
            let roundtripped_instance = Instance::try_from(v1_instance).unwrap();
            assert_eq!(original_instance, roundtripped_instance);
        }
    }

    #[test]
    fn test_instance_parse_rejects_reserved_annotation_key() {
        let v1_instance = v1::Instance {
            annotations: HashMap::from([(
                crate::annotation_keys::INSTANCE_TITLE.to_string(),
                "bad".to_string(),
            )]),
            ..Default::default()
        };
        let result = v1_instance.parse(&());
        insta::assert_snapshot!(result.unwrap_err().to_string(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Instance[annotations]
        Annotation key `org.ommx.v1.instance.title` is reserved for OMMX metadata and cannot be stored in extension annotations.
        "###);
    }

    #[test]
    fn test_instance_parse_transfers_one_hot_hint_context() {
        let v1_instance = v1::Instance {
            sense: v1::instance::Sense::Minimize as i32,
            objective: Some(crate::Function::Zero.into()),
            decision_variables: binary_decision_variables(),
            constraints: vec![labeled_constraint(1, "exactly_one")],
            constraint_hints: Some(v1::ConstraintHints {
                one_hot_constraints: vec![v1::OneHot {
                    constraint_id: 1,
                    decision_variables: vec![0, 1],
                }],
                sos1_constraints: vec![],
            }),
            ..Default::default()
        };

        let parsed = v1_instance.parse(&()).unwrap();
        let regular_id = ConstraintID::from(1);
        let one_hot_id = crate::OneHotConstraintID::from(1);

        assert!(!parsed.constraints().contains_key(&regular_id));
        assert!(!parsed.constraint_context().contains(regular_id));
        assert!(parsed.one_hot_constraints().contains_key(&one_hot_id));
        assert_eq!(
            parsed.one_hot_constraint_context().name(one_hot_id),
            Some("exactly_one")
        );
        assert_eq!(
            parsed.one_hot_constraint_context().subscripts(one_hot_id),
            &[1]
        );
    }

    #[test]
    fn test_parametric_instance_parse_transfers_one_hot_hint_context() {
        let v1_parametric_instance = v1::ParametricInstance {
            sense: v1::instance::Sense::Minimize as i32,
            objective: Some(crate::Function::Zero.into()),
            decision_variables: binary_decision_variables(),
            constraints: vec![labeled_constraint(1, "exactly_one")],
            constraint_hints: Some(v1::ConstraintHints {
                one_hot_constraints: vec![v1::OneHot {
                    constraint_id: 1,
                    decision_variables: vec![0, 1],
                }],
                sos1_constraints: vec![],
            }),
            ..Default::default()
        };

        let parsed = v1_parametric_instance.parse(&()).unwrap();
        let regular_id = ConstraintID::from(1);
        let one_hot_id = crate::OneHotConstraintID::from(1);

        assert!(!parsed.constraints().contains_key(&regular_id));
        assert!(!parsed.constraint_context().contains(regular_id));
        assert!(parsed.one_hot_constraints().contains_key(&one_hot_id));
        assert_eq!(
            parsed.one_hot_constraint_context().name(one_hot_id),
            Some("exactly_one")
        );
        assert_eq!(
            parsed.one_hot_constraint_context().subscripts(one_hot_id),
            &[1]
        );
    }

    #[test]
    fn test_instance_parse_transfers_sos1_binary_hint_context_only() {
        let v1_instance = v1::Instance {
            sense: v1::instance::Sense::Minimize as i32,
            objective: Some(crate::Function::Zero.into()),
            decision_variables: binary_decision_variables(),
            constraints: vec![
                labeled_constraint(10, "sos1_cardinality"),
                labeled_constraint(11, "big_m_encoding"),
            ],
            constraint_hints: Some(v1::ConstraintHints {
                one_hot_constraints: vec![],
                sos1_constraints: vec![v1::Sos1 {
                    binary_constraint_id: 10,
                    big_m_constraint_ids: vec![11],
                    decision_variables: vec![0, 1],
                }],
            }),
            ..Default::default()
        };

        let parsed = v1_instance.parse(&()).unwrap();
        let binary_id = ConstraintID::from(10);
        let big_m_id = ConstraintID::from(11);
        let sos1_id = crate::Sos1ConstraintID::from(10);

        assert!(!parsed.constraints().contains_key(&binary_id));
        assert!(!parsed.constraints().contains_key(&big_m_id));
        assert!(!parsed.constraint_context().contains(binary_id));
        assert!(!parsed.constraint_context().contains(big_m_id));
        assert!(parsed.sos1_constraints().contains_key(&sos1_id));
        assert_eq!(
            parsed.sos1_constraint_context().name(sos1_id),
            Some("sos1_cardinality")
        );
        assert_eq!(parsed.sos1_constraint_context().subscripts(sos1_id), &[10]);
    }

    #[test]
    fn test_parametric_instance_parse_rejects_reserved_annotation_key() {
        let v1_parametric_instance = v1::ParametricInstance {
            annotations: HashMap::from([(
                format!(
                    "{}.title",
                    crate::annotation_keys::PARAMETRIC_INSTANCE_NAMESPACE
                ),
                "bad".to_string(),
            )]),
            ..Default::default()
        };
        let result = v1_parametric_instance.parse(&());
        insta::assert_snapshot!(result.unwrap_err().to_string(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.ParametricInstance[annotations]
        Annotation key `org.ommx.v1.parametric-instance.title` is reserved for OMMX metadata and cannot be stored in extension annotations.
        "###);
    }

    #[test]
    fn test_instance_to_bytes_filters_reserved_annotation_key() {
        let instance = Instance {
            annotations: HashMap::from([
                (
                    crate::annotation_keys::INSTANCE_TITLE.to_string(),
                    "invalid extension title".to_string(),
                ),
                ("org.example.owner".to_string(), "domain".to_string()),
            ]),
            ..Default::default()
        };

        let restored = Instance::from_bytes(&instance.to_bytes()).unwrap();

        assert!(!restored
            .annotations
            .contains_key(crate::annotation_keys::INSTANCE_TITLE));
        assert_eq!(
            restored.annotations.get("org.example.owner"),
            Some(&"domain".to_string())
        );
    }

    #[test]
    fn test_parametric_instance_to_bytes_filters_reserved_annotation_key() {
        let mut instance: crate::ParametricInstance = Instance::default().into();
        let reserved_key = format!(
            "{}.title",
            crate::annotation_keys::PARAMETRIC_INSTANCE_NAMESPACE
        );
        instance.annotations = HashMap::from([
            (reserved_key.clone(), "invalid extension title".to_string()),
            ("org.example.owner".to_string(), "domain".to_string()),
        ]);

        let restored = crate::ParametricInstance::from_bytes(&instance.to_bytes()).unwrap();

        assert!(!restored.annotations.contains_key(&reserved_key));
        assert_eq!(
            restored.annotations.get("org.example.owner"),
            Some(&"domain".to_string())
        );
    }

    #[test]
    fn test_parametric_instance_parse_fails_with_undefined_variable_in_objective() {
        use crate::{coeff, linear, DecisionVariable, Function, VariableID};
        use std::collections::HashMap;

        // Create a v1::ParametricInstance with undefined variable in objective
        let v1_parametric_instance = v1::ParametricInstance {
            sense: v1::instance::Sense::Minimize as i32,
            objective: Some(Function::from(linear!(999) + coeff!(1.0)).into()),
            decision_variables: vec![decision_variable_to_v1(
                VariableID::from(1),
                DecisionVariable::binary(),
                Default::default(),
            )],
            parameters: vec![v1::Parameter {
                id: 100,
                name: Some("p1".to_string()),
                ..Default::default()
            }],
            constraints: vec![],
            named_functions: vec![],
            removed_constraints: vec![],
            decision_variable_dependency: HashMap::new(),
            constraint_hints: None,
            description: None,
            ..Default::default()
        };

        // This should fail because variable ID 999 is used in objective but not defined
        let result = v1_parametric_instance.parse(&());
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.ParametricInstance[objective]
        Undefined variable ID is used: VariableID(999)
        "###);
    }

    #[test]
    fn test_parametric_instance_parse_fails_with_undefined_variable_in_constraint() {
        use crate::{
            coeff, linear, Constraint, ConstraintID, DecisionVariable, Function, VariableID,
        };
        use std::collections::HashMap;

        // Create a v1::ParametricInstance with undefined variable in constraint
        let v1_parametric_instance = v1::ParametricInstance {
            sense: v1::instance::Sense::Minimize as i32,
            objective: Some(Function::from(linear!(1) + coeff!(1.0)).into()),
            decision_variables: vec![decision_variable_to_v1(
                VariableID::from(1),
                DecisionVariable::binary(),
                Default::default(),
            )],
            parameters: vec![v1::Parameter {
                id: 100,
                name: Some("p1".to_string()),
                ..Default::default()
            }],
            constraints: vec![constraint_to_v1(
                ConstraintID::from(1),
                Constraint::equal_to_zero(Function::from(linear!(999) + coeff!(1.0))),
                Default::default(),
            )],
            named_functions: vec![],
            removed_constraints: vec![],
            decision_variable_dependency: HashMap::new(),
            constraint_hints: None,
            description: None,
            ..Default::default()
        };

        // This should fail because variable ID 999 is used in constraint but not defined
        let result = v1_parametric_instance.parse(&());
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.ParametricInstance[constraints]
        Undefined variable ID is used: VariableID(999)
        "###);
    }

    #[test]
    fn test_instance_parse_fails_with_undefined_variable_in_objective() {
        use crate::{coeff, linear, DecisionVariable, Function, VariableID};
        use std::collections::HashMap;

        // Create a v1::Instance with undefined variable in objective
        let v1_instance = v1::Instance {
            sense: v1::instance::Sense::Minimize as i32,
            objective: Some(Function::from(linear!(999) + coeff!(1.0)).into()),
            decision_variables: vec![decision_variable_to_v1(
                VariableID::from(1),
                DecisionVariable::binary(),
                Default::default(),
            )],
            constraints: vec![],
            named_functions: vec![],
            removed_constraints: vec![],
            decision_variable_dependency: HashMap::new(),
            parameters: None,
            description: None,
            constraint_hints: None,
            ..Default::default()
        };

        // This should fail because variable ID 999 is used in objective but not defined
        let result = v1_instance.parse(&());
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Instance[objective]
        Undefined variable ID is used: VariableID(999)
        "###);
    }

    #[test]
    fn test_instance_parse_rejects_fixed_variable_used_in_objective() {
        use crate::{linear, Function};
        use std::collections::HashMap;

        let v1_instance = v1::Instance {
            sense: v1::instance::Sense::Minimize as i32,
            objective: Some(Function::from(linear!(1)).into()),
            decision_variables: vec![v1::DecisionVariable {
                id: 1,
                kind: v1::decision_variable::Kind::Binary as i32,
                bound: Some(crate::Bound::of_binary().into()),
                substituted_value: Some(1.0),
                ..Default::default()
            }],
            constraints: vec![],
            named_functions: vec![],
            removed_constraints: vec![],
            decision_variable_dependency: HashMap::new(),
            parameters: None,
            description: None,
            constraint_hints: None,
            ..Default::default()
        };

        let result = v1_instance.parse(&());
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Instance[decision_variables]
        Fixed variable VariableID(1) cannot be used in objectives or constraints
        "###);
    }

    #[test]
    fn test_instance_parse_fails_with_undefined_variable_in_constraint() {
        use crate::{
            coeff, linear, Constraint, ConstraintID, DecisionVariable, Function, VariableID,
        };
        use std::collections::HashMap;

        // Create a v1::Instance with undefined variable in constraint
        let v1_instance = v1::Instance {
            sense: v1::instance::Sense::Minimize as i32,
            objective: Some(Function::from(linear!(1) + coeff!(1.0)).into()),
            decision_variables: vec![decision_variable_to_v1(
                VariableID::from(1),
                DecisionVariable::binary(),
                Default::default(),
            )],
            constraints: vec![constraint_to_v1(
                ConstraintID::from(1),
                Constraint::equal_to_zero(Function::from(linear!(999) + coeff!(1.0))),
                Default::default(),
            )],
            named_functions: vec![],
            removed_constraints: vec![],
            decision_variable_dependency: HashMap::new(),
            parameters: None,
            description: None,
            constraint_hints: None,
            ..Default::default()
        };

        // This should fail because variable ID 999 is used in constraint but not defined
        let result = v1_instance.parse(&());
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Instance[constraints]
        Undefined variable ID is used: VariableID(999)
        "###);
    }

    #[test]
    fn test_parametric_instance_parse_fails_with_duplicate_constraint_ids() {
        use crate::{
            coeff, linear, Constraint, ConstraintID, DecisionVariable, Function, VariableID,
        };
        use std::collections::HashMap;

        // Create a v1::ParametricInstance with duplicate constraint IDs in constraints and removed_constraints
        let cid = ConstraintID::from(1);
        let constraint = Constraint::equal_to_zero(Function::from(linear!(1) + coeff!(1.0)));
        let removed_reason = crate::constraint::RemovedReason {
            reason: "test".to_string(),
            parameters: Default::default(),
        };
        let removed_constraint =
            removed_constraint_to_v1(cid, constraint.clone(), Default::default(), removed_reason);

        let v1_parametric_instance = v1::ParametricInstance {
            sense: v1::instance::Sense::Minimize as i32,
            objective: Some(Function::from(linear!(1) + coeff!(1.0)).into()),
            decision_variables: vec![decision_variable_to_v1(
                VariableID::from(1),
                DecisionVariable::binary(),
                Default::default(),
            )],
            parameters: vec![v1::Parameter {
                id: 100,
                name: Some("p1".to_string()),
                ..Default::default()
            }],
            constraints: vec![constraint_to_v1(cid, constraint, Default::default())],
            named_functions: vec![],
            removed_constraints: vec![removed_constraint],
            decision_variable_dependency: HashMap::new(),
            constraint_hints: None,
            description: None,
            ..Default::default()
        };

        // This should fail because constraint ID 1 appears in both constraints and removed_constraints
        let result = v1_parametric_instance.parse(&());
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.ParametricInstance[removed_constraints]
        Duplicated constraint ID is found in definition: ConstraintID(1)
        "###);
    }

    #[test]
    fn test_instance_parse_fails_with_duplicate_constraint_ids() {
        use crate::{
            coeff, linear, Constraint, ConstraintID, DecisionVariable, Function, VariableID,
        };
        use std::collections::HashMap;

        // Create a v1::Instance with duplicate constraint IDs in constraints and removed_constraints
        let cid = ConstraintID::from(1);
        let constraint = Constraint::equal_to_zero(Function::from(linear!(1) + coeff!(1.0)));
        let removed_reason = crate::constraint::RemovedReason {
            reason: "test".to_string(),
            parameters: Default::default(),
        };
        let removed_constraint =
            removed_constraint_to_v1(cid, constraint.clone(), Default::default(), removed_reason);

        let v1_instance = v1::Instance {
            sense: v1::instance::Sense::Minimize as i32,
            objective: Some(Function::from(linear!(1) + coeff!(1.0)).into()),
            decision_variables: vec![decision_variable_to_v1(
                VariableID::from(1),
                DecisionVariable::binary(),
                Default::default(),
            )],
            constraints: vec![constraint_to_v1(cid, constraint, Default::default())],
            named_functions: vec![],
            removed_constraints: vec![removed_constraint],
            decision_variable_dependency: HashMap::new(),
            parameters: None,
            description: None,
            constraint_hints: None,
            ..Default::default()
        };

        // This should fail because constraint ID 1 appears in both constraints and removed_constraints
        let result = v1_instance.parse(&());
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Instance[removed_constraints]
        Duplicated constraint ID is found in definition: ConstraintID(1)
        "###);
    }

    #[test]
    fn test_parametric_instance_parse_with_invalid_sense_uses_default() {
        use crate::{coeff, linear, DecisionVariable, Function, Sense, VariableID};
        use std::collections::HashMap;

        // Create a v1::ParametricInstance with invalid sense value
        let v1_parametric_instance = v1::ParametricInstance {
            sense: 999, // Invalid sense value
            objective: Some(Function::from(linear!(1) + coeff!(1.0)).into()),
            decision_variables: vec![decision_variable_to_v1(
                VariableID::from(1),
                DecisionVariable::binary(),
                Default::default(),
            )],
            parameters: vec![v1::Parameter {
                id: 100,
                name: Some("p1".to_string()),
                ..Default::default()
            }],
            constraints: vec![],
            named_functions: vec![],
            removed_constraints: vec![],
            decision_variable_dependency: HashMap::new(),
            constraint_hints: None,
            description: None,
            ..Default::default()
        };

        // Invalid sense value should be converted to default (Minimize)
        let result = v1_parametric_instance.parse(&());
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.sense, Sense::Minimize);
    }

    #[test]
    fn test_instance_parse_with_invalid_sense_uses_default() {
        use crate::{coeff, linear, DecisionVariable, Function, Sense, VariableID};
        use std::collections::HashMap;

        // Create a v1::Instance with invalid sense value
        let v1_instance = v1::Instance {
            sense: 999, // Invalid sense value
            objective: Some(Function::from(linear!(1) + coeff!(1.0)).into()),
            decision_variables: vec![decision_variable_to_v1(
                VariableID::from(1),
                DecisionVariable::binary(),
                Default::default(),
            )],
            constraints: vec![],
            named_functions: vec![],
            removed_constraints: vec![],
            decision_variable_dependency: HashMap::new(),
            parameters: None,
            description: None,
            constraint_hints: None,
            ..Default::default()
        };

        // Invalid sense value should be converted to default (Minimize)
        let result = v1_instance.parse(&());
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.sense, Sense::Minimize);
    }

    #[test]
    fn test_parametric_instance_parse_fails_with_missing_objective() {
        use crate::{DecisionVariable, VariableID};
        use std::collections::HashMap;

        // Create a v1::ParametricInstance without objective
        let v1_parametric_instance = v1::ParametricInstance {
            sense: v1::instance::Sense::Minimize as i32,
            objective: None, // Missing objective
            decision_variables: vec![decision_variable_to_v1(
                VariableID::from(1),
                DecisionVariable::binary(),
                Default::default(),
            )],
            parameters: vec![v1::Parameter {
                id: 100,
                name: Some("p1".to_string()),
                ..Default::default()
            }],
            constraints: vec![],
            named_functions: vec![],
            removed_constraints: vec![],
            decision_variable_dependency: HashMap::new(),
            constraint_hints: None,
            description: None,
            ..Default::default()
        };

        // This should fail because objective is missing
        let result = v1_parametric_instance.parse(&());
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        Field objective in ommx.v1.ParametricInstance is missing.
        "###);
    }

    #[test]
    fn test_instance_parse_fails_with_missing_objective() {
        use crate::{DecisionVariable, VariableID};
        use std::collections::HashMap;

        // Create a v1::Instance without objective
        let v1_instance = v1::Instance {
            sense: v1::instance::Sense::Minimize as i32,
            objective: None, // Missing objective
            decision_variables: vec![decision_variable_to_v1(
                VariableID::from(1),
                DecisionVariable::binary(),
                Default::default(),
            )],
            constraints: vec![],
            named_functions: vec![],
            removed_constraints: vec![],
            decision_variable_dependency: HashMap::new(),
            parameters: None,
            description: None,
            constraint_hints: None,
            ..Default::default()
        };

        // This should fail because objective is missing
        let result = v1_instance.parse(&());
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        Field objective in ommx.v1.Instance is missing.
        "###);
    }

    #[test]
    fn test_parametric_instance_parse_fails_with_duplicated_variable_id() {
        use crate::{coeff, linear, DecisionVariable, Function, VariableID};
        use std::collections::HashMap;

        // Create a v1::ParametricInstance with same ID for decision variable and parameter
        let v1_parametric_instance = v1::ParametricInstance {
            sense: v1::instance::Sense::Minimize as i32,
            objective: Some(Function::from(linear!(1) + coeff!(1.0)).into()),
            decision_variables: vec![decision_variable_to_v1(
                VariableID::from(1),
                DecisionVariable::binary(),
                Default::default(),
            )],
            parameters: vec![v1::Parameter {
                id: 1,
                name: Some("p1".to_string()),
                ..Default::default()
            }], // Same ID as decision variable
            constraints: vec![],
            named_functions: vec![],
            removed_constraints: vec![],
            decision_variable_dependency: HashMap::new(),
            constraint_hints: None,
            description: None,
            ..Default::default()
        };

        // This should fail because ID 1 is used for both decision variable and parameter
        let result = v1_parametric_instance.parse(&());
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.ParametricInstance[parameters]
        Duplicated variable ID is found in definition: VariableID(1)
        "###);
    }

    #[test]
    fn test_parametric_instance_parse_fails_with_duplicated_parameter_id() {
        use crate::{linear, DecisionVariable, Function, VariableID};
        use std::collections::HashMap;

        let v1_parametric_instance = v1::ParametricInstance {
            sense: v1::instance::Sense::Minimize as i32,
            objective: Some(Function::from(linear!(100)).into()),
            decision_variables: vec![decision_variable_to_v1(
                VariableID::from(1),
                DecisionVariable::binary(),
                Default::default(),
            )],
            parameters: vec![
                v1::Parameter {
                    id: 100,
                    name: Some("p".to_string()),
                    ..Default::default()
                },
                v1::Parameter {
                    id: 100,
                    name: Some("q".to_string()),
                    ..Default::default()
                },
            ],
            constraints: vec![],
            named_functions: vec![],
            removed_constraints: vec![],
            decision_variable_dependency: HashMap::new(),
            constraint_hints: None,
            description: None,
            ..Default::default()
        };

        let result = v1_parametric_instance.parse(&());
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.ParametricInstance[parameters]
        Duplicated parameter ID is found in definition: VariableID(100)
        "###);
    }

    #[test]
    fn test_parametric_instance_parse_fails_with_duplicated_constraint_id_in_constraints() {
        use crate::{
            coeff, linear, Constraint, ConstraintID, DecisionVariable, Function, VariableID,
        };
        use std::collections::HashMap;

        // Create a v1::ParametricInstance with duplicate constraint IDs within constraints
        let cid = ConstraintID::from(1);
        let constraint1 = Constraint::equal_to_zero(Function::from(linear!(1) + coeff!(1.0)));
        let constraint2 = Constraint::equal_to_zero(Function::from(linear!(1) + coeff!(2.0))); // Same ID

        let v1_parametric_instance = v1::ParametricInstance {
            sense: v1::instance::Sense::Minimize as i32,
            objective: Some(Function::from(linear!(1) + coeff!(1.0)).into()),
            decision_variables: vec![decision_variable_to_v1(
                VariableID::from(1),
                DecisionVariable::binary(),
                Default::default(),
            )],
            parameters: vec![v1::Parameter {
                id: 100,
                name: Some("p1".to_string()),
                ..Default::default()
            }],
            constraints: vec![
                constraint_to_v1(cid, constraint1, Default::default()),
                constraint_to_v1(cid, constraint2, Default::default()),
            ],
            named_functions: vec![],
            removed_constraints: vec![],
            decision_variable_dependency: HashMap::new(),
            constraint_hints: None,
            description: None,
            ..Default::default()
        };

        // This should fail because constraint ID 1 appears twice in constraints
        let result = v1_parametric_instance.parse(&());
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.ParametricInstance[constraints]
        Duplicated constraint ID is found in definition: ConstraintID(1)
        "###);
    }

    #[test]
    fn test_instance_parse_fails_with_duplicated_constraint_id_in_constraints() {
        use crate::{
            coeff, linear, Constraint, ConstraintID, DecisionVariable, Function, VariableID,
        };
        use std::collections::HashMap;

        // Create a v1::Instance with duplicate constraint IDs within constraints
        let cid = ConstraintID::from(1);
        let constraint1 = Constraint::equal_to_zero(Function::from(linear!(1) + coeff!(1.0)));
        let constraint2 = Constraint::equal_to_zero(Function::from(linear!(1) + coeff!(2.0))); // Same ID

        let v1_instance = v1::Instance {
            sense: v1::instance::Sense::Minimize as i32,
            objective: Some(Function::from(linear!(1) + coeff!(1.0)).into()),
            decision_variables: vec![decision_variable_to_v1(
                VariableID::from(1),
                DecisionVariable::binary(),
                Default::default(),
            )],
            constraints: vec![
                constraint_to_v1(cid, constraint1, Default::default()),
                constraint_to_v1(cid, constraint2, Default::default()),
            ],
            named_functions: vec![],
            removed_constraints: vec![],
            decision_variable_dependency: HashMap::new(),
            parameters: None,
            description: None,
            constraint_hints: None,
            ..Default::default()
        };

        // This should fail because constraint ID 1 appears twice in constraints
        let result = v1_instance.parse(&());
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Instance[constraints]
        Duplicated constraint ID is found in definition: ConstraintID(1)
        "###);
    }

    // Data produced by a future SDK whose format version exceeds what this SDK supports
    // must be rejected with a clear upgrade-the-SDK error rather than silently misread.
    #[test]
    fn test_instance_parse_rejects_future_format_version() {
        let v1_instance = v1::Instance {
            sense: v1::instance::Sense::Minimize as i32,
            objective: Some(Default::default()),
            format_version: 1,
            ..Default::default()
        };
        let result = v1_instance.parse(&());
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Instance[format_version]
        Unsupported ommx format version: data has format_version=1, but this SDK supports up to 0. Please upgrade the OMMX SDK.
        "###);
    }

    #[test]
    fn test_parametric_instance_parse_rejects_future_format_version() {
        let v1_parametric_instance = v1::ParametricInstance {
            sense: v1::instance::Sense::Minimize as i32,
            objective: Some(Default::default()),
            format_version: 1,
            ..Default::default()
        };
        let result = v1_parametric_instance.parse(&());
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.ParametricInstance[format_version]
        Unsupported ommx format version: data has format_version=1, but this SDK supports up to 0. Please upgrade the OMMX SDK.
        "###);
    }

    /// Regression: `From<ParametricInstance> for v1::ParametricInstance`
    /// must drain both the variable labels and constraint context stores
    /// onto each per-element proto, the same way `Instance` does. A
    /// previous version of this conversion bound `variable_labels: _`
    /// and `let (.., _context) = into_parts()`, silently dropping every
    /// name / subscript / parameter / description across a bytes
    /// round-trip.
    #[test]
    fn test_parametric_instance_roundtrip_preserves_labels_and_context() {
        use crate::{
            coeff, linear, Constraint, ConstraintContext, ConstraintID, DecisionVariable, Function,
            ModelingLabel, Sense, VariableID,
        };

        let var_id = VariableID::from(1);
        let cid = ConstraintID::from(10);

        let mut instance = ParametricInstance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(1)))
            .decision_variables(maplit::btreemap! {
                var_id => DecisionVariable::binary(),
            })
            .parameters(ParameterTable::default())
            .constraints(maplit::btreemap! {
                cid => Constraint::equal_to_zero(Function::from(linear!(1) + coeff!(-1.0))),
            })
            .build()
            .unwrap();

        instance
            .set_variable_label(
                var_id,
                ModelingLabel {
                    name: Some("x".to_string()),
                    subscripts: vec![0],
                    ..Default::default()
                },
            )
            .unwrap();
        instance
            .set_constraint_context(
                cid,
                ConstraintContext {
                    label: ModelingLabel {
                        name: Some("balance".to_string()),
                        description: Some("demand-balance row".to_string()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .unwrap();

        let bytes = instance.to_bytes();
        let recovered = ParametricInstance::from_bytes(&bytes).unwrap();

        assert_eq!(recovered.variable_labels().name(var_id), Some("x"));
        assert_eq!(recovered.variable_labels().subscripts(var_id), &[0]);
        assert_eq!(
            recovered.constraint_collection().context().name(cid),
            Some("balance"),
        );
        assert_eq!(
            recovered.constraint_collection().context().description(cid),
            Some("demand-balance row"),
        );
    }

    /// Regression: `From<Instance> for v1::Instance` and the matching
    /// `Parse for v1::Instance` must drain / re-attach the
    /// `named_function_labels` SoA store across a bytes round-trip,
    /// the same way constraint context and variable labels do.
    #[test]
    fn test_instance_roundtrip_preserves_named_function_labels() {
        use crate::{
            coeff, linear, DecisionVariable, Function, NamedFunctionID, Sense, VariableID,
        };

        let var_id = VariableID::from(1);
        let nf_id = NamedFunctionID::from(0);

        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(1)))
            .decision_variables(maplit::btreemap! {
                var_id => DecisionVariable::binary(),
            })
            .constraints(BTreeMap::new())
            .build()
            .unwrap();

        instance
            .new_named_function(
                Function::from(linear!(1) + coeff!(1.0)),
                Some("offset_x".to_string()),
                vec![0],
                fnv::FnvHashMap::default(),
                Some("x plus a constant".to_string()),
            )
            .unwrap();

        let bytes = instance.to_bytes();
        let recovered = Instance::from_bytes(&bytes).unwrap();

        assert_eq!(
            recovered.named_function_labels().name(nf_id),
            Some("offset_x"),
        );
        assert_eq!(recovered.named_function_labels().subscripts(nf_id), &[0]);
        assert_eq!(
            recovered.named_function_labels().description(nf_id),
            Some("x plus a constant"),
        );
    }
}
