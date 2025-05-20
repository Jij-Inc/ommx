use crate::{
    v1::{
        Constraint, Equality, EvaluatedConstraint, Function, RemovedConstraint, SampledConstraint,
        Samples, State,
    },
    Evaluate, VariableIDSet,
};
use anyhow::{bail, ensure, Context, Result};
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
        ensure!(atol > 0.0, "atol must be positive");
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
        ensure!(atol > 0.0, "atol must be positive");
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

impl Evaluate for Constraint {
    type Output = EvaluatedConstraint;
    type SampledOutput = SampledConstraint;

    fn evaluate(&self, solution: &State, atol: crate::ATol) -> Result<Self::Output> {
        let evaluated_value = self.function().evaluate(solution, atol)?;
        let used_decision_variable_ids = self
            .function()
            .required_ids()
            .into_iter()
            .map(|id| id.into_inner())
            .collect();
        Ok(EvaluatedConstraint {
            id: self.id,
            equality: self.equality,
            evaluated_value,
            used_decision_variable_ids,
            name: self.name.clone(),
            subscripts: self.subscripts.clone(),
            parameters: self.parameters.clone(),
            description: self.description.clone(),
            dual_variable: None,
            removed_reason: None,
            removed_reason_parameters: Default::default(),
        })
    }

    fn partial_evaluate(&mut self, state: &State, atol: crate::ATol) -> Result<()> {
        let Some(f) = self.function.as_mut() else {
            // Since empty function means zero constant, we can return an empty set
            return Ok(());
        };
        f.partial_evaluate(state, atol)
    }

    fn evaluate_samples(
        &self,
        samples: &Samples,
        atol: crate::ATol,
    ) -> Result<Self::SampledOutput> {
        let evaluated_values = self.function().evaluate_samples(samples, atol)?;
        let feasible: HashMap<u64, bool> = evaluated_values
            .iter()
            .map(|(sample_id, value)| {
                if self.equality() == Equality::EqualToZero {
                    return Ok((*sample_id, value.abs() < *atol));
                }
                if self.equality() == Equality::LessThanOrEqualToZero {
                    return Ok((*sample_id, *value < *atol));
                }
                bail!("Unsupported equality: {:?}", self.equality());
            })
            .collect::<Result<_>>()?;
        Ok(SampledConstraint {
            id: self.id,
            evaluated_values: Some(evaluated_values),
            used_decision_variable_ids: self
                .function()
                .required_ids()
                .into_iter()
                .map(|id| id.into_inner())
                .collect(),
            name: self.name.clone(),
            subscripts: self.subscripts.clone(),
            parameters: self.parameters.clone(),
            description: self.description.clone(),
            equality: self.equality,
            feasible,
            removed_reason: None,
            removed_reason_parameters: Default::default(),
        })
    }

    fn required_ids(&self) -> VariableIDSet {
        self.function
            .as_ref()
            .map_or(VariableIDSet::default(), |f| f.required_ids())
    }
}

impl Evaluate for RemovedConstraint {
    type Output = EvaluatedConstraint;
    type SampledOutput = SampledConstraint;

    fn evaluate(&self, solution: &State, atol: crate::ATol) -> Result<Self::Output> {
        let mut out = self
            .constraint
            .as_ref()
            .context("RemovedConstraint does not contain constraint")?
            .evaluate(solution, atol)?;
        out.removed_reason = Some(self.removed_reason.clone());
        out.removed_reason_parameters = self.removed_reason_parameters.clone();
        Ok(out)
    }

    fn partial_evaluate(&mut self, state: &State, atol: crate::ATol) -> Result<()> {
        self.constraint
            .as_mut()
            .context("RemovedConstraint does not contain constraint")?
            .partial_evaluate(state, atol)
    }

    fn evaluate_samples(
        &self,
        samples: &Samples,
        atol: crate::ATol,
    ) -> Result<Self::SampledOutput> {
        let mut evaluated = self
            .constraint
            .as_ref()
            .expect("RemovedConstraint does not contain constraint")
            .evaluate_samples(samples, atol)?;
        evaluated.removed_reason = Some(self.removed_reason.clone());
        evaluated.removed_reason_parameters = self.removed_reason_parameters.clone();
        Ok(evaluated)
    }

    fn required_ids(&self) -> VariableIDSet {
        self.constraint
            .as_ref()
            .map_or(VariableIDSet::default(), |c| c.required_ids())
    }
}
