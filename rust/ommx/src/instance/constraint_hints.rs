use super::{parse::*, Instance, ParametricInstance};
use crate::{
    parse::{Parse, ParseError, RawParseError},
    v1::{self, Samples, State},
    Constraint, ConstraintID, DecisionVariable, Evaluate, InstanceError, RemovedConstraint,
    VariableID, VariableIDSet,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneHot {
    pub id: ConstraintID,
    pub variables: BTreeSet<VariableID>,
}

impl Parse for v1::OneHot {
    type Output = OneHot;
    type Context = (
        BTreeMap<VariableID, DecisionVariable>,
        BTreeMap<ConstraintID, Constraint>,
        BTreeMap<ConstraintID, RemovedConstraint>,
    );
    fn parse(
        self,
        (decision_variable, constraints, removed_constraints): &Self::Context,
    ) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.OneHot";
        let constraint_id = as_constraint_id(constraints, removed_constraints, self.constraint_id)
            .map_err(|e| e.context(message, "constraint_id"))?;
        let mut variables = BTreeSet::new();
        for v in &self.decision_variables {
            let id = as_variable_id(decision_variable, *v)
                .map_err(|e| e.context(message, "decision_variables"))?;
            if !variables.insert(id) {
                return Err(
                    RawParseError::InstanceError(InstanceError::NonUniqueVariableID { id })
                        .context(message, "decision_variables"),
                );
            }
        }
        Ok(OneHot {
            id: constraint_id,
            variables,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sos1 {
    pub binary_constraint_id: ConstraintID,
    pub variables: BTreeSet<VariableID>,
    /// Map from variable ID to corresponding big-M constraint ID
    pub variable_to_big_m_constraint: BTreeMap<VariableID, ConstraintID>,
}

impl Sos1 {
    /// Get all big-M constraint IDs
    pub fn big_m_constraint_ids(&self) -> BTreeSet<ConstraintID> {
        self.variable_to_big_m_constraint
            .values()
            .cloned()
            .collect()
    }
}

impl Parse for v1::Sos1 {
    type Output = Sos1;
    type Context = (
        BTreeMap<VariableID, DecisionVariable>,
        BTreeMap<ConstraintID, Constraint>,
        BTreeMap<ConstraintID, RemovedConstraint>,
    );
    fn parse(
        self,
        (decision_variable, constraints, removed_constraints): &Self::Context,
    ) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.Sos1";
        let binary_constraint_id =
            as_constraint_id(constraints, removed_constraints, self.binary_constraint_id)
                .map_err(|e| e.context(message, "binary_constraint_id"))?;

        // Parse big_m_constraint_ids
        let mut parsed_big_m_ids = Vec::new();
        for id in &self.big_m_constraint_ids {
            let id = as_constraint_id(constraints, removed_constraints, *id)
                .map_err(|e| e.context(message, "big_m_constraint_ids"))?;
            parsed_big_m_ids.push(id);
        }

        // Parse variables
        let mut variables = BTreeSet::new();
        let mut parsed_var_ids = Vec::new();
        for id in &self.decision_variables {
            let id = as_variable_id(decision_variable, *id)
                .map_err(|e| e.context(message, "decision_variables"))?;
            if !variables.insert(id) {
                return Err(
                    RawParseError::InstanceError(InstanceError::NonUniqueVariableID { id })
                        .context(message, "decision_variables"),
                );
            }
            parsed_var_ids.push(id);
        }

        // Build variable to big-M constraint map
        // Assumes variables and big_m_constraint_ids have 1:1 correspondence
        let mut variable_to_big_m_constraint = BTreeMap::new();
        if parsed_var_ids.len() == parsed_big_m_ids.len() {
            for (var_id, constraint_id) in
                parsed_var_ids.into_iter().zip(parsed_big_m_ids.into_iter())
            {
                variable_to_big_m_constraint.insert(var_id, constraint_id);
            }
        }
        // If lengths don't match, leave the map empty (backward compatibility)

        Ok(Sos1 {
            binary_constraint_id,
            variables,
            variable_to_big_m_constraint,
        })
    }
}

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

impl From<OneHot> for v1::OneHot {
    fn from(value: OneHot) -> Self {
        Self {
            constraint_id: *value.id,
            decision_variables: value.variables.into_iter().map(|v| *v).collect(),
        }
    }
}

impl From<Sos1> for v1::Sos1 {
    fn from(value: Sos1) -> Self {
        // Reconstruct the original ordering of big_m_constraint_ids and decision_variables
        let mut big_m_constraint_ids = Vec::new();
        let mut decision_variables = Vec::new();

        // We need to maintain the same order, so iterate through variables
        for var_id in &value.variables {
            decision_variables.push(**var_id);
            if let Some(constraint_id) = value.variable_to_big_m_constraint.get(var_id) {
                big_m_constraint_ids.push(**constraint_id);
            }
        }

        Self {
            binary_constraint_id: *value.binary_constraint_id,
            big_m_constraint_ids,
            decision_variables,
        }
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

impl Evaluate for OneHot {
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
        let mut variables_to_remove = Vec::new();
        let mut should_discard = false;

        for &var_id in &self.variables {
            if let Some(&value) = state.entries.get(&var_id.into_inner()) {
                if value.abs() < *atol {
                    // If the value is 0 (within tolerance), remove the variable
                    variables_to_remove.push(var_id);
                } else {
                    // If the value is non-zero, warn and discard the hint
                    log::warn!(
                        "OneHot constraint hint {} has variable {} with non-zero value {}. Discarding the hint.",
                        self.id,
                        var_id,
                        value
                    );
                    should_discard = true;
                    break;
                }
            }
        }

        if should_discard {
            self.variables.clear();
        } else {
            for var in variables_to_remove {
                self.variables.remove(&var);
            }
        }

        Ok(())
    }

    fn required_ids(&self) -> VariableIDSet {
        self.variables.clone()
    }
}

impl Evaluate for Sos1 {
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
        let mut variables_to_remove = Vec::new();
        let mut should_discard = false;

        for &var_id in &self.variables {
            if let Some(&value) = state.entries.get(&var_id.into_inner()) {
                if value.abs() < *atol {
                    // If the value is 0 (within tolerance), remove the variable
                    variables_to_remove.push(var_id);
                } else {
                    // If the value is non-zero, warn and discard the hint
                    log::warn!(
                        "Sos1 constraint hint with binary_constraint_id {} has variable {} with non-zero value {}. Discarding the hint.",
                        self.binary_constraint_id,
                        var_id,
                        value
                    );
                    should_discard = true;
                    break;
                }
            }
        }

        if should_discard {
            self.variables.clear();
            self.variable_to_big_m_constraint.clear();
        } else {
            for var in variables_to_remove {
                self.variables.remove(&var);
                // Remove corresponding big-M constraint from the map
                self.variable_to_big_m_constraint.remove(&var);
            }
        }

        Ok(())
    }

    fn required_ids(&self) -> VariableIDSet {
        self.variables.clone()
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

impl Instance {
    pub fn add_constraint_hints(
        &mut self,
        constraint_hints: ConstraintHints,
    ) -> anyhow::Result<()> {
        // Validate constraint_hints using Parse trait
        let hints: v1::ConstraintHints = constraint_hints.into();
        let context = (
            self.decision_variables.clone(),
            self.constraints.clone(),
            self.removed_constraints.clone(),
        );
        let constraint_hints = hints.parse(&context)?;
        self.constraint_hints = constraint_hints;
        Ok(())
    }

    pub fn with_constraint_hints(
        mut self,
        constraint_hints: ConstraintHints,
    ) -> anyhow::Result<Self> {
        self.add_constraint_hints(constraint_hints)?;
        Ok(self)
    }
}

impl ParametricInstance {
    pub fn add_constraint_hints(
        &mut self,
        constraint_hints: ConstraintHints,
    ) -> anyhow::Result<()> {
        // Validate constraint_hints using Parse trait
        let hints: v1::ConstraintHints = constraint_hints.into();
        let context = (
            self.decision_variables.clone(),
            self.constraints.clone(),
            self.removed_constraints.clone(),
        );
        let constraint_hints = hints.parse(&context)?;
        self.constraint_hints = constraint_hints;
        Ok(())
    }

    pub fn with_constraint_hints(
        mut self,
        constraint_hints: ConstraintHints,
    ) -> anyhow::Result<Self> {
        self.add_constraint_hints(constraint_hints)?;
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff,
        constraint::{Constraint, ConstraintID},
        constraint_hints::{ConstraintHints, OneHot},
        linear, DecisionVariable, Sense, VariableID,
    };
    use maplit::{btreemap, btreeset, hashmap};
    use std::collections::BTreeSet;

    #[test]
    fn test_instance_add_constraint_hints() {
        // Test adding constraint hints to an instance
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
        };

        let objective = (linear!(1) + coeff!(1.0)).into();
        let constraints = btreemap! {
            ConstraintID::from(1) => Constraint::equal_to_zero(ConstraintID::from(1), (linear!(1) + coeff!(1.0)).into()),
        };

        let mut variables = BTreeSet::new();
        variables.insert(VariableID::from(1));
        variables.insert(VariableID::from(2));

        let one_hot = OneHot {
            id: ConstraintID::from(1),
            variables,
        };

        let constraint_hints = ConstraintHints {
            one_hot_constraints: vec![one_hot],
            sos1_constraints: vec![],
        };

        let instance = Instance::new(Sense::Minimize, objective, decision_variables, constraints)
            .unwrap()
            .with_constraint_hints(constraint_hints)
            .unwrap();

        assert_eq!(instance.constraint_hints.one_hot_constraints.len(), 1);
    }

    #[test]
    fn test_parametric_instance_add_constraint_hints() {
        // Test adding constraint hints to a parametric instance
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
        };

        let parameters = btreemap! {
            VariableID::from(100) => v1::Parameter { id: 100, name: Some("p1".to_string()), ..Default::default() },
        };

        let objective = (linear!(1) + coeff!(1.0)).into();
        let constraints = btreemap! {
            ConstraintID::from(1) => Constraint::equal_to_zero(ConstraintID::from(1), (linear!(1) + coeff!(1.0)).into()),
        };

        let mut variables = BTreeSet::new();
        variables.insert(VariableID::from(1));
        variables.insert(VariableID::from(2));

        let one_hot = OneHot {
            id: ConstraintID::from(1),
            variables,
        };

        let constraint_hints = ConstraintHints {
            one_hot_constraints: vec![one_hot],
            sos1_constraints: vec![],
        };

        let parametric_instance = ParametricInstance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            parameters,
            constraints,
        )
        .unwrap()
        .with_constraint_hints(constraint_hints)
        .unwrap();

        assert_eq!(
            parametric_instance
                .constraint_hints
                .one_hot_constraints
                .len(),
            1
        );
    }

    #[test]
    fn test_one_hot_partial_evaluate_remove_zero() {
        // Test that OneHot removes variables with value 0
        let mut one_hot = OneHot {
            id: ConstraintID::from(1),
            variables: btreeset! {
                VariableID::from(1),
                VariableID::from(2),
                VariableID::from(3),
            },
        };

        let state = State {
            entries: hashmap! {
                1 => 0.0,  // Should be removed
                2 => 0.0,  // Should be removed
            },
        };

        one_hot
            .partial_evaluate(&state, crate::ATol::default())
            .unwrap();

        // Only variable 3 should remain
        assert_eq!(one_hot.variables.len(), 1);
        assert!(one_hot.variables.contains(&VariableID::from(3)));
    }

    #[test]
    fn test_one_hot_partial_evaluate_discard_nonzero() {
        // Test that OneHot is discarded when a variable has non-zero value
        let mut one_hot = OneHot {
            id: ConstraintID::from(1),
            variables: btreeset! {
                VariableID::from(1),
                VariableID::from(2),
                VariableID::from(3),
            },
        };

        let state = State {
            entries: hashmap! {
                1 => 0.0,
                2 => 1.0,  // Non-zero value should cause discard
            },
        };

        one_hot
            .partial_evaluate(&state, crate::ATol::default())
            .unwrap();

        // All variables should be cleared
        assert!(one_hot.variables.is_empty());
    }

    #[test]
    fn test_sos1_partial_evaluate_remove_zero() {
        // Test that Sos1 removes variables with value 0 and their corresponding big-M constraints
        let mut sos1 = Sos1 {
            binary_constraint_id: ConstraintID::from(1),
            variables: btreeset! {
                VariableID::from(1),
                VariableID::from(2),
                VariableID::from(3),
            },
            variable_to_big_m_constraint: btreemap! {
                VariableID::from(1) => ConstraintID::from(10),
                VariableID::from(2) => ConstraintID::from(20),
                VariableID::from(3) => ConstraintID::from(30),
            },
        };

        let state = State {
            entries: hashmap! {
                1 => 0.0,  // Should be removed with constraint 10
                2 => 0.0,  // Should be removed with constraint 20
            },
        };

        sos1.partial_evaluate(&state, crate::ATol::default())
            .unwrap();

        // Only variable 3 should remain
        assert_eq!(sos1.variables.len(), 1);
        assert!(sos1.variables.contains(&VariableID::from(3)));

        // Only constraint 30 should remain in the map
        assert_eq!(sos1.variable_to_big_m_constraint.len(), 1);
        assert_eq!(
            sos1.variable_to_big_m_constraint.get(&VariableID::from(3)),
            Some(&ConstraintID::from(30))
        );

        // Check big_m_constraint_ids() method
        let big_m_ids = sos1.big_m_constraint_ids();
        assert_eq!(big_m_ids.len(), 1);
        assert!(big_m_ids.contains(&ConstraintID::from(30)));
    }

    #[test]
    fn test_sos1_partial_evaluate_discard_nonzero() {
        // Test that Sos1 is discarded when a variable has non-zero value
        let mut sos1 = Sos1 {
            binary_constraint_id: ConstraintID::from(1),
            variables: btreeset! {
                VariableID::from(1),
                VariableID::from(2),
                VariableID::from(3),
            },
            variable_to_big_m_constraint: btreemap! {
                VariableID::from(1) => ConstraintID::from(10),
                VariableID::from(2) => ConstraintID::from(20),
                VariableID::from(3) => ConstraintID::from(30),
            },
        };

        let state = State {
            entries: hashmap! {
                1 => 0.0,
                2 => 0.5,  // Non-zero value should cause discard
            },
        };

        sos1.partial_evaluate(&state, crate::ATol::default())
            .unwrap();

        // All fields should be cleared
        assert!(sos1.variables.is_empty());
        assert!(sos1.variable_to_big_m_constraint.is_empty());
        assert!(sos1.big_m_constraint_ids().is_empty());
    }

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
