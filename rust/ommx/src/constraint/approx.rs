use super::*;
use ::approx::AbsDiffEq;

impl AbsDiffEq for Constraint {
    type Epsilon = crate::ATol;

    fn default_epsilon() -> Self::Epsilon {
        Function::default_epsilon()
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        self.equality == other.equality && self.function.abs_diff_eq(&other.function, epsilon)
    }
}

impl AbsDiffEq for RemovedConstraint {
    type Epsilon = crate::ATol;

    fn default_epsilon() -> Self::Epsilon {
        Constraint::default_epsilon()
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        self.constraint.abs_diff_eq(&other.constraint, epsilon)
    }
}
