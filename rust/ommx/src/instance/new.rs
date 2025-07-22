use super::*;
use crate::{
    v1, AcyclicAssignments, Constraint, ConstraintID, DecisionVariable, Evaluate, Function,
    VariableID, VariableIDSet,
};
use std::collections::BTreeMap;

impl Instance {
    pub fn new(
        sense: Sense,
        objective: Function,
        decision_variables: BTreeMap<VariableID, DecisionVariable>,
        constraints: BTreeMap<ConstraintID, Constraint>,
    ) -> anyhow::Result<Self> {
        let variable_ids: VariableIDSet = decision_variables.keys().cloned().collect();
        for id in objective.required_ids() {
            if !variable_ids.contains(&id) {
                return Err(InstanceError::UndefinedVariableID { id }.into());
            }
        }
        for constraint in constraints.values() {
            for id in constraint.required_ids() {
                if !variable_ids.contains(&id) {
                    return Err(InstanceError::UndefinedVariableID { id }.into());
                }
            }
        }

        Ok(Instance {
            sense,
            objective,
            decision_variables,
            constraints,
            removed_constraints: BTreeMap::new(),
            decision_variable_dependency: AcyclicAssignments::default(),
            constraint_hints: ConstraintHints::default(),
            parameters: None,
            description: None,
        })
    }
}

impl ParametricInstance {
    pub fn new(
        sense: Sense,
        objective: Function,
        decision_variables: BTreeMap<VariableID, DecisionVariable>,
        parameters: BTreeMap<VariableID, v1::Parameter>,
        constraints: BTreeMap<ConstraintID, Constraint>,
    ) -> anyhow::Result<Self> {
        // Check that decision variable IDs and parameter IDs are disjoint
        let decision_variable_ids: VariableIDSet = decision_variables.keys().cloned().collect();
        let parameter_ids: VariableIDSet = parameters.keys().cloned().collect();

        let intersection: VariableIDSet = decision_variable_ids
            .intersection(&parameter_ids)
            .cloned()
            .collect();
        if !intersection.is_empty() {
            return Err(InstanceError::DuplicatedVariableID {
                id: *intersection.iter().next().unwrap(),
            }
            .into());
        }

        // Combine decision variables and parameters for validation
        let all_variable_ids: VariableIDSet = decision_variable_ids
            .union(&parameter_ids)
            .cloned()
            .collect();

        // Check that all IDs used in objective are defined
        for id in objective.required_ids() {
            if !all_variable_ids.contains(&id) {
                return Err(InstanceError::UndefinedVariableID { id }.into());
            }
        }

        // Check that all IDs used in constraints are defined
        for constraint in constraints.values() {
            for id in constraint.required_ids() {
                if !all_variable_ids.contains(&id) {
                    return Err(InstanceError::UndefinedVariableID { id }.into());
                }
            }
        }

        Ok(ParametricInstance {
            sense,
            objective,
            decision_variables,
            parameters,
            constraints,
            removed_constraints: BTreeMap::new(),
            decision_variable_dependency: AcyclicAssignments::default(),
            constraint_hints: ConstraintHints::default(),
            description: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff,
        constraint::{Constraint, ConstraintID},
        linear, DecisionVariable, VariableID,
    };
    use maplit::btreemap;

    #[test]
    fn test_instance_new_fails_with_undefined_variable_in_objective() {
        // Create decision variables that do not include variable ID 999
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
        };

        // Create objective function that uses undefined variable ID 999
        let objective = (linear!(999) + coeff!(1.0)).into();

        let constraints = BTreeMap::new();

        // This should fail because variable ID 999 is used in objective but not defined
        insta::assert_snapshot!(
            Instance::new(
                Sense::Minimize,
                objective,
                decision_variables,
                constraints,
            )
            .unwrap_err(),
            @r#"Undefined variable ID is used: VariableID(999)"#
        );
    }

    #[test]
    fn test_instance_new_fails_with_undefined_variable_in_constraint() {
        // Create decision variables that do not include variable ID 999
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
        };

        // Create simple objective function using defined variables
        let objective = (linear!(1) + coeff!(1.0)).into();

        // Create constraint that uses undefined variable ID 999
        let constraints = btreemap! {
            ConstraintID::from(1) => Constraint::equal_to_zero(ConstraintID::from(1), (linear!(999) + coeff!(1.0)).into()),
        };

        // This should fail because variable ID 999 is used in constraint but not defined
        insta::assert_snapshot!(
            Instance::new(
                Sense::Minimize,
                objective,
                decision_variables,
                constraints,
            )
            .unwrap_err(),
            @r#"Undefined variable ID is used: VariableID(999)"#
        );
    }

    #[test]
    fn test_parametric_instance_new_succeeds() {
        // Test successful creation with decision variables and parameters in both objective and constraints
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
        };

        let parameters = btreemap! {
            VariableID::from(100) => v1::Parameter { id: 100, name: Some("p1".to_string()), ..Default::default() },
            VariableID::from(101) => v1::Parameter { id: 101, name: Some("p2".to_string()), ..Default::default() },
        };

        // Objective function uses both decision variables and parameters
        let objective = (linear!(1) + linear!(100) + coeff!(1.0)).into();

        // Constraints also use both decision variables and parameters
        let constraints = btreemap! {
            ConstraintID::from(1) => Constraint::equal_to_zero(ConstraintID::from(1), (linear!(2) + linear!(101) + coeff!(1.0)).into()),
            ConstraintID::from(2) => Constraint::less_than_or_equal_to_zero(ConstraintID::from(2), (linear!(1) + linear!(100) + coeff!(2.0)).into()),
        };

        let parametric_instance = ParametricInstance::new(
            Sense::Maximize,
            objective,
            decision_variables,
            parameters,
            constraints,
        )
        .unwrap();

        assert_eq!(parametric_instance.sense, Sense::Maximize);
        assert_eq!(parametric_instance.decision_variables.len(), 2);
        assert_eq!(parametric_instance.parameters.len(), 2);
        assert_eq!(parametric_instance.constraints.len(), 2);
    }

    #[test]
    fn test_parametric_instance_new_fails_with_duplicated_variable_id() {
        // Test detection of ID collision between decision variables and parameters
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
        };

        // Parameter with same ID as decision variable
        let parameters = btreemap! {
            VariableID::from(1) => v1::Parameter { id: 1, name: Some("p1".to_string()), ..Default::default() },
            VariableID::from(100) => v1::Parameter { id: 100, name: Some("p2".to_string()), ..Default::default() },
        };

        let objective = (linear!(1) + coeff!(1.0)).into();
        let constraints = BTreeMap::new();

        insta::assert_snapshot!(
            ParametricInstance::new(
                Sense::Minimize,
                objective,
                decision_variables,
                parameters,
                constraints,
            )
            .unwrap_err(),
            @"Duplicated variable ID is found in definition: VariableID(1)"
        );
    }

    #[test]
    fn test_parametric_instance_new_fails_with_undefined_variable_in_objective() {
        // Test detection of undefined variable ID in objective function
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
        };

        let parameters = btreemap! {
            VariableID::from(100) => v1::Parameter { id: 100, name: Some("p1".to_string()), ..Default::default() },
        };

        // Objective function uses undefined variable ID 999
        let objective = (linear!(999) + coeff!(1.0)).into();

        let constraints = BTreeMap::new();

        insta::assert_snapshot!(
            ParametricInstance::new(
                Sense::Minimize,
                objective,
                decision_variables,
                parameters,
                constraints,
            )
            .unwrap_err(),
            @r#"Undefined variable ID is used: VariableID(999)"#
        );
    }

    #[test]
    fn test_parametric_instance_new_fails_with_undefined_variable_in_constraint() {
        // Test detection of undefined variable ID in constraint
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
        };

        let parameters = btreemap! {
            VariableID::from(100) => v1::Parameter { id: 100, name: Some("p1".to_string()), ..Default::default() },
        };

        let objective = (linear!(1) + coeff!(1.0)).into();

        // Constraint uses undefined variable ID 999
        let constraints = btreemap! {
            ConstraintID::from(1) => Constraint::equal_to_zero(ConstraintID::from(1), (linear!(999) + coeff!(1.0)).into()),
        };

        insta::assert_snapshot!(
            ParametricInstance::new(
                Sense::Minimize,
                objective,
                decision_variables,
                parameters,
                constraints,
            )
            .unwrap_err(),
            @r#"Undefined variable ID is used: VariableID(999)"#
        );
    }
}
