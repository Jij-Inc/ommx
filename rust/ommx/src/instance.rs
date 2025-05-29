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
    ///
    pub fn new(
        sense: Sense,
        objective: Function,
        decision_variables: BTreeMap<VariableID, DecisionVariable>,
        constraints: BTreeMap<ConstraintID, Constraint>,
        removed_constraints: BTreeMap<ConstraintID, RemovedConstraint>,
        constraint_hints: ConstraintHints,
    ) -> anyhow::Result<Self> {
        let instance = Instance {
            sense,
            objective,
            decision_variables,
            constraints,
            removed_constraints,
            decision_variable_dependency: BTreeMap::new(),
            parameters: None,
            description: None,
            constraint_hints,
        };

        instance.validate()?;

        Ok(instance)
    }

    ///
    /// - All `VariableID`s in `Function`s contained both directly and indirectly must be keys of `decision_variables`.
    /// - Key of `constraints` and `removed_constraints` are disjoint.
    /// - The keys of `decision_variable_dependency` are also keys of `decision_variables`.
    ///
    pub fn validate(&self) -> anyhow::Result<()> {
        self.validate_decision_variable_ids()?;
        self.validate_constraint_ids()?;
        self.validate_decision_variable_dependency_keys()?;

        Ok(())
    }

    fn validate_decision_variable_ids(&self) -> anyhow::Result<()> {
        let used_ids = self.required_ids();
        let defined_ids: std::collections::HashSet<_> = 
            self.decision_variables.keys().cloned().collect();

        if !used_ids.is_subset(&defined_ids) {
            let undefined_ids: Vec<_> = used_ids.difference(&defined_ids).collect();
            anyhow::bail!("Undefined decision variable IDs: {:?}", undefined_ids);
        }
        Ok(())
    }

    fn validate_constraint_ids(&self) -> anyhow::Result<()> {
        let mut map = std::collections::HashSet::new();

        for &constraint_id in self.constraints.keys() {
            if !map.insert(constraint_id) {
                anyhow::bail!("Duplicated constraint ID: {:?}", constraint_id);
            }
        }

        for &constraint_id in self.removed_constraints.keys() {
            if !map.insert(constraint_id) {
                anyhow::bail!("Duplicated constraint ID: {:?}", constraint_id);
            }
        }
        Ok(())
    }

    fn validate_decision_variable_dependency_keys(&self) -> anyhow::Result<()> {
        for &dep_var_id in self.decision_variable_dependency.keys() {
            if !self.decision_variables.contains_key(&dep_var_id) {
                anyhow::bail!(
                    "Decision variable dependency key {} is not defined in decision_variables",
                    dep_var_id
                );
            }
        }
        Ok(())
    }

    fn required_ids(&self) -> std::collections::HashSet<VariableID> {
        use crate::Evaluate;
        let mut used_ids = std::collections::HashSet::new();
        used_ids.extend(self.objective.required_ids());

        for constraint in self.constraints.values() {
            used_ids.extend(constraint.function.required_ids());
        }

        for removed_constraint in self.removed_constraints.values() {
            used_ids.extend(removed_constraint.constraint.function.required_ids());
        }

        for function in self.decision_variable_dependency.values() {
            used_ids.extend(function.required_ids());
        }

        used_ids
    }

    pub fn minimize() -> Self {
        Self {
            sense: Sense::Minimize,
            objective: Function::Zero,
            decision_variables: BTreeMap::new(),
            constraints: BTreeMap::new(),
            removed_constraints: BTreeMap::new(),
            decision_variable_dependency: BTreeMap::new(),
            parameters: None,
            description: None,
            constraint_hints: ConstraintHints::default(),
        }
    }

    pub fn maximize() -> Self {
        Self {
            sense: Sense::Maximize,
            objective: Function::Zero,
            decision_variables: BTreeMap::new(),
            constraints: BTreeMap::new(),
            removed_constraints: BTreeMap::new(),
            decision_variable_dependency: BTreeMap::new(),
            parameters: None,
            description: None,
            constraint_hints: ConstraintHints::default(),
        }
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
        )
        .unwrap();
        let var2 = DecisionVariable::new(
            VariableID::from(2),
            Kind::Continuous,
            Bound::new(-1.0, 1.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();
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
    fn test_new_valid_instance() {
        let decision_variables = create_valid_decision_variables();
        let constraints = create_valid_constraints();
        let removed_constraints = BTreeMap::new();

        let result = Instance::new(
            Sense::Minimize,
            Function::Zero,
            decision_variables,
            constraints,
            removed_constraints,
            ConstraintHints::default(),
        );

        assert!(
            result.is_ok(),
            "Valid instance should be created successfully"
        );
    }

    #[test]
    fn test_new_duplicate_constraint_ids() {
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

        let result = Instance::new(
            Sense::Minimize,
            Function::Zero,
            decision_variables,
            constraints,
            removed_constraints,
            ConstraintHints::default(),
        );

        assert!(
            result.is_err(),
            "Instance with duplicate constraint IDs should fail"
        );
    }
}
