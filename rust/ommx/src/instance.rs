use crate::{
    parse::{Parse, ParseError, RawParseError},
    v1::{self},
    Constraint, ConstraintID, DecisionVariable, Function, RemovedConstraint, VariableID,
};
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sense {
    Minimize,
    Maximize,
}

impl Parse for v1::instance::Sense {
    type Output = Sense;
    type Context = ();
    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        match self {
            v1::instance::Sense::Minimize => Ok(Sense::Minimize),
            v1::instance::Sense::Maximize => Ok(Sense::Maximize),
            v1::instance::Sense::Unspecified => Err(RawParseError::UnspecifiedEnum {
                enum_name: "ommx.v1.instance.Sense",
            }
            .into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneHot {
    pub id: ConstraintID,
    pub variables: BTreeSet<VariableID>,
}

impl Parse for v1::OneHot {
    type Output = OneHot;
    type Context = (
        HashMap<VariableID, DecisionVariable>,
        HashMap<ConstraintID, Constraint>,
    );
    fn parse(
        self,
        (decision_variable, constraints): &Self::Context,
    ) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.OneHot";
        let constraint_id = as_constraint_id(constraints, self.constraint_id)
            .map_err(|e| e.context(message, "constraint_id"))?;
        let mut variables = BTreeSet::new();
        for v in &self.decision_variables {
            let id = as_variable_id(decision_variable, *v)
                .map_err(|e| e.context(message, "decision_variables"))?;
            if !variables.insert(id) {
                return Err(RawParseError::NonUniqueVariableID { id }
                    .context(message, "decision_variables"));
            }
        }
        Ok(OneHot {
            id: constraint_id,
            variables,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SOS1Constraints {
    pub binary_constraint_id: ConstraintID,
    pub big_m_constraint_ids: BTreeSet<ConstraintID>,
    pub variables: BTreeSet<VariableID>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ConstraintHints {
    pub one_hot_constraints: Vec<OneHot>,
    pub sos1_constraints: Vec<SOS1Constraints>,
}

/// Instance, represents a mathematical optimization problem.
///
/// Invariants
/// -----------
/// - All `VariableID`s in `Function`s contained both directly and indirectly must be keys of `decision_variables`.
/// - Key of `constraints` and `removed_constraints` are disjoint.
/// - The keys of `decision_variable_dependency` are also keys of `decision_variables`.
///
#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    sense: Sense,
    objective: Function,
    decision_variables: HashMap<VariableID, DecisionVariable>,
    constraints: HashMap<ConstraintID, Constraint>,
    removed_constraints: HashMap<ConstraintID, RemovedConstraint>,
    decision_variable_dependency: HashMap<VariableID, Function>,
    parameters: Option<v1::Parameters>,
    description: Option<v1::instance::Description>,
    constraint_hints: ConstraintHints,
}

impl TryFrom<v1::Instance> for Instance {
    type Error = ParseError;
    fn try_from(value: v1::Instance) -> Result<Self, Self::Error> {
        let message = "ommx.v1.Instance";
        let sense = value.sense().parse_as(&(), message, "sense")?;

        let decision_variables =
            value
                .decision_variables
                .parse_as(&(), message, "decision_variables")?;

        let objective = value
            .objective
            .ok_or(RawParseError::MissingField {
                message,
                field: "objective",
            })?
            .parse_as(&(), message, "objective")?;

        let constraints = value.constraints.parse_as(&(), message, "constraints")?;
        let removed_constraints =
            value
                .removed_constraints
                .parse_as(&constraints, message, "removed_constraints")?;

        let as_variable_id = |id: u64| {
            let id = VariableID::from(id);
            if !decision_variables.contains_key(&id) {
                return Err(RawParseError::UndefinedVariableID { id });
            }
            Ok(id)
        };
        let as_constraint_id = |id: u64| {
            let id = ConstraintID::from(id);
            if !constraints.contains_key(&id) {
                return Err(RawParseError::UndefinedConstraintID { id });
            }
            Ok(id)
        };

        let mut decision_variable_dependency = HashMap::new();
        for (id, f) in value.decision_variable_dependency {
            decision_variable_dependency.insert(
                as_variable_id(id)
                    .map_err(|e| e.context(message, "decision_variable_dependency"))?,
                f.parse_as(&(), message, "decision_variable_dependency")?,
            );
        }

        let constraint_hints = if let Some(hints) = value.constraint_hints {
            let mut one_hot_constraints = Vec::new();
            for onehot in hints.one_hot_constraints {
                let constraint_id = as_constraint_id(onehot.constraint_id).map_err(|e| {
                    e.context("ommx.v1.OneHotConstraint", "constraint_id")
                        .context("ommx.v1.ConstraintHints", "one_hot_constraints")
                        .context(message, "constraint_hints")
                })?;
                let mut variables = BTreeSet::new();
                for v in &onehot.decision_variables {
                    let id = as_variable_id(*v)?;
                    if !variables.insert(id) {
                        todo!("One-hot constraint {constraint_id:?} contains duplicated decision variable {id:?}");
                    }
                }
                one_hot_constraints.push(OneHot {
                    id: constraint_id,
                    variables,
                });
            }
            let mut sos1_constraints = Vec::new();
            for sos1 in hints.sos1_constraints {
                let variables = sos1
                    .decision_variables
                    .into_iter()
                    .map(as_variable_id)
                    .collect::<Result<_, RawParseError>>()?;
                let big_m_constraint_ids = sos1
                    .big_m_constraint_ids
                    .into_iter()
                    .map(as_constraint_id)
                    .collect::<Result<_, RawParseError>>()?;
                sos1_constraints.push(SOS1Constraints {
                    binary_constraint_id: as_constraint_id(sos1.binary_constraint_id)?,
                    big_m_constraint_ids,
                    variables,
                });
            }
            ConstraintHints {
                one_hot_constraints,
                sos1_constraints,
            }
        } else {
            ConstraintHints::default()
        };

        Ok(Self {
            sense,
            objective,
            constraints,
            decision_variables,
            removed_constraints,
            decision_variable_dependency,
            parameters: value.parameters,
            description: value.description,
            constraint_hints,
        })
    }
}

fn as_constraint_id(
    constraints: &HashMap<ConstraintID, Constraint>,
    id: u64,
) -> Result<ConstraintID, ParseError> {
    let id = ConstraintID::from(id);
    if !constraints.contains_key(&id) {
        return Err(RawParseError::UndefinedConstraintID { id }.into());
    }
    Ok(id)
}

fn as_variable_id(
    decision_variables: &HashMap<VariableID, DecisionVariable>,
    id: u64,
) -> Result<VariableID, ParseError> {
    let id = VariableID::from(id);
    if !decision_variables.contains_key(&id) {
        return Err(RawParseError::UndefinedVariableID { id }.into());
    }
    Ok(id)
}
