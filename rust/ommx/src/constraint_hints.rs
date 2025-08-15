mod one_hot;
mod sos1;

pub use one_hot::OneHot;
pub use sos1::Sos1;

use crate::{
    parse::{Parse, ParseError},
    v1::{self, State},
    Constraint, ConstraintID, ConstraintIDSet, DecisionVariable, RemovedConstraint, VariableID, VariableIDSet,
};
use std::collections::BTreeMap;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ConstraintHints {
    pub one_hot_constraints: Vec<OneHot>,
    pub sos1_constraints: Vec<Sos1>,
}

impl ConstraintHints {
    pub fn is_empty(&self) -> bool {
        self.one_hot_constraints.is_empty() && self.sos1_constraints.is_empty()
    }
}

impl Parse for v1::ConstraintHints {
    type Output = ConstraintHints;
    type Context = (
        BTreeMap<VariableID, DecisionVariable>,
        BTreeMap<ConstraintID, Constraint>,
        BTreeMap<ConstraintID, RemovedConstraint>,
    );
    fn parse(self, context: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.ConstraintHints";
        let one_hot_constraints = self
            .one_hot_constraints
            .into_iter()
            .map(|c| c.parse_as(context, message, "one_hot_constraints"))
            .collect::<Result<Vec<_>, ParseError>>()?;
        let sos1_constraints = self
            .sos1_constraints
            .into_iter()
            .map(|c| c.parse_as(context, message, "sos1_constraints"))
            .collect::<Result<_, ParseError>>()?;
        Ok(ConstraintHints {
            one_hot_constraints,
            sos1_constraints,
        })
    }
}

impl From<ConstraintHints> for v1::ConstraintHints {
    fn from(value: ConstraintHints) -> Self {
        Self {
            one_hot_constraints: value
                .one_hot_constraints
                .into_iter()
                .map(|oh| oh.into())
                .collect(),
            sos1_constraints: value
                .sos1_constraints
                .into_iter()
                .map(|s| s.into())
                .collect(),
        }
    }
}

impl ConstraintHints {
    /// Apply partial evaluation to all constraint hints
    pub fn partial_evaluate(&mut self, state: &State, atol: crate::ATol) {
        // Apply partial evaluation to each OneHot constraint and keep only the valid ones
        self.one_hot_constraints = self
            .one_hot_constraints
            .drain(..)
            .filter_map(|one_hot| one_hot.partial_evaluate(state, atol))
            .collect();

        // Apply partial evaluation to each Sos1 constraint and keep only the valid ones
        self.sos1_constraints = self
            .sos1_constraints
            .drain(..)
            .filter_map(|sos1| sos1.partial_evaluate(state, atol))
            .collect();
    }

    /// Get all decision variable IDs used by constraint hints
    pub fn used_decision_variable_ids(&self) -> VariableIDSet {
        let mut ids = VariableIDSet::new();

        for one_hot in &self.one_hot_constraints {
            ids.extend(one_hot.used_decision_variable_ids());
        }

        for sos1 in &self.sos1_constraints {
            ids.extend(sos1.used_decision_variable_ids());
        }

        ids
    }

    /// Get all constraint IDs used by constraint hints
    pub fn used_constraint_ids(&self) -> ConstraintIDSet {
        let mut ids = ConstraintIDSet::new();

        for one_hot in &self.one_hot_constraints {
            ids.extend(one_hot.used_constraint_ids());
        }

        for sos1 in &self.sos1_constraints {
            ids.extend(sos1.used_constraint_ids());
        }

        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::State;
    use maplit::{btreemap, btreeset, hashmap};

    #[test]
    fn test_constraint_hints_partial_evaluate() {
        // Test ConstraintHints partial evaluation
        let mut constraint_hints = ConstraintHints {
            one_hot_constraints: vec![
                OneHot {
                    id: ConstraintID::from(1),
                    variables: btreeset! {
                        VariableID::from(1),
                        VariableID::from(2),
                    },
                },
                OneHot {
                    id: ConstraintID::from(2),
                    variables: btreeset! {
                        VariableID::from(3),
                        VariableID::from(4),
                    },
                },
            ],
            sos1_constraints: vec![Sos1 {
                binary_constraint_id: ConstraintID::from(3),
                variables: btreeset! {
                    VariableID::from(5),
                    VariableID::from(6),
                    VariableID::from(7),
                },
                variable_to_big_m_constraint: btreemap! {
                    VariableID::from(5) => ConstraintID::from(50),
                    VariableID::from(6) => ConstraintID::from(60),
                    VariableID::from(7) => ConstraintID::from(70),
                },
            }],
        };

        let state = State {
            entries: hashmap! {
                1 => 0.0,  // Remove from first OneHot
                3 => 1.0,  // Discard second OneHot
                5 => 0.0,  // Remove from Sos1
            },
        };

        constraint_hints
            .partial_evaluate(&state, crate::ATol::default());

        // First OneHot should have one variable, second should be removed
        assert_eq!(constraint_hints.one_hot_constraints.len(), 1);
        assert_eq!(constraint_hints.one_hot_constraints[0].variables.len(), 1);
        assert!(constraint_hints.one_hot_constraints[0]
            .variables
            .contains(&VariableID::from(2)));

        // Sos1 should have two variables remaining
        assert_eq!(constraint_hints.sos1_constraints.len(), 1);
        assert_eq!(constraint_hints.sos1_constraints[0].variables.len(), 2);
        assert!(constraint_hints.sos1_constraints[0]
            .variables
            .contains(&VariableID::from(6)));
        assert!(constraint_hints.sos1_constraints[0]
            .variables
            .contains(&VariableID::from(7)));
    }

    #[test]
    fn test_constraint_hints_used_ids() {
        // Test that used_decision_variable_ids and used_constraint_ids return correct IDs
        let constraint_hints = ConstraintHints {
            one_hot_constraints: vec![OneHot {
                id: ConstraintID::from(1),
                variables: btreeset! {
                    VariableID::from(1),
                    VariableID::from(2),
                },
            }],
            sos1_constraints: vec![Sos1 {
                binary_constraint_id: ConstraintID::from(2),
                variables: btreeset! {
                    VariableID::from(3),
                    VariableID::from(4),
                },
                variable_to_big_m_constraint: btreemap! {
                    VariableID::from(3) => ConstraintID::from(30),
                    VariableID::from(4) => ConstraintID::from(40),
                },
            }],
        };

        // Test decision variable IDs
        let decision_var_ids = constraint_hints.used_decision_variable_ids();
        assert_eq!(decision_var_ids.len(), 4);
        assert!(decision_var_ids.contains(&VariableID::from(1)));
        assert!(decision_var_ids.contains(&VariableID::from(2)));
        assert!(decision_var_ids.contains(&VariableID::from(3)));
        assert!(decision_var_ids.contains(&VariableID::from(4)));

        // Test constraint IDs
        let constraint_ids = constraint_hints.used_constraint_ids();
        assert_eq!(constraint_ids.len(), 4);
        assert!(constraint_ids.contains(&ConstraintID::from(1))); // OneHot
        assert!(constraint_ids.contains(&ConstraintID::from(2))); // Sos1 binary
        assert!(constraint_ids.contains(&ConstraintID::from(30))); // Sos1 big-M
        assert!(constraint_ids.contains(&ConstraintID::from(40))); // Sos1 big-M
    }
}
