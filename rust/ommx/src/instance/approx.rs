use ::approx::AbsDiffEq;

use super::*;
use crate::Sense;
use std::ops::Neg;

impl AbsDiffEq for Instance {
    type Epsilon = crate::ATol;

    fn default_epsilon() -> Self::Epsilon {
        Function::default_epsilon()
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        // Compare the used decision variables
        let self_vars = self.used_decision_variables();
        let other_vars = other.used_decision_variables();
        if self_vars.len() != other_vars.len() {
            return false;
        }
        for ((id, var), (other_id, other_var)) in self_vars.iter().zip(other_vars.iter()) {
            if id != other_id || !var.abs_diff_eq(other_var, epsilon) {
                return false;
            }
        }

        // Compare the objective function
        // Note that min f(x) and max -f(x) are equivalent
        match (self.sense, other.sense) {
            (Sense::Minimize, Sense::Maximize) | (Sense::Maximize, Sense::Minimize) => {
                if !self
                    .objective
                    .clone()
                    .neg()
                    .abs_diff_eq(&other.objective, epsilon)
                {
                    return false;
                }
            }
            _ => {
                if !self.objective.abs_diff_eq(&other.objective, epsilon) {
                    return false;
                }
            }
        }

        // Compare constraints
        // Note that `removed_constraints` are not considered in the comparison
        for (id, c_self) in &self.constraints {
            match other.constraints.get(id) {
                Some(c_other) => {
                    if !c_self.abs_diff_eq(c_other, epsilon) {
                        return false;
                    }
                }
                None => return false,
            }
        }
        true
    }
}
