use super::*;
use crate::ATol;
use ::approx::AbsDiffEq;

impl AbsDiffEq for Function {
    type Epsilon = ATol;

    fn default_epsilon() -> Self::Epsilon {
        ATol::default()
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        if self.is_polynomial() && other.is_polynomial() {
            let Ok(diff) = self.clone() - other.clone() else {
                return false;
            };
            return diff
                .values()
                .expect("difference of polynomials is a polynomial")
                .map(|c| c.abs())
                .max()
                .is_none_or(|c| c <= epsilon);
        }

        let (Function::Expression(lhs), Function::Expression(rhs)) = (self, other) else {
            return false;
        };
        lhs.instructions().len() == rhs.instructions().len()
            && lhs
                .instructions()
                .iter()
                .zip(rhs.instructions())
                .all(|(lhs, rhs)| instruction_abs_diff_eq(lhs, rhs, epsilon))
    }
}

fn instruction_abs_diff_eq(lhs: &Instruction, rhs: &Instruction, epsilon: ATol) -> bool {
    match (lhs, rhs) {
        (Instruction::Push(lhs), Instruction::Push(rhs)) => {
            lhs.to_function().abs_diff_eq(&rhs.to_function(), epsilon)
        }
        (Instruction::Unary(lhs), Instruction::Unary(rhs)) => lhs == rhs,
        (Instruction::Associative(lhs), Instruction::Associative(rhs)) => lhs == rhs,
        (Instruction::Binary(lhs), Instruction::Binary(rhs)) => lhs == rhs,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coeff;
    use ::approx::assert_abs_diff_eq;

    #[test]
    fn test_abs_diff_eq() {
        let f = Function::from(coeff!(1.0));
        let g = Function::from(coeff!(1.0000000001));
        assert_abs_diff_eq!(f, g);
    }

    #[test]
    fn expression_atoms_use_coefficient_tolerance() {
        let f = Function::from(coeff!(1.0)).abs();
        let g = Function::from(coeff!(1.0000000001)).abs();
        assert_abs_diff_eq!(f, g);
    }
}
