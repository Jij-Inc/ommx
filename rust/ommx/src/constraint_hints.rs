mod one_hot;
mod sos1;

pub use one_hot::OneHot;
pub use sos1::Sos1;

use crate::{
    parse::{Parse, ParseError},
    v1::{self, Samples, State},
    Constraint, ConstraintID, DecisionVariable, Evaluate, RemovedConstraint, VariableID,
    VariableIDSet,
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

impl Evaluate for ConstraintHints {
    type Output = ();
    type SampledOutput = ();

    fn evaluate(&self, _state: &State, _atol: crate::ATol) -> anyhow::Result<Self::Output> {
        Ok(())
    }

    fn evaluate_samples(
        &self,
        _samples: &Samples,
        _atol: crate::ATol,
    ) -> anyhow::Result<Self::SampledOutput> {
        Ok(())
    }

    fn partial_evaluate(&mut self, state: &State, atol: crate::ATol) -> anyhow::Result<()> {
        // Partially evaluate each OneHot constraint
        for one_hot in &mut self.one_hot_constraints {
            one_hot.partial_evaluate(state, atol)?;
        }

        // Remove empty OneHot constraints
        self.one_hot_constraints
            .retain(|oh| !oh.variables.is_empty());

        // Partially evaluate each Sos1 constraint
        for sos1 in &mut self.sos1_constraints {
            sos1.partial_evaluate(state, atol)?;
        }

        // Remove empty Sos1 constraints
        self.sos1_constraints.retain(|s| !s.variables.is_empty());

        Ok(())
    }

    fn required_ids(&self) -> VariableIDSet {
        let mut ids = VariableIDSet::new();

        for one_hot in &self.one_hot_constraints {
            ids.extend(one_hot.required_ids());
        }

        for sos1 in &self.sos1_constraints {
            ids.extend(sos1.required_ids());
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
            .partial_evaluate(&state, crate::ATol::default())
            .unwrap();

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
    fn test_constraint_hints_required_ids() {
        // Test that required_ids returns all variable IDs
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

        let required = constraint_hints.required_ids();

        assert_eq!(required.len(), 4);
        assert!(required.contains(&VariableID::from(1)));
        assert!(required.contains(&VariableID::from(2)));
        assert!(required.contains(&VariableID::from(3)));
        assert!(required.contains(&VariableID::from(4)));
    }
}
