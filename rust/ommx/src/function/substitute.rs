use super::*;
use crate::{Substitute, VariableID};

impl Substitute for Function {
    type Output = Self;

    fn substitute_one(
        self,
        assigned: VariableID,
        linear: &Linear,
    ) -> Result<Self::Output, crate::substitute::RecursiveAssignmentError> {
        match self {
            Function::Zero => Ok(Function::Zero),
            Function::Constant(c) => Ok(Function::Constant(c)),
            Function::Linear(l) => {
                let substituted = l.substitute_one(assigned, linear)?;
                Ok(Function::from(substituted))
            }
            Function::Quadratic(q) => {
                let substituted = q.substitute_one(assigned, linear)?;
                Ok(Function::from(substituted))
            }
            Function::Polynomial(p) => {
                let substituted = p.substitute_one(assigned, linear)?;
                Ok(Function::from(substituted))
            }
        }
    }
}
