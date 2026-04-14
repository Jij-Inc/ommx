use super::*;
use ::approx::AbsDiffEq;

impl AbsDiffEq for Constraint<Created> {
    type Epsilon = crate::ATol;

    fn default_epsilon() -> Self::Epsilon {
        Function::default_epsilon()
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        self.equality == other.equality
            && self
                .stage
                .function
                .abs_diff_eq(&other.stage.function, epsilon)
    }
}

impl AbsDiffEq for RemovedConstraint {
    type Epsilon = crate::ATol;

    fn default_epsilon() -> Self::Epsilon {
        Constraint::<Created>::default_epsilon()
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        self.equality == other.equality
            && self
                .stage
                .function
                .abs_diff_eq(&other.stage.function, epsilon)
            && self.stage.removed_reason == other.stage.removed_reason
    }
}
