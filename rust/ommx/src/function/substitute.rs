use super::*;
use crate::{substitute::IdempotentAssignments, Evaluate, Substitute, VariableID, VariableIDSet};

impl Function {
    /// Apply the substitution owner's idempotent table across Function and
    /// compact-polynomial boundaries without revisiting substituted expressions.
    pub(crate) fn substitute_idempotent(
        self,
        flattened: &IdempotentAssignments<'_>,
    ) -> Result<Self, crate::SubstitutionError> {
        match self {
            Function::Zero | Function::Constant(_) => Ok(self),
            Function::Linear(value) => value.substitute_idempotent(flattened),
            Function::Quadratic(value) => value.substitute_idempotent(flattened),
            Function::Polynomial(value) => value.substitute_idempotent(flattened),
        }
    }
}

impl Substitute for Function {
    type Output = Self;

    fn substitute_acyclic(
        self,
        acyclic: &crate::AcyclicAssignments,
    ) -> Result<Self::Output, crate::SubstitutionError> {
        // Early return if no substitution is needed
        if acyclic.is_empty() {
            return Ok(self);
        }
        let substituted_variables: VariableIDSet = acyclic.keys().collect();
        let required_ids = self.required_ids();
        if required_ids.is_disjoint(&substituted_variables) {
            return Ok(self);
        }
        let flattened = acyclic.flatten_for(&required_ids)?;
        self.substitute_idempotent(&flattened)
    }

    fn substitute_one(
        self,
        assigned: VariableID,
        f: &Function,
    ) -> Result<Self, crate::substitute::SubstitutionError> {
        match self {
            Function::Zero => Ok(Function::Zero),
            Function::Constant(c) => Ok(Function::Constant(c)),
            Function::Linear(l) => {
                let substituted = l.substitute_one(assigned, f)?;
                Ok(substituted)
            }
            Function::Quadratic(q) => {
                let substituted = q.substitute_one(assigned, f)?;
                Ok(substituted)
            }
            Function::Polynomial(p) => {
                let substituted = p.substitute_one(assigned, f)?;
                Ok(substituted)
            }
        }
    }
}
