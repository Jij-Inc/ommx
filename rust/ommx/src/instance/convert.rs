use super::*;
use std::ops::Neg;

impl Instance {
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

impl From<Instance> for ParametricInstance {
    fn from(
        Instance {
            sense,
            objective,
            decision_variables,
            constraints,
            removed_constraints,
            decision_variable_dependency,
            constraint_hints,
            description,
            ..
        }: Instance,
    ) -> Self {
        ParametricInstance {
            sense,
            objective,
            decision_variables,
            parameters: BTreeMap::default(),
            constraints,
            removed_constraints,
            decision_variable_dependency,
            constraint_hints,
            description,
        }
    }
}

impl ParametricInstance {
    pub fn with_parameters(self, parameters: BTreeMap<VariableID, f64>) -> anyhow::Result<Self> {
        todo!()
    }
}
