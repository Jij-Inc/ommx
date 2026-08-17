#[cfg(test)]
use super::operation::as_constant;
use super::operation::{
    from_instructions_exact, instructions, into_expression_instructions, into_instructions,
    Instruction,
};
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
        let substituted = match self {
            Function::Zero => Ok::<_, crate::SubstitutionError>(Function::Zero),
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
            Function::Expression(expression) => {
                let mut instructions = Vec::with_capacity(instructions(&expression).len());
                for instruction in into_expression_instructions(expression) {
                    match instruction {
                        Instruction::Push(atom) => {
                            let substituted = atom.into_function().substitute_one(assigned, f)?;
                            instructions.extend(into_instructions(substituted));
                        }
                        operation => instructions.push(operation),
                    }
                }
                Ok(from_instructions_exact(instructions)
                    .expect("replacing pushes with valid programs preserves expression validity"))
            }
        }?;

        // Preserve the eager constant folding previously performed while
        // rebuilding unary expression nodes. Undefined closed expressions
        // remain represented, just as division-by-zero and negative powers
        // did before the RPN migration.
        if substituted.required_ids().is_empty() && !substituted.is_polynomial() {
            if let Ok(value) =
                substituted.evaluate(&crate::v1::State::default(), crate::ATol::default())
            {
                return Ok(Function::try_from(value)
                    .expect("successful Function evaluation always returns a finite value"));
            }
        }
        Ok(substituted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linear;

    #[test]
    fn deep_expression_substitution_is_iterative() {
        let mut function = Function::from(linear!(1));
        for _ in 0..4096 {
            function = function.powi(2);
        }

        let substituted = function.substitute_one(1.into(), &Function::one()).unwrap();
        assert_eq!(as_constant(&substituted), Some(1.0));
    }

    #[test]
    fn substitution_preserves_undefined_closed_expression() {
        let function = (Function::one() / Function::from(linear!(1))).unwrap();
        let substituted = function
            .substitute_one(1.into(), &Function::zero())
            .unwrap();

        assert!(matches!(substituted, Function::Expression(_)));
        assert!(substituted
            .evaluate(&crate::v1::State::default(), crate::ATol::default())
            .is_err());
    }
}
