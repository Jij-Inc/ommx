use crate::v1::{Constraint, Equality, EvaluatedConstraint, Function, SampledConstraint};
use anyhow::{bail, Context, Result};
use approx::AbsDiffEq;
use num::Zero;
use std::{borrow::Cow, collections::HashMap};

impl Constraint {
    pub fn function(&self) -> Cow<Function> {
        match &self.function {
            Some(f) => Cow::Borrowed(f),
            // Empty function is regarded as zero function
            None => Cow::Owned(Function::zero()),
        }
    }
}

impl EvaluatedConstraint {
    pub fn is_feasible(&self, atol: crate::ATol) -> Result<bool> {
        if self.equality() == Equality::EqualToZero {
            return Ok(self.evaluated_value.abs() < *atol);
        } else if self.equality() == Equality::LessThanOrEqualToZero {
            return Ok(self.evaluated_value < *atol);
        }
        bail!("Unsupported equality: {:?}", self.equality());
    }
}

impl SampledConstraint {
    pub fn is_feasible(&self, atol: crate::ATol) -> Result<HashMap<u64, bool>> {
        let values = self
            .evaluated_values
            .as_ref()
            .context("evaluated_values of SampledConstraints is lacked")?;
        if self.equality() == Equality::EqualToZero {
            return Ok(values
                .iter()
                .map(|(id, value)| (*id, value.abs() < *atol))
                .collect());
        } else if self.equality() == Equality::LessThanOrEqualToZero {
            return Ok(values
                .iter()
                .map(|(id, value)| (*id, *value < *atol))
                .collect());
        }
        bail!("Unsupported equality: {:?}", self.equality());
    }

    pub fn get(&self, sample_id: u64) -> Result<EvaluatedConstraint> {
        Ok(EvaluatedConstraint {
            id: self.id,
            equality: self.equality,
            evaluated_value: self
                .evaluated_values
                .as_ref()
                .context("evaluated_values of SampledConstraints is lacked")?
                .get(sample_id)
                .context("SampledConstraint lacks evaluated value")?,
            used_decision_variable_ids: self.used_decision_variable_ids.clone(),
            name: self.name.clone(),
            subscripts: self.subscripts.clone(),
            parameters: self.parameters.clone(),
            description: self.description.clone(),
            removed_reason: self.removed_reason.clone(),
            removed_reason_parameters: self.removed_reason_parameters.clone(),
            dual_variable: None,
        })
    }
}

impl AbsDiffEq for Constraint {
    type Epsilon = crate::ATol;

    fn default_epsilon() -> Self::Epsilon {
        crate::ATol::default()
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        if self.equality != other.equality {
            return false;
        }
        if let (Some(f), Some(g)) = (&self.function, &other.function) {
            f.abs_diff_eq(g, epsilon)
        } else {
            false
        }
    }
}
