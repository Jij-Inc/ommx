use super::*;
use crate::{
    parse::Parse, v1, AcyclicAssignments, Constraint, ConstraintID, DecisionVariable, Evaluate,
    Function, VariableID, VariableIDSet,
};
use std::collections::BTreeMap;

impl Instance {
    pub fn new(
        sense: Sense,
        objective: Function,
        decision_variables: BTreeMap<VariableID, DecisionVariable>,
        constraints: BTreeMap<ConstraintID, Constraint>,
        constraint_hints: ConstraintHints,
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

        // Validate constraint_hints using Parse trait
        let hints: v1::ConstraintHints = constraint_hints.into();
        let context = (decision_variables, constraints, BTreeMap::new());
        let constraint_hints = hints.parse(&context)?;

        Ok(Instance {
            sense,
            objective,
            decision_variables: context.0,
            constraints: context.1,
            removed_constraints: BTreeMap::new(),
            decision_variable_dependency: AcyclicAssignments::default(),
            constraint_hints,
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
        constraint_hints: ConstraintHints,
    ) -> anyhow::Result<Self> {
        // Check that decision variable IDs and parameter IDs are disjoint
        let decision_variable_ids: VariableIDSet = decision_variables.keys().cloned().collect();
        let parameter_ids: VariableIDSet = parameters.keys().cloned().collect();
        
        let intersection: VariableIDSet = decision_variable_ids.intersection(&parameter_ids).cloned().collect();
        if !intersection.is_empty() {
            return Err(InstanceError::DuplicatedVariableID { 
                id: *intersection.iter().next().unwrap() 
            }.into());
        }

        // Combine decision variables and parameters for validation
        let all_variable_ids: VariableIDSet = decision_variable_ids.union(&parameter_ids).cloned().collect();
        
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

        // Validate constraint_hints using Parse trait
        let hints: v1::ConstraintHints = constraint_hints.into();
        let context = (decision_variables, constraints, BTreeMap::new());
        let constraint_hints = hints.parse(&context)?;

        Ok(ParametricInstance {
            sense,
            objective,
            decision_variables: context.0,
            parameters,
            constraints: context.1,
            removed_constraints: BTreeMap::new(),
            decision_variable_dependency: AcyclicAssignments::default(),
            constraint_hints,
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
    use std::collections::BTreeSet;

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
        let constraint_hints = ConstraintHints::default();

        // This should fail because variable ID 999 is used in objective but not defined
        insta::assert_snapshot!(
            Instance::new(
                Sense::Minimize,
                objective,
                decision_variables,
                constraints,
                constraint_hints,
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

        let constraint_hints = ConstraintHints::default();

        // This should fail because variable ID 999 is used in constraint but not defined
        insta::assert_snapshot!(
            Instance::new(
                Sense::Minimize,
                objective,
                decision_variables,
                constraints,
                constraint_hints,
            )
            .unwrap_err(),
            @r#"Undefined variable ID is used: VariableID(999)"#
        );
    }

    #[test]
    fn test_instance_new_fails_with_undefined_variable_in_constraint_hints() {
        // Create decision variables that do not include variable ID 999
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
        };

        // Create simple objective function using defined variables
        let objective = (linear!(1) + coeff!(1.0)).into();

        // Create constraint using defined variables
        let constraints = btreemap! {
            ConstraintID::from(1) => Constraint::equal_to_zero(ConstraintID::from(1), (linear!(1) + coeff!(1.0)).into()),
        };

        // Create constraint hints with OneHot that references undefined variable ID 999
        let mut variables = BTreeSet::new();
        variables.insert(VariableID::from(1));
        variables.insert(VariableID::from(999)); // undefined variable

        let one_hot = OneHot {
            id: ConstraintID::from(1),
            variables,
        };

        let constraint_hints = ConstraintHints {
            one_hot_constraints: vec![one_hot],
            sos1_constraints: vec![],
        };

        insta::assert_snapshot!(
            Instance::new(
                Sense::Minimize,
                objective,
                decision_variables,
                constraints,
                constraint_hints,
            )
            .unwrap_err(),
            @r###"
            Traceback for OMMX Message parse error:
            └─ommx.v1.ConstraintHints[one_hot_constraints]
              └─ommx.v1.OneHot[decision_variables]
            Undefined variable ID is used: VariableID(999)
            "###
        );
    }

    #[test]
    fn test_instance_new_fails_with_undefined_constraint_in_constraint_hints() {
        // Create decision variables
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
        };

        // Create simple objective function using defined variables
        let objective = (linear!(1) + coeff!(1.0)).into();

        // Create constraint with ID 1
        let constraints = btreemap! {
            ConstraintID::from(1) => Constraint::equal_to_zero(ConstraintID::from(1), (linear!(1) + coeff!(1.0)).into()),
        };

        // Create constraint hints with OneHot that references undefined constraint ID 999
        let mut variables = BTreeSet::new();
        variables.insert(VariableID::from(1));
        variables.insert(VariableID::from(2));

        let one_hot = OneHot {
            id: ConstraintID::from(999), // undefined constraint ID
            variables,
        };

        let constraint_hints = ConstraintHints {
            one_hot_constraints: vec![one_hot],
            sos1_constraints: vec![],
        };

        insta::assert_snapshot!(
            Instance::new(
                Sense::Minimize,
                objective,
                decision_variables,
                constraints,
                constraint_hints,
            )
            .unwrap_err(),
            @r###"
            Traceback for OMMX Message parse error:
            └─ommx.v1.ConstraintHints[one_hot_constraints]
              └─ommx.v1.OneHot[constraint_id]
            Undefined constraint ID is used: ConstraintID(999)
            "###
        );
    }
}
