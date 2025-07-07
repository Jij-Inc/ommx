mod analysis;
mod approx;
mod arbitrary;
mod constraint_hints;
mod decision_variable;
mod error;
mod evaluate;
mod log_encode;
mod parse;
mod pass;
mod reduce_binary_power;
mod serialize;
mod substitute;

use std::{collections::BTreeMap, ops::Neg};

pub use analysis::*;
pub use constraint_hints::*;
pub use error::*;
pub use log_encode::*;

use crate::{
    parse::Parse, v1, AcyclicAssignments, Constraint, ConstraintID, DecisionVariable, Evaluate,
    Function, RemovedConstraint, VariableID, VariableIDSet,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Sense {
    #[default]
    Minimize,
    Maximize,
}

/// Instance, represents a mathematical optimization problem.
///
/// Invariants
/// -----------
/// - [`Self::decision_variables`] contains all decision variables used in the problem.
/// - The keys of [`Self::constraints`] and [`Self::removed_constraints`] are disjoint sets.
/// - The keys of [`Self::decision_variable_dependency`] are not used. See also the document of [`DecisionVariableAnalysis`].
///
#[derive(Debug, Clone, PartialEq, getset::Getters, Default)]
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
    decision_variable_dependency: AcyclicAssignments,

    /// The constraint hints, i.e. some constraints are in form of one-hot, SOS1,2, or other special types.
    ///
    /// Note
    /// -----
    /// This struct does not validate the hints in mathematical sense.
    /// Only checks the decision variable and constraint IDs are valid.
    #[getset(get = "pub")]
    constraint_hints: ConstraintHints,

    // Optional fields for additional metadata.
    // These fields are public since arbitrary values can be set without validation.
    pub parameters: Option<v1::Parameters>,
    pub description: Option<v1::instance::Description>,
}

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

    /// Set the objective function
    pub fn set_objective(&mut self, objective: Function) -> anyhow::Result<()> {
        // Validate that all variables in the objective are defined
        let variable_ids: VariableIDSet = self.decision_variables.keys().cloned().collect();
        let required_ids = objective.required_ids();
        if !required_ids.is_subset(&variable_ids) {
            let undefined_id = required_ids.difference(&variable_ids).next().unwrap();
            return Err(InstanceError::UndefinedVariableID { id: *undefined_id }.into());
        }
        self.objective = objective;
        Ok(())
    }

    /// Insert a constraint into the instance. If the constraint already exists, it will be replaced.
    pub fn insert_constraint(
        &mut self,
        constraint: Constraint,
    ) -> anyhow::Result<Option<Constraint>> {
        // Validate that all variables in the constraints are defined
        let variable_ids: VariableIDSet = self.decision_variables.keys().cloned().collect();
        let required_ids = constraint.required_ids();
        if !required_ids.is_subset(&variable_ids) {
            let undefined_id = required_ids.difference(&variable_ids).next().unwrap();
            return Err(InstanceError::UndefinedVariableID { id: *undefined_id }.into());
        }
        Ok(self.constraints.insert(constraint.id, constraint))
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff,
        constraint::{Constraint, ConstraintID, Equality},
        polynomial_base::{Linear, LinearMonomial},
        Coefficient, DecisionVariable, Function, VariableID,
    };
    use fnv::FnvHashMap;
    use std::collections::BTreeSet;

    /// Helper function to create a simple constraint
    fn create_constraint(id: u64, variable_id: u64) -> Constraint {
        let linear = Linear::single_term(LinearMonomial::Variable(variable_id.into()), coeff!(1.0));
        Constraint {
            id: ConstraintID::from(id),
            function: Function::Linear(linear),
            equality: Equality::EqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: FnvHashMap::default(),
            description: None,
        }
    }

    #[test]
    fn test_instance_new_fails_with_undefined_variable_in_objective() {
        // Create decision variables that do not include variable ID 999
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(
            VariableID::from(1),
            DecisionVariable::binary(VariableID::from(1)),
        );
        decision_variables.insert(
            VariableID::from(2),
            DecisionVariable::binary(VariableID::from(2)),
        );

        // Create objective function that uses undefined variable ID 999
        let linear = Linear::single_term(
            LinearMonomial::Variable(VariableID::from(999)),
            Coefficient::try_from(1.0).unwrap(),
        );
        let objective = Function::Linear(linear);

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
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(
            VariableID::from(1),
            DecisionVariable::binary(VariableID::from(1)),
        );
        decision_variables.insert(
            VariableID::from(2),
            DecisionVariable::binary(VariableID::from(2)),
        );

        // Create simple objective function using defined variables
        let linear = Linear::single_term(
            LinearMonomial::Variable(VariableID::from(1)),
            Coefficient::try_from(1.0).unwrap(),
        );
        let objective = Function::Linear(linear);

        // Create constraint that uses undefined variable ID 999
        let mut constraints = BTreeMap::new();
        constraints.insert(ConstraintID::from(1), create_constraint(1, 999));

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
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(
            VariableID::from(1),
            DecisionVariable::binary(VariableID::from(1)),
        );
        decision_variables.insert(
            VariableID::from(2),
            DecisionVariable::binary(VariableID::from(2)),
        );

        // Create simple objective function using defined variables
        let linear = Linear::single_term(
            LinearMonomial::Variable(VariableID::from(1)),
            Coefficient::try_from(1.0).unwrap(),
        );
        let objective = Function::Linear(linear);

        // Create constraint using defined variables
        let mut constraints = BTreeMap::new();
        constraints.insert(ConstraintID::from(1), create_constraint(1, 1));

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
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(
            VariableID::from(1),
            DecisionVariable::binary(VariableID::from(1)),
        );
        decision_variables.insert(
            VariableID::from(2),
            DecisionVariable::binary(VariableID::from(2)),
        );

        // Create simple objective function using defined variables
        let linear = Linear::single_term(
            LinearMonomial::Variable(VariableID::from(1)),
            Coefficient::try_from(1.0).unwrap(),
        );
        let objective = Function::Linear(linear);

        // Create constraint with ID 1
        let mut constraints = BTreeMap::new();
        constraints.insert(ConstraintID::from(1), create_constraint(1, 1));

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
