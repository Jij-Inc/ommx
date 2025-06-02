use super::*;
use crate::{Substitute, VariableID};

impl Substitute for Function {
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
