use super::*;
use crate::{substitute_acyclic_default, Substitute, VariableID};

impl Substitute for Function {
    type Output = Self;

    fn substitute_acyclic(
        self,
        acyclic: &crate::AcyclicAssignments,
    ) -> Result<Self::Output, crate::SubstitutionError> {
        substitute_acyclic_default(self, acyclic)
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
