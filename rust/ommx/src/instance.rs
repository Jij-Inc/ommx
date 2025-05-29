mod analysis;
mod approx;
mod arbitrary;
mod constraint_hints;
mod evaluate;
mod parse;
mod pass;

use std::collections::BTreeMap;

pub use analysis::*;
pub use constraint_hints::*;

use crate::{
    v1, Constraint, ConstraintID, DecisionVariable, Function, RemovedConstraint, VariableID,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sense {
    Minimize,
    Maximize,
}

/// Instance, represents a mathematical optimization problem.
///
/// Invariants
/// -----------
/// - All `VariableID`s in `Function`s contained both directly and indirectly must be keys of `decision_variables`.
/// - Key of `constraints` and `removed_constraints` are disjoint.
/// - The keys of `decision_variable_dependency` are also keys of `decision_variables`.
///
#[derive(Debug, Clone, PartialEq, getset::Getters)]
pub struct Instance {
    #[getset(get = "pub")]
    sense: Sense,
    #[getset(get = "pub")]
    objective: Function,
    #[getset(get = "pub")]
    decision_variables: BTreeMap<VariableID, DecisionVariable>,
    #[getset(get = "pub")]
    constraints: BTreeMap<ConstraintID, Constraint>,
    #[getset(get = "pub")]
    removed_constraints: BTreeMap<ConstraintID, RemovedConstraint>,
    #[getset(get = "pub")]
    decision_variable_dependency: BTreeMap<VariableID, Function>,
    #[getset(get = "pub")]
    parameters: Option<v1::Parameters>,
    #[getset(get = "pub")]
    description: Option<v1::instance::Description>,
    #[getset(get = "pub")]
    constraint_hints: ConstraintHints,
}

impl Instance {
    ///
    /// - All `VariableID`s in `Function`s contained both directly and indirectly must be keys of `decision_variables`.
    /// - Key of `constraints` and `removed_constraints` are disjoint.
    /// - The keys of `decision_variable_dependency` are also keys of `decision_variables`.
    ///
    pub fn try_new(
        sense: Sense,
        objective: Function,
        decision_variables: BTreeMap<VariableID, DecisionVariable>,
        constraints: BTreeMap<ConstraintID, Constraint>,
        removed_constraints: BTreeMap<ConstraintID, RemovedConstraint>,
        decision_variable_dependency: BTreeMap<VariableID, Function>,
        parameters: Option<v1::Parameters>,
        description: Option<v1::instance::Description>,
        constraint_hints: ConstraintHints,
    ) -> anyhow::Result<Self> {
        let instance = Instance {
            sense,
            objective,
            decision_variables,
            constraints,
            removed_constraints,
            decision_variable_dependency,
            parameters,
            description,
            constraint_hints,
        };
        
        instance.validate_all_invariants()?;
        
        Ok(instance)
    }
    
    fn validate_all_invariants(&self) -> anyhow::Result<()> {
        let v1_instance: crate::v1::Instance = self.clone().into();
        v1_instance.validate()?;
        
        for &dep_var_id in self.decision_variable_dependency.keys() {
            if !self.decision_variables.contains_key(&dep_var_id) {
                anyhow::bail!("Decision variable dependency key {} is not defined in decision_variables", dep_var_id);
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ATol, Bound, Equality, Kind};
    use std::collections::BTreeMap;

    fn create_valid_decision_variables() -> BTreeMap<VariableID, DecisionVariable> {
        let mut vars = BTreeMap::new();
        let var1 = DecisionVariable::new(
            VariableID::from(1),
            Kind::Continuous,
            Bound::new(-1.0, 1.0).unwrap(),
            None,
            ATol::default(),
        ).unwrap();
        let var2 = DecisionVariable::new(
            VariableID::from(2),
            Kind::Continuous,
            Bound::new(-1.0, 1.0).unwrap(),
            None,
            ATol::default(),
        ).unwrap();
        vars.insert(VariableID::from(1), var1);
        vars.insert(VariableID::from(2), var2);
        vars
    }

    fn create_valid_constraints() -> BTreeMap<ConstraintID, Constraint> {
        let mut constraints = BTreeMap::new();
        let constraint = Constraint {
            id: ConstraintID::from(1),
            function: Function::Zero,
            equality: Equality::EqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: Default::default(),
            description: None,
        };
        constraints.insert(ConstraintID::from(1), constraint);
        constraints
    }

    #[test]
    fn test_try_new_valid_instance() {
        let decision_variables = create_valid_decision_variables();
        let constraints = create_valid_constraints();
        let removed_constraints = BTreeMap::new();
        let decision_variable_dependency = BTreeMap::new();
        
        let result = Instance::try_new(
            Sense::Minimize,
            Function::Zero,
            decision_variables,
            constraints,
            removed_constraints,
            decision_variable_dependency,
            None,
            None,
            ConstraintHints::default(),
        );
        
        assert!(result.is_ok(), "Valid instance should be created successfully");
    }

    #[test]
    fn test_try_new_invalid_decision_variable_dependency() {
        let decision_variables = create_valid_decision_variables();
        let constraints = create_valid_constraints();
        let removed_constraints = BTreeMap::new();
        
        let mut decision_variable_dependency = BTreeMap::new();
        decision_variable_dependency.insert(VariableID::from(999), Function::Zero);
        
        let result = Instance::try_new(
            Sense::Minimize,
            Function::Zero,
            decision_variables,
            constraints,
            removed_constraints,
            decision_variable_dependency,
            None,
            None,
            ConstraintHints::default(),
        );
        
        assert!(result.is_err(), "Instance with invalid dependency should fail");
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Decision variable dependency key 999 is not defined"));
    }

    #[test]
    fn test_try_new_duplicate_constraint_ids() {
        let decision_variables = create_valid_decision_variables();
        let constraints = create_valid_constraints();
        
        let mut removed_constraints = BTreeMap::new();
        let constraint_id = ConstraintID::from(1);
        let removed_constraint = RemovedConstraint {
            constraint: Constraint {
                id: constraint_id,
                function: Function::Zero,
                equality: Equality::EqualToZero,
                name: None,
                subscripts: Vec::new(),
                parameters: Default::default(),
                description: None,
            },
            removed_reason: "Test".to_string(),
            removed_reason_parameters: Default::default(),
        };
        removed_constraints.insert(constraint_id, removed_constraint);
        
        let result = Instance::try_new(
            Sense::Minimize,
            Function::Zero,
            decision_variables,
            constraints,
            removed_constraints,
            BTreeMap::new(),
            None,
            None,
            ConstraintHints::default(),
        );
        
        assert!(result.is_err(), "Instance with duplicate constraint IDs should fail");
    }

    #[test]
    fn test_try_new_valid_decision_variable_dependency() {
        let decision_variables = create_valid_decision_variables();
        let constraints = create_valid_constraints();
        let removed_constraints = BTreeMap::new();
        
        let mut decision_variable_dependency = BTreeMap::new();
        decision_variable_dependency.insert(VariableID::from(1), Function::Zero);
        
        let result = Instance::try_new(
            Sense::Minimize,
            Function::Zero,
            decision_variables,
            constraints,
            removed_constraints,
            decision_variable_dependency,
            None,
            None,
            ConstraintHints::default(),
        );
        
        assert!(result.is_ok(), "Instance with valid dependency should succeed");
    }
}
