use super::*;
use crate::{substitute::ResolvedAssignments, Evaluate, Substitute, VariableID, VariableIDSet};

impl Function {
    /// The substitution owner resolves dependencies once before passing this
    /// read-only plan across Function and compact-polynomial boundaries.
    pub(crate) fn substitute_resolved(
        self,
        resolved: &ResolvedAssignments<'_>,
    ) -> Result<Self, crate::SubstitutionError> {
        match self {
            Function::Zero | Function::Constant(_) => Ok(self),
            Function::Linear(value) => value.substitute_resolved(resolved),
            Function::Quadratic(value) => value.substitute_resolved(resolved),
            Function::Polynomial(value) => value.substitute_resolved(resolved),
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
        let resolved = acyclic.resolve_for(&required_ids)?;
        self.substitute_resolved(&resolved)
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
