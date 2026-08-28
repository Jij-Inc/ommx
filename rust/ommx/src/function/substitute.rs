use super::*;
use crate::{substitute_acyclic_via_one, Evaluate, Substitute, VariableID, VariableIDSet};

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
        substitute_acyclic_via_one(self, acyclic)
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
