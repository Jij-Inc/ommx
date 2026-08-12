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

        match (self, other) {
            (Function::Unary(lhs), Function::Unary(rhs)) => {
                lhs.operator() == rhs.operator()
                    && lhs.operand().abs_diff_eq(rhs.operand(), epsilon)
            }
            (Function::Nary(lhs), Function::Nary(rhs)) => {
                lhs.operator() == rhs.operator()
                    && lhs.operands().len() == rhs.operands().len()
                    && lhs
                        .operands()
                        .iter()
                        .zip(rhs.operands())
                        .all(|(lhs, rhs)| lhs.abs_diff_eq(rhs, epsilon))
            }
            (Function::Binary(lhs), Function::Binary(rhs)) => {
                lhs.operator() == rhs.operator()
                    && lhs.lhs().abs_diff_eq(rhs.lhs(), epsilon)
                    && lhs.rhs().abs_diff_eq(rhs.rhs(), epsilon)
            }
            _ => false,
        }
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
}
