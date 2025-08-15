use super::*;
use crate::{
    parse::{as_variable_id, Parse, ParseError, RawParseError},
    v1::{self},
    Constraint, InstanceError, VariableID,
};

impl Parse for v1::instance::Sense {
    type Output = Sense;
    type Context = ();
    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        match self {
            v1::instance::Sense::Minimize => Ok(Sense::Minimize),
            v1::instance::Sense::Maximize => Ok(Sense::Maximize),
            v1::instance::Sense::Unspecified => {
                log::warn!("Unspecified ommx.v1.instance.Sense found, defaulting to Minimize");
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

impl From<Constraint> for v1::Constraint {
    fn from(value: Constraint) -> Self {
        Self {
            id: *value.id,
            equality: value.equality.into(),
            function: Some(value.function.into()),
            name: value.name,
            subscripts: value.subscripts,
            parameters: value.parameters.into_iter().collect(),
            description: value.description,
        }
    }
}

impl From<RemovedConstraint> for v1::RemovedConstraint {
    fn from(value: RemovedConstraint) -> Self {
        Self {
            constraint: Some(value.constraint.into()),
            removed_reason: value.removed_reason,
            removed_reason_parameters: value.removed_reason_parameters.into_iter().collect(),
        }
    }
}

impl Parse for v1::Instance {
    type Output = Instance;
    type Context = ();
    fn parse(self, _context: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.Instance";
        let sense = self.sense().parse_as(&(), message, "sense")?;

        let decision_variables =
            self.decision_variables
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
                return Err(
                    RawParseError::from(InstanceError::UndefinedVariableID { id })
                        .context(message, "objective"),
                );
            }
        }

        let constraints = self.constraints.parse_as(&(), message, "constraints")?;

        // Validate that all variables used in constraints are defined as decision variables
        for constraint in constraints.values() {
            for id in constraint.required_ids() {
                if !decision_variable_ids.contains(&id) {
                    return Err(
                        RawParseError::from(InstanceError::UndefinedVariableID { id })
                            .context(message, "constraints"),
                    );
                }
            }
        }
        let removed_constraints =
            self.removed_constraints
                .parse_as(&constraints, message, "removed_constraints")?;

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
        let (decision_variables, constraints, removed_constraints) = context;

        Ok(Instance {
            sense,
            objective,
            constraints,
            decision_variables,
            removed_constraints,
            decision_variable_dependency,
            parameters: self.parameters,
            description: self.description,
            constraint_hints,
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
        let decision_variables = value
            .decision_variables
            .into_values()
            .map(|dv| dv.into())
            .collect();
        let constraints = value.constraints.into_values().map(|c| c.into()).collect();
        let removed_constraints = value
            .removed_constraints
            .into_values()
            .map(|rc| rc.into())
            .collect();
        let decision_variable_dependency = value
            .decision_variable_dependency
            .into_iter()
            .map(|(id, dep)| (id.into(), dep.into()))
            .collect();
        Self {
            sense: v1::instance::Sense::from(value.sense).into(),
            decision_variables,
            objective: Some(value.objective.into()),
            constraints,
            removed_constraints,
            decision_variable_dependency,
            parameters: value.parameters,
            description: value.description,
            constraint_hints: Some(value.constraint_hints.into()),
        }
    }
}

impl Parse for v1::ParametricInstance {
    type Output = ParametricInstance;
    type Context = ();
    fn parse(self, _context: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.ParametricInstance";
        let sense = self.sense().parse_as(&(), message, "sense")?;

        let decision_variables =
            self.decision_variables
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
            return Err(RawParseError::from(InstanceError::DuplicatedVariableID {
                id: *intersection.iter().next().unwrap(),
            })
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
                return Err(
                    RawParseError::from(InstanceError::UndefinedVariableID { id })
                        .context(message, "objective"),
                );
            }
        }

        let constraints = self.constraints.parse_as(&(), message, "constraints")?;

        // Validate that all variables used in constraints are defined (either as decision variables or parameters)
        for constraint in constraints.values() {
            for id in constraint.required_ids() {
                if !all_variable_ids.contains(&id) {
                    return Err(
                        RawParseError::from(InstanceError::UndefinedVariableID { id })
                            .context(message, "constraints"),
                    );
                }
            }
        }

        let removed_constraints =
            self.removed_constraints
                .parse_as(&constraints, message, "removed_constraints")?;

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
        let (decision_variables, constraints, removed_constraints) = context;

        Ok(ParametricInstance {
            sense,
            objective,
            decision_variables,
            parameters,
            constraints,
            removed_constraints,
            decision_variable_dependency,
            constraint_hints,
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
            constraints,
            removed_constraints,
            decision_variable_dependency,
            constraint_hints,
            description,
        }: ParametricInstance,
    ) -> Self {
        Self {
            description,
            sense: v1::instance::Sense::from(sense) as i32,
            objective: Some(objective.into()),
            decision_variables: decision_variables
                .into_values()
                .map(|dv| dv.into())
                .collect(),
            parameters: parameters.into_values().collect(),
            constraints: constraints.into_values().map(|c| c.into()).collect(),
            removed_constraints: removed_constraints
                .into_values()
                .map(|rc| rc.into())
                .collect(),
            decision_variable_dependency: decision_variable_dependency
                .into_iter()
                .map(|(id, dep)| (id.into(), dep.into()))
                .collect(),
            constraint_hints: if constraint_hints.is_empty() {
                None
            } else {
                Some(constraint_hints.into())
            },
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
            decision_variables: vec![DecisionVariable::binary(VariableID::from(1)).into()],
            parameters: vec![v1::Parameter {
                id: 100,
                name: Some("p1".to_string()),
                ..Default::default()
            }],
            constraints: vec![],
            removed_constraints: vec![],
            decision_variable_dependency: HashMap::new(),
            constraint_hints: None,
            description: None,
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
            decision_variables: vec![DecisionVariable::binary(VariableID::from(1)).into()],
            parameters: vec![v1::Parameter {
                id: 100,
                name: Some("p1".to_string()),
                ..Default::default()
            }],
            constraints: vec![Constraint::equal_to_zero(
                ConstraintID::from(1),
                Function::from(linear!(999) + coeff!(1.0)),
            )
            .into()],
            removed_constraints: vec![],
            decision_variable_dependency: HashMap::new(),
            constraint_hints: None,
            description: None,
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
            decision_variables: vec![DecisionVariable::binary(VariableID::from(1)).into()],
            constraints: vec![],
            removed_constraints: vec![],
            decision_variable_dependency: HashMap::new(),
            parameters: None,
            description: None,
            constraint_hints: None,
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
            decision_variables: vec![DecisionVariable::binary(VariableID::from(1)).into()],
            constraints: vec![Constraint::equal_to_zero(
                ConstraintID::from(1),
                Function::from(linear!(999) + coeff!(1.0)),
            )
            .into()],
            removed_constraints: vec![],
            decision_variable_dependency: HashMap::new(),
            parameters: None,
            description: None,
            constraint_hints: None,
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
            coeff, linear, Constraint, ConstraintID, DecisionVariable, Function, RemovedConstraint,
            VariableID,
        };
        use std::collections::HashMap;

        // Create a v1::ParametricInstance with duplicate constraint IDs in constraints and removed_constraints
        let constraint = Constraint::equal_to_zero(
            ConstraintID::from(1),
            Function::from(linear!(1) + coeff!(1.0)),
        );
        let removed_constraint = RemovedConstraint {
            constraint: constraint.clone(),
            removed_reason: "test".to_string(),
            removed_reason_parameters: Default::default(),
        };

        let v1_parametric_instance = v1::ParametricInstance {
            sense: v1::instance::Sense::Minimize as i32,
            objective: Some(Function::from(linear!(1) + coeff!(1.0)).into()),
            decision_variables: vec![DecisionVariable::binary(VariableID::from(1)).into()],
            parameters: vec![v1::Parameter {
                id: 100,
                name: Some("p1".to_string()),
                ..Default::default()
            }],
            constraints: vec![constraint.into()],
            removed_constraints: vec![removed_constraint.into()],
            decision_variable_dependency: HashMap::new(),
            constraint_hints: None,
            description: None,
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
            coeff, linear, Constraint, ConstraintID, DecisionVariable, Function, RemovedConstraint,
            VariableID,
        };
        use std::collections::HashMap;

        // Create a v1::Instance with duplicate constraint IDs in constraints and removed_constraints
        let constraint = Constraint::equal_to_zero(
            ConstraintID::from(1),
            Function::from(linear!(1) + coeff!(1.0)),
        );
        let removed_constraint = RemovedConstraint {
            constraint: constraint.clone(),
            removed_reason: "test".to_string(),
            removed_reason_parameters: Default::default(),
        };

        let v1_instance = v1::Instance {
            sense: v1::instance::Sense::Minimize as i32,
            objective: Some(Function::from(linear!(1) + coeff!(1.0)).into()),
            decision_variables: vec![DecisionVariable::binary(VariableID::from(1)).into()],
            constraints: vec![constraint.into()],
            removed_constraints: vec![removed_constraint.into()],
            decision_variable_dependency: HashMap::new(),
            parameters: None,
            description: None,
            constraint_hints: None,
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
            decision_variables: vec![DecisionVariable::binary(VariableID::from(1)).into()],
            parameters: vec![v1::Parameter {
                id: 100,
                name: Some("p1".to_string()),
                ..Default::default()
            }],
            constraints: vec![],
            removed_constraints: vec![],
            decision_variable_dependency: HashMap::new(),
            constraint_hints: None,
            description: None,
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
            decision_variables: vec![DecisionVariable::binary(VariableID::from(1)).into()],
            constraints: vec![],
            removed_constraints: vec![],
            decision_variable_dependency: HashMap::new(),
            parameters: None,
            description: None,
            constraint_hints: None,
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
            decision_variables: vec![DecisionVariable::binary(VariableID::from(1)).into()],
            parameters: vec![v1::Parameter {
                id: 100,
                name: Some("p1".to_string()),
                ..Default::default()
            }],
            constraints: vec![],
            removed_constraints: vec![],
            decision_variable_dependency: HashMap::new(),
            constraint_hints: None,
            description: None,
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
            decision_variables: vec![DecisionVariable::binary(VariableID::from(1)).into()],
            constraints: vec![],
            removed_constraints: vec![],
            decision_variable_dependency: HashMap::new(),
            parameters: None,
            description: None,
            constraint_hints: None,
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
            decision_variables: vec![DecisionVariable::binary(VariableID::from(1)).into()],
            parameters: vec![v1::Parameter {
                id: 1,
                name: Some("p1".to_string()),
                ..Default::default()
            }], // Same ID as decision variable
            constraints: vec![],
            removed_constraints: vec![],
            decision_variable_dependency: HashMap::new(),
            constraint_hints: None,
            description: None,
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
        let constraint1 = Constraint::equal_to_zero(
            ConstraintID::from(1),
            Function::from(linear!(1) + coeff!(1.0)),
        );
        let constraint2 = Constraint::equal_to_zero(
            ConstraintID::from(1),
            Function::from(linear!(1) + coeff!(2.0)),
        ); // Same ID

        let v1_parametric_instance = v1::ParametricInstance {
            sense: v1::instance::Sense::Minimize as i32,
            objective: Some(Function::from(linear!(1) + coeff!(1.0)).into()),
            decision_variables: vec![DecisionVariable::binary(VariableID::from(1)).into()],
            parameters: vec![v1::Parameter {
                id: 100,
                name: Some("p1".to_string()),
                ..Default::default()
            }],
            constraints: vec![constraint1.into(), constraint2.into()],
            removed_constraints: vec![],
            decision_variable_dependency: HashMap::new(),
            constraint_hints: None,
            description: None,
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
        let constraint1 = Constraint::equal_to_zero(
            ConstraintID::from(1),
            Function::from(linear!(1) + coeff!(1.0)),
        );
        let constraint2 = Constraint::equal_to_zero(
            ConstraintID::from(1),
            Function::from(linear!(1) + coeff!(2.0)),
        ); // Same ID

        let v1_instance = v1::Instance {
            sense: v1::instance::Sense::Minimize as i32,
            objective: Some(Function::from(linear!(1) + coeff!(1.0)).into()),
            decision_variables: vec![DecisionVariable::binary(VariableID::from(1)).into()],
            constraints: vec![constraint1.into(), constraint2.into()],
            removed_constraints: vec![],
            decision_variable_dependency: HashMap::new(),
            parameters: None,
            description: None,
            constraint_hints: None,
        };

        // This should fail because constraint ID 1 appears twice in constraints
        let result = v1_instance.parse(&());
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Instance[constraints]
        Duplicated constraint ID is found in definition: ConstraintID(1)
        "###);
    }
}
