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
        match self {
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{linear, ATol, Evaluate};

    #[test]
    fn deep_expression_substitution_is_iterative() {
        let mut function = Function::from(linear!(1));
        for _ in 0..4096 {
            function = function.powi(2);
        }

        let substituted = function.substitute_one(1.into(), &Function::one()).unwrap();
        assert!(matches!(&substituted, Function::Expression(_)));
        assert_eq!(
            substituted
                .evaluate(&crate::v1::State::default(), crate::ATol::default())
                .unwrap(),
            1.0
        );
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

    #[test]
    fn substitution_preserves_the_atol_choice_for_later_evaluation() {
        let function = Function::from(linear!(1)).signum();
        let substituted = function
            .substitute_one(1.into(), &Function::try_from(1e-8).unwrap())
            .unwrap();

        assert!(matches!(substituted, Function::Expression(_)));
        assert_eq!(
            substituted
                .evaluate(&crate::v1::State::default(), ATol::new(1e-6).unwrap())
                .unwrap(),
            0.0
        );
        assert_eq!(
            substituted
                .evaluate(&crate::v1::State::default(), ATol::new(1e-9).unwrap())
                .unwrap(),
            1.0
        );
    }
}
