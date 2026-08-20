use ::approx::AbsDiffEq;

use super::*;
use crate::Sense;
use std::ops::Neg;

fn objective_pairs_abs_diff_eq(
    left_sense: Sense,
    left: &Function,
    right_sense: Sense,
    right: &Function,
    epsilon: crate::ATol,
) -> bool {
    match (left_sense, right_sense) {
        (Sense::Minimize, Sense::Maximize) | (Sense::Maximize, Sense::Minimize) => {
            left.clone().neg().abs_diff_eq(right, epsilon)
        }
        _ => left.abs_diff_eq(right, epsilon),
    }
}

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
        if !objective_pairs_abs_diff_eq(
            self.sense,
            &self.objective,
            other.sense,
            &other.objective,
            epsilon,
        ) {
            return false;
        }

        // Compare the effective output objective pair under the same weak
        // mathematical equivalence as the active formulation. As elsewhere in
        // this implementation, unrelated inactive state is intentionally not
        // part of Instance::abs_diff_eq.
        let (self_output_sense, self_output) = self.objective_for_output();
        let (other_output_sense, other_output) = other.objective_for_output();
        if !objective_pairs_abs_diff_eq(
            self_output_sense,
            self_output,
            other_output_sense,
            other_output,
            epsilon,
        ) {
            return false;
        }
        let self_preserves_optimality = self
            .output_objective()
            .is_none_or(OutputObjective::preserves_optimality);
        let other_preserves_optimality = other
            .output_objective()
            .is_none_or(OutputObjective::preserves_optimality);
        if self_preserves_optimality != other_preserves_optimality {
            return false;
        }

        // Compare constraints
        // Note that `removed_constraints` are not considered in the comparison
        for (id, c_self) in self.constraints() {
            match other.constraints().get(id) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, linear, DecisionVariable};
    use std::collections::BTreeMap;

    fn instance() -> Instance {
        Instance::new(
            Sense::Maximize,
            Function::from(linear!(1)),
            BTreeMap::from([(VariableID::from(1), DecisionVariable::binary())]),
            BTreeMap::new(),
        )
        .unwrap()
    }

    #[test]
    fn transformed_and_input_output_semantics_are_equivalent() {
        let original = instance();
        let mut transformed = original.clone();
        assert!(transformed.as_minimization_problem());

        assert!(original.abs_diff_eq(&transformed, crate::ATol::default()));
    }

    #[test]
    fn different_effective_output_objectives_are_not_equivalent() {
        let left = instance();
        let mut right = left.clone();
        right.output_objective = Some(OutputObjective::new(
            Sense::Maximize,
            Function::from(coeff!(2.0) * linear!(1)),
            true,
        ));

        assert!(!left.abs_diff_eq(&right, crate::ATol::default()));
    }

    #[test]
    fn different_output_optimality_guarantees_are_not_equivalent() {
        let left = instance();
        let mut right = left.clone();
        right.output_objective = Some(OutputObjective::new(
            right.sense,
            right.objective.clone(),
            false,
        ));

        assert!(!left.abs_diff_eq(&right, crate::ATol::default()));
    }
}
