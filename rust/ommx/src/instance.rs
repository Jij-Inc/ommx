use crate::{
    parse::{Parse, ParseError, RawParseError},
    v1::{self},
    Constraint, ConstraintID, DecisionVariable, Function, RemovedConstraint, VariableID,
};
use std::collections::{BTreeSet, HashMap, HashSet};

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
pub struct Sos1 {
    pub binary_constraint_id: ConstraintID,
    pub big_m_constraint_ids: BTreeSet<ConstraintID>,
    pub variables: BTreeSet<VariableID>,
}

impl Parse for v1::Sos1 {
    type Output = Sos1;
    type Context = (
        HashMap<VariableID, DecisionVariable>,
        HashMap<ConstraintID, Constraint>,
    );
    fn parse(
        self,
        (decision_variable, constraints): &Self::Context,
    ) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.Sos1";
        let binary_constraint_id = as_constraint_id(constraints, self.binary_constraint_id)
            .map_err(|e| e.context(message, "binary_constraint_id"))?;
        let mut big_m_constraint_ids = BTreeSet::new();
        for id in &self.big_m_constraint_ids {
            let id = as_constraint_id(constraints, *id)
                .map_err(|e| e.context(message, "big_m_constraint_ids"))?;
            if !big_m_constraint_ids.insert(id) {
                return Err(RawParseError::NonUniqueConstraintID { id }
                    .context(message, "big_m_constraint_ids"));
            }
        }
        let mut variables = BTreeSet::new();
        for id in &self.decision_variables {
            let id = as_variable_id(decision_variable, *id)
                .map_err(|e| e.context(message, "decision_variables"))?;
            if !variables.insert(id) {
                return Err(RawParseError::NonUniqueVariableID { id }
                    .context(message, "decision_variables"));
            }
        }
        Ok(Sos1 {
            binary_constraint_id,
            big_m_constraint_ids,
            variables,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KHot {
    pub id: ConstraintID,
    pub variables: BTreeSet<VariableID>,
    pub num_hot_vars: u64,
}

impl Parse for v1::KHot {
    type Output = KHot;
    type Context = (
        HashMap<VariableID, DecisionVariable>,
        HashMap<ConstraintID, Constraint>,
    );
    fn parse(
        self,
        (decision_variable, constraints): &Self::Context,
    ) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.KHot";
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
        Ok(KHot {
            id: constraint_id,
            variables,
            num_hot_vars: self.num_hot_vars,
        })
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ConstraintHints {
    pub one_hot_constraints: Vec<OneHot>,
    pub sos1_constraints: Vec<Sos1>,
    pub k_hot_constraints: HashMap<u64, Vec<KHot>>,
}

impl Parse for v1::ConstraintHints {
    type Output = ConstraintHints;
    type Context = (
        HashMap<VariableID, DecisionVariable>,
        HashMap<ConstraintID, Constraint>,
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

        let mut k_hot_constraints = HashMap::new();
        for (k, k_hot_list) in self.k_hot_constraints {
            let constraints = k_hot_list
                .constraints
                .into_iter()
                .map(|c| c.parse_as(context, message, "k_hot_constraints"))
                .collect::<Result<Vec<_>, ParseError>>()?;
            if !constraints.is_empty() {
                k_hot_constraints.insert(k, constraints);
            }
        }

        Ok(ConstraintHints {
            one_hot_constraints,
            sos1_constraints,
            k_hot_constraints,
        })
    }
}

impl ConstraintHints {
    pub fn one_hot_constraints(&self) -> Vec<OneHot> {
        let mut result = self.one_hot_constraints.clone();
        let mut constraint_ids: HashSet<ConstraintID> = result.iter().map(|c| c.id).collect();

        if let Some(k_hot_list) = self.k_hot_constraints.get(&1) {
            for k_hot in k_hot_list {
                if !constraint_ids.contains(&k_hot.id) {
                    constraint_ids.insert(k_hot.id);
                    result.push(OneHot {
                        id: k_hot.id,
                        variables: k_hot.variables.clone(),
                    });
                }
            }
        }

        result
    }

    pub fn k_hot_constraints(&self) -> HashMap<u64, Vec<KHot>> {
        let mut result: HashMap<u64, Vec<KHot>> = HashMap::new();

        for (k, k_hot_list) in &self.k_hot_constraints {
            result.insert(*k, k_hot_list.clone());
        }

        let mut k1_constraint_ids: HashSet<ConstraintID> = match result.get(&1) {
            Some(k_hots) => k_hots.iter().map(|c| c.id).collect(),
            None => HashSet::new(),
        };

        let mut k1_constraints = match result.get(&1) {
            Some(k_hots) => k_hots.clone(),
            None => Vec::new(),
        };

        for one_hot in &self.one_hot_constraints {
            if !k1_constraint_ids.contains(&one_hot.id) {
                k1_constraint_ids.insert(one_hot.id);
                k1_constraints.push(KHot {
                    id: one_hot.id,
                    variables: one_hot.variables.clone(),
                    num_hot_vars: 1,
                });
            }
        }

        if !k1_constraints.is_empty() {
            result.insert(1, k1_constraints);
        }

        result
    }
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

        let mut decision_variable_dependency = HashMap::new();
        for (id, f) in value.decision_variable_dependency {
            decision_variable_dependency.insert(
                as_variable_id(&decision_variables, id)
                    .map_err(|e| e.context(message, "decision_variable_dependency"))?,
                f.parse_as(&(), message, "decision_variable_dependency")?,
            );
        }

        let context = (decision_variables, constraints);
        let constraint_hints = if let Some(hints) = value.constraint_hints {
            hints.parse_as(&context, message, "constraint_hints")?
        } else {
            Default::default()
        };
        let (decision_variables, constraints) = context;

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
