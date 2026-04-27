use super::*;
use crate::{
    constraint::{ConstraintMetadata, RemovedReason},
    constraint_hints::ConstraintHints,
    constraint_type::ConstraintCollection,
    parse::{as_variable_id, Parse, ParseError, RawParseError},
    v1::{self},
    Constraint, ConstraintID, VariableID,
};

/// Convert parsed `ConstraintHints` to first-class OneHot/SOS1 constraint collections,
/// and return the set of regular constraint IDs that should be removed from the
/// constraint collection (they are subsumed by the new first-class constraints).
fn convert_hints_to_collections(
    hints: &ConstraintHints,
) -> (
    BTreeMap<crate::OneHotConstraintID, crate::OneHotConstraint>,
    BTreeMap<crate::Sos1ConstraintID, crate::Sos1Constraint>,
    std::collections::BTreeSet<crate::ConstraintID>,
) {
    let mut one_hot_active = BTreeMap::new();
    let mut absorbed_constraint_ids = std::collections::BTreeSet::new();
    for hint in &hints.one_hot_constraints {
        let id = crate::OneHotConstraintID::from(*hint.id);
        one_hot_active.insert(id, crate::OneHotConstraint::new(hint.variables.clone()));
        absorbed_constraint_ids.insert(hint.id);
    }
    let mut sos1_active = BTreeMap::new();
    for hint in &hints.sos1_constraints {
        let id = crate::Sos1ConstraintID::from(*hint.binary_constraint_id);
        sos1_active.insert(id, crate::Sos1Constraint::new(hint.variables.clone()));
        absorbed_constraint_ids.insert(hint.binary_constraint_id);
        absorbed_constraint_ids.extend(&hint.big_m_constraint_ids);
    }
    (one_hot_active, sos1_active, absorbed_constraint_ids)
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

/// Build a v1 `Constraint` from per-element data plus drained metadata.
///
/// Per-element constraint structs no longer carry metadata; the enclosing
/// collection is the canonical source. Serialization paths fetch the
/// metadata from the collection's [`ConstraintMetadataStore`] and join it
/// at this boundary.
pub(crate) fn constraint_to_v1(
    id: ConstraintID,
    value: Constraint,
    metadata: ConstraintMetadata,
) -> v1::Constraint {
    v1::Constraint {
        id: id.into_inner(),
        equality: value.equality.into(),
        function: Some(value.stage.function.into()),
        name: metadata.name,
        subscripts: metadata.subscripts,
        parameters: metadata.parameters.into_iter().collect(),
        description: metadata.description,
    }
}

pub(crate) fn removed_constraint_to_v1(
    id: ConstraintID,
    constraint: Constraint,
    metadata: ConstraintMetadata,
    removed_reason: RemovedReason,
) -> v1::RemovedConstraint {
    v1::RemovedConstraint {
        constraint: Some(constraint_to_v1(id, constraint, metadata)),
        removed_reason: removed_reason.reason,
        removed_reason_parameters: removed_reason.parameters.into_iter().collect(),
    }
}

// NOTE: There are intentionally no `impl From<(ConstraintID, Constraint)>
// for v1::Constraint` (or `v1::RemovedConstraint`). v3 keeps metadata at
// the collection layer, so a per-element conversion would have to default
// every metadata field — silently dropping any caller-supplied metadata.
// Callers must instead go through [`constraint_to_v1`] /
// [`removed_constraint_to_v1`], which take the metadata explicitly.
// `From<Instance> for v1::Instance` (above) drains the SoA store and
// threads the metadata through these helpers.

impl Parse for v1::Instance {
    type Output = Instance;
    type Context = ();
    fn parse(self, _context: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.Instance";
        crate::parse::check_format_version(self.format_version, message)?;
        let sense = self.sense().parse_as(&(), message, "sense")?;

        let (decision_variables, variable_metadata): (
            BTreeMap<VariableID, DecisionVariable>,
            crate::VariableMetadataStore,
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

        let (constraints, mut constraint_metadata): (
            BTreeMap<ConstraintID, Constraint>,
            crate::ConstraintMetadataStore<ConstraintID>,
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
        // joined with metadata + removed reason. Strip metadata into the SoA store
        // so the collection-level `(active, removed)` shape stays uniform.
        let removed_constraints_with_metadata: BTreeMap<
            ConstraintID,
            (Constraint, ConstraintMetadata, RemovedReason),
        > = self
            .removed_constraints
            .parse_as(&constraints, message, "removed_constraints")?;
        let mut removed_constraints: BTreeMap<ConstraintID, (Constraint, RemovedReason)> =
            BTreeMap::new();
        for (id, (c, metadata, reason)) in removed_constraints_with_metadata {
            constraint_metadata.insert(id, metadata);
            removed_constraints.insert(id, (c, reason));
        }

        let named_functions = self
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
            convert_hints_to_collections(&constraint_hints);
        // Remove regular constraints that are absorbed by OneHot/SOS1
        for id in &absorbed_ids {
            constraints.remove(id);
        }

        Ok(Instance {
            sense,
            objective,
            decision_variables,
            variable_metadata,
            constraint_collection: ConstraintCollection::with_metadata(
                constraints,
                removed_constraints,
                constraint_metadata,
            ),
            indicator_constraint_collection: Default::default(),
            one_hot_constraint_collection: ConstraintCollection::new(
                one_hot_active,
                BTreeMap::new(),
            ),
            sos1_constraint_collection: ConstraintCollection::new(sos1_active, BTreeMap::new()),
            decision_variable_dependency,
            parameters: self.parameters,
            description: self.description,
            named_functions,
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
        // Drain per-element data and join with metadata from the SoA stores.
        let variable_metadata = value.variable_metadata;
        let decision_variables = value
            .decision_variables
            .into_iter()
            .map(|(id, dv)| {
                let metadata = variable_metadata.collect_for(id);
                crate::decision_variable::parse::decision_variable_to_v1(dv, metadata)
            })
            .collect();
        let (active, removed, mut constraint_metadata) = value.constraint_collection.into_parts();
        let constraints = active
            .into_iter()
            .map(|(id, c)| constraint_to_v1(id, c, constraint_metadata.remove(id)))
            .collect();
        let named_functions = value
            .named_functions
            .into_values()
            .map(|nf| nf.into())
            .collect();
        let removed_constraints = removed
            .into_iter()
            .map(|(id, (c, r))| removed_constraint_to_v1(id, c, constraint_metadata.remove(id), r))
            .collect();
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
        }
    }
}

impl Parse for v1::ParametricInstance {
    type Output = ParametricInstance;
    type Context = ();
    fn parse(self, _context: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.ParametricInstance";
        crate::parse::check_format_version(self.format_version, message)?;
        let sense = self.sense().parse_as(&(), message, "sense")?;

        let (decision_variables, variable_metadata): (
            BTreeMap<VariableID, DecisionVariable>,
            crate::VariableMetadataStore,
        ) = self
            .decision_variables
            .parse_as(&(), message, "decision_variables")?;

        let parameters: BTreeMap<VariableID, v1::Parameter> = self
            .parameters
            .into_iter()
            .map(|p| (VariableID::from(p.id), p))
            .collect();

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

        let (constraints, mut constraint_metadata): (
            BTreeMap<ConstraintID, Constraint>,
            crate::ConstraintMetadataStore<ConstraintID>,
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

        // Drain removed-constraint metadata into the SoA store, then keep
        // (active, removed) tuples without metadata for the collection.
        let removed_constraints_with_metadata: BTreeMap<
            ConstraintID,
            (Constraint, ConstraintMetadata, RemovedReason),
        > = self
            .removed_constraints
            .parse_as(&constraints, message, "removed_constraints")?;
        let mut removed_constraints: BTreeMap<ConstraintID, (Constraint, RemovedReason)> =
            BTreeMap::new();
        for (id, (c, metadata, reason)) in removed_constraints_with_metadata {
            constraint_metadata.insert(id, metadata);
            removed_constraints.insert(id, (c, reason));
        }

        let named_functions = self
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
            convert_hints_to_collections(&constraint_hints);
        // Remove regular constraints that are absorbed by OneHot/SOS1
        for id in &absorbed_ids {
            constraints.remove(id);
        }

        Ok(ParametricInstance {
            sense,
            objective,
            decision_variables,
            parameters,
            variable_metadata,
            constraint_collection: ConstraintCollection::with_metadata(
                constraints,
                removed_constraints,
                constraint_metadata,
            ),
            indicator_constraint_collection: Default::default(),
            one_hot_constraint_collection: ConstraintCollection::new(
                one_hot_active,
                BTreeMap::new(),
            ),
            sos1_constraint_collection: ConstraintCollection::new(sos1_active, BTreeMap::new()),
            named_functions,
            decision_variable_dependency,
            description: self.description,
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
            variable_metadata,
            constraint_collection,
            indicator_constraint_collection,
            one_hot_constraint_collection,
            sos1_constraint_collection,
            decision_variable_dependency,
            description,
            named_functions,
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
        // Drain per-element data and join with metadata from the SoA stores.
        // (Same shape as `From<Instance> for v1::Instance` above; a stale
        // version of this conversion silently dropped both metadata stores.)
        let v1_decision_variables = decision_variables
            .into_iter()
            .map(|(id, dv)| {
                let metadata = variable_metadata.collect_for(id);
                crate::decision_variable::parse::decision_variable_to_v1(dv, metadata)
            })
            .collect();
        let (active, removed, mut constraint_metadata) = constraint_collection.into_parts();
        let v1_constraints = active
            .into_iter()
            .map(|(id, c)| constraint_to_v1(id, c, constraint_metadata.remove(id)))
            .collect();
        let v1_removed_constraints = removed
            .into_iter()
            .map(|(id, (c, r))| removed_constraint_to_v1(id, c, constraint_metadata.remove(id), r))
            .collect();
        Self {
            description,
            sense: v1::instance::Sense::from(sense) as i32,
            objective: Some(objective.into()),
            decision_variables: v1_decision_variables,
            parameters: parameters.into_values().collect(),
            constraints: v1_constraints,
            named_functions: named_functions.into_values().map(|nf| nf.into()).collect(),
            removed_constraints: v1_removed_constraints,
            decision_variable_dependency: decision_variable_dependency
                .into_iter()
                .map(|(id, dep)| (id.into(), dep.into()))
                .collect(),
            constraint_hints: None,
            format_version: crate::CURRENT_FORMAT_VERSION,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance::Instance;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn instance_roundtrip(original_instance in Instance::arbitrary()) {
            let v1_instance: v1::Instance = original_instance.clone().into();
            let roundtripped_instance = Instance::try_from(v1_instance).unwrap();
            assert_eq!(original_instance, roundtripped_instance);
        }
    }

    #[test]
    fn test_parametric_instance_parse_fails_with_undefined_variable_in_objective() {
        use crate::{coeff, linear, DecisionVariable, Function, VariableID};
        use std::collections::HashMap;

        // Create a v1::ParametricInstance with undefined variable in objective
        let v1_parametric_instance = v1::ParametricInstance {
            sense: v1::instance::Sense::Minimize as i32,
            objective: Some(Function::from(linear!(999) + coeff!(1.0)).into()),
            decision_variables: vec![crate::decision_variable::parse::decision_variable_to_v1(
                DecisionVariable::binary(VariableID::from(1)),
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
            decision_variables: vec![crate::decision_variable::parse::decision_variable_to_v1(
                DecisionVariable::binary(VariableID::from(1)),
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
            decision_variables: vec![crate::decision_variable::parse::decision_variable_to_v1(
                DecisionVariable::binary(VariableID::from(1)),
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
    fn test_instance_parse_fails_with_undefined_variable_in_constraint() {
        use crate::{
            coeff, linear, Constraint, ConstraintID, DecisionVariable, Function, VariableID,
        };
        use std::collections::HashMap;

        // Create a v1::Instance with undefined variable in constraint
        let v1_instance = v1::Instance {
            sense: v1::instance::Sense::Minimize as i32,
            objective: Some(Function::from(linear!(1) + coeff!(1.0)).into()),
            decision_variables: vec![crate::decision_variable::parse::decision_variable_to_v1(
                DecisionVariable::binary(VariableID::from(1)),
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
            decision_variables: vec![crate::decision_variable::parse::decision_variable_to_v1(
                DecisionVariable::binary(VariableID::from(1)),
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
            decision_variables: vec![crate::decision_variable::parse::decision_variable_to_v1(
                DecisionVariable::binary(VariableID::from(1)),
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
            decision_variables: vec![crate::decision_variable::parse::decision_variable_to_v1(
                DecisionVariable::binary(VariableID::from(1)),
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
            decision_variables: vec![crate::decision_variable::parse::decision_variable_to_v1(
                DecisionVariable::binary(VariableID::from(1)),
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
            decision_variables: vec![crate::decision_variable::parse::decision_variable_to_v1(
                DecisionVariable::binary(VariableID::from(1)),
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
            decision_variables: vec![crate::decision_variable::parse::decision_variable_to_v1(
                DecisionVariable::binary(VariableID::from(1)),
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
            decision_variables: vec![crate::decision_variable::parse::decision_variable_to_v1(
                DecisionVariable::binary(VariableID::from(1)),
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
            decision_variables: vec![crate::decision_variable::parse::decision_variable_to_v1(
                DecisionVariable::binary(VariableID::from(1)),
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
            decision_variables: vec![crate::decision_variable::parse::decision_variable_to_v1(
                DecisionVariable::binary(VariableID::from(1)),
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
    /// must drain both the variable and the constraint metadata stores
    /// onto each per-element proto, the same way `Instance` does. A
    /// previous version of this conversion bound `variable_metadata: _`
    /// and `let (.., _metadata) = into_parts()`, silently dropping every
    /// name / subscript / parameter / description across a bytes
    /// round-trip.
    #[test]
    fn test_parametric_instance_roundtrip_preserves_metadata() {
        use crate::{
            coeff, linear, Constraint, ConstraintID, DecisionVariable, Function, Sense, VariableID,
        };

        let var_id = VariableID::from(1);
        let cid = ConstraintID::from(10);

        let mut instance = ParametricInstance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(1)))
            .decision_variables(maplit::btreemap! {
                var_id => DecisionVariable::binary(var_id),
            })
            .parameters(BTreeMap::new())
            .constraints(maplit::btreemap! {
                cid => Constraint::equal_to_zero(Function::from(linear!(1) + coeff!(-1.0))),
            })
            .build()
            .unwrap();

        instance.variable_metadata_mut().set_name(var_id, "x");
        instance
            .variable_metadata_mut()
            .set_subscripts(var_id, vec![0]);
        instance
            .constraint_collection_mut()
            .metadata_mut()
            .set_name(cid, "balance");
        instance
            .constraint_collection_mut()
            .metadata_mut()
            .set_description(cid, "demand-balance row");

        let bytes = instance.to_bytes();
        let recovered = ParametricInstance::from_bytes(&bytes).unwrap();

        assert_eq!(recovered.variable_metadata().name(var_id), Some("x"));
        assert_eq!(recovered.variable_metadata().subscripts(var_id), &[0]);
        assert_eq!(
            recovered.constraint_collection().metadata().name(cid),
            Some("balance"),
        );
        assert_eq!(
            recovered
                .constraint_collection()
                .metadata()
                .description(cid),
            Some("demand-balance row"),
        );
    }
}
