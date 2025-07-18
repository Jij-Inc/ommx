use crate::{
    v1::{Function, Instance, Parameters, ParametricInstance, State},
    Evaluate, VariableIDSet,
};
use anyhow::{bail, Result};
use std::{borrow::Cow, collections::BTreeSet};

impl From<Instance> for ParametricInstance {
    fn from(
        Instance {
            description,
            objective,
            constraints,
            decision_variables,
            sense,
            constraint_hints,
            removed_constraints,
            parameters: _, // Drop previous parameters
            decision_variable_dependency,
        }: Instance,
    ) -> Self {
        Self {
            description,
            objective,
            constraints,
            decision_variables,
            sense,
            parameters: Default::default(),
            constraint_hints,
            removed_constraints,
            decision_variable_dependency,
        }
    }
}

impl From<State> for Parameters {
    fn from(State { entries }: State) -> Self {
        Self { entries }
    }
}

impl From<Parameters> for State {
    fn from(Parameters { entries }: Parameters) -> Self {
        Self { entries }
    }
}

impl ParametricInstance {
    pub fn objective(&self) -> Cow<Function> {
        match &self.objective {
            Some(f) => Cow::Borrowed(f),
            None => Cow::Owned(Function::default()),
        }
    }

    /// Used decision variable and parameter IDs in the objective and constraints.
    pub fn used_ids(&self) -> Result<VariableIDSet> {
        let mut used_ids = self.objective().required_ids();
        for c in &self.constraints {
            used_ids.extend(c.function().required_ids());
        }
        Ok(used_ids)
    }

    /// Defined decision variable IDs. These IDs may not be used in the objective and constraints.
    pub fn defined_decision_variable_ids(&self) -> VariableIDSet {
        self.decision_variables
            .iter()
            .map(|dv| dv.id.into())
            .collect()
    }

    /// Defined parameter IDs. These IDs may not be used in the objective and constraints.
    pub fn defined_parameter_ids(&self) -> VariableIDSet {
        self.parameters.iter().map(|p| p.id.into()).collect()
    }

    pub fn validate(&self) -> Result<()> {
        self.validate_ids()?;
        self.validate_constraint_ids()?;
        Ok(())
    }

    pub fn validate_ids(&self) -> Result<()> {
        let mut ids = VariableIDSet::default();
        for dv in &self.decision_variables {
            if !ids.insert(dv.id.into()) {
                bail!("Duplicate decision variable ID: {}", dv.id);
            }
        }
        for p in &self.parameters {
            if !ids.insert(p.id.into()) {
                bail!("Duplicate parameter ID: {}", p.id);
            }
        }
        let used_ids = self.used_ids()?;
        if !used_ids.is_subset(&ids) {
            let sub = used_ids.difference(&ids).collect::<BTreeSet<_>>();
            bail!("Undefined ID is used: {:?}", sub);
        }
        Ok(())
    }

    pub fn validate_constraint_ids(&self) -> Result<()> {
        let mut ids = BTreeSet::new();
        for c in &self.constraints {
            if !ids.insert(c.id) {
                bail!("Duplicate constraint ID: {}", c.id);
            }
        }
        for c in &self.removed_constraints {
            if let Some(c) = c.constraint.as_ref() {
                if !ids.insert(c.id) {
                    bail!("Duplicate removed constraint ID: {}", c.id);
                }
            }
        }
        Ok(())
    }
}
