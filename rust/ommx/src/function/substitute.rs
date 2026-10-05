use super::operation::{
    from_instructions_exact, instructions, into_expression_instructions, into_instructions,
    Instruction,
};
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
            Function::Expression(expression) => {
                let mut output = Vec::with_capacity(instructions(&expression).len());
                for instruction in into_expression_instructions(expression) {
                    match instruction {
                        Instruction::Push(atom) => output.extend(into_instructions(
                            atom.into_function().substitute_idempotent(flattened)?,
                        )),
                        operation => output.push(operation),
                    }
                }
                Ok(from_instructions_exact(output)
                    .expect("replacing pushes with valid programs preserves expression validity"))
            }
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
    fn unused_dependency_arithmetic_is_not_evaluated() {
        let assignments = crate::AcyclicAssignments::new([
            (0.into(), Function::from(linear!(10))),
            (
                1.into(),
                Function::from((crate::coeff!(f64::MAX) * linear!(2)).unwrap()),
            ),
            (
                2.into(),
                Function::from((crate::coeff!(2.0) * linear!(3)).unwrap()),
            ),
        ])
        .unwrap();
        let result = Function::from(linear!(0))
            .substitute_acyclic(&assignments)
            .unwrap();
        assert_eq!(result, Function::from(linear!(10)));

        let mut function = Function::from(linear!(1));
        let before = function.clone();
        let error = crate::substitute_acyclic(&mut function, &assignments).unwrap_err();
        assert!(matches!(error, crate::SubstitutionError::Coefficient(_)));
        assert_eq!(function, before);
    }

    #[test]
    fn batch_substitution_preserves_expression_operators_and_dependencies() {
        let function = (Function::from(linear!(0)).signum() / Function::from(linear!(1)))
            .unwrap()
            .powi(2);
        let assignments = crate::AcyclicAssignments::new([
            (0.into(), Function::from(linear!(2))),
            (1.into(), Function::from(linear!(3))),
            (2.into(), Function::try_from(1e-8).unwrap()),
            (3.into(), Function::try_from(2.0).unwrap()),
        ])
        .unwrap();
        let batch = function.clone().substitute_acyclic(&assignments).unwrap();
        let sequential: Function =
            crate::substitute_acyclic_via_one(function, &assignments).unwrap();
        assert!(matches!(batch, Function::Expression(_)));
        for atol in [ATol::new(1e-6).unwrap(), ATol::new(1e-9).unwrap()] {
            let state = crate::v1::State::default();
            assert_eq!(
                batch.evaluate(&state, atol).unwrap(),
                sequential.evaluate(&state, atol).unwrap()
            );
        }
    }

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
