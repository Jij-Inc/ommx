use ::approx::AbsDiffEq;

use super::*;
use crate::{Kind, Sense, Substitute};
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

/// Normalize the effective output function by applying every deterministic
/// state-population rule. Active used variables remain free; fixed,
/// irrelevant, and dependent variables are replaced with the values or
/// functions that [`Instance::evaluate`] supplies before evaluating the output
/// objective.
fn populated_output_objective(instance: &Instance) -> Option<(Sense, Function)> {
    let (sense, output) = instance.objective_for_output();
    let used = instance.used_decision_variable_ids();
    let dependent: VariableIDSet = instance.decision_variable_dependency.keys().collect();
    let fixed: VariableIDSet = instance
        .fixed_decision_variable_values()
        .keys()
        .copied()
        .collect();
    let relevant: VariableIDSet = used
        .iter()
        .chain(fixed.iter())
        .chain(dependent.iter())
        .copied()
        .collect();

    let mut assignments = Vec::new();
    for (&id, &value) in instance.fixed_decision_variable_values() {
        if !dependent.contains(&id) {
            assignments.push((id, Function::try_from(value).ok()?));
        }
    }
    for (&id, variable) in instance.decision_variables.iter() {
        if !relevant.contains(&id) {
            let value = match variable.kind() {
                Kind::SemiInteger | Kind::SemiContinuous => 0.0,
                Kind::Binary | Kind::Integer | Kind::Continuous => {
                    variable.bound().nearest_to_zero()
                }
            };
            assignments.push((id, Function::try_from(value).ok()?));
        }
    }
    assignments.extend(
        instance
            .decision_variable_dependency
            .iter()
            .map(|(id, function)| (*id, function.clone())),
    );

    let assignments = AcyclicAssignments::new(assignments).ok()?;
    let output = output.clone().substitute_acyclic(&assignments).ok()?;
    Some((sense, output))
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
        let Some((self_output_sense, self_output)) = populated_output_objective(self) else {
            return false;
        };
        let Some((other_output_sense, other_output)) = populated_output_objective(other) else {
            return false;
        };
        if !objective_pairs_abs_diff_eq(
            self_output_sense,
            &self_output,
            other_output_sense,
            &other_output,
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
        assert!(transformed.convert_active_objective(Sense::Minimize));

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

    #[test]
    fn different_fixed_output_population_is_not_equivalent() {
        let make_instance = |value| {
            let mut instance = Instance::builder()
                .sense(Sense::Minimize)
                .objective(Function::Zero)
                .decision_variables(BTreeMap::from([(
                    VariableID::from(1),
                    DecisionVariable::binary(),
                )]))
                .fixed_decision_variable_values(BTreeMap::from([(VariableID::from(1), value)]))
                .constraints(BTreeMap::new())
                .build()
                .unwrap();
            instance.output_objective = Some(OutputObjective::new(
                Sense::Minimize,
                Function::from(linear!(1)),
                true,
            ));
            instance
        };

        let left = make_instance(0.0);
        let right = make_instance(1.0);

        assert_eq!(
            *left
                .evaluate(&crate::v1::State::default(), crate::ATol::default())
                .unwrap()
                .objective(),
            0.0
        );
        assert_eq!(
            *right
                .evaluate(&crate::v1::State::default(), crate::ATol::default())
                .unwrap()
                .objective(),
            1.0
        );
        assert!(!left.abs_diff_eq(&right, crate::ATol::default()));
    }

    #[test]
    fn different_dependent_output_population_is_not_equivalent() {
        let make_instance = |factor| {
            let used = VariableID::from(1);
            let dependent = VariableID::from(2);
            let dependency = Function::from((coeff!(factor) * linear!(used)).unwrap());
            let mut instance = Instance::builder()
                .sense(Sense::Minimize)
                .objective(Function::from(linear!(used)))
                .decision_variables(BTreeMap::from([
                    (used, DecisionVariable::continuous()),
                    (dependent, DecisionVariable::continuous()),
                ]))
                .constraints(BTreeMap::new())
                .decision_variable_dependency(
                    AcyclicAssignments::new([(dependent, dependency)]).unwrap(),
                )
                .build()
                .unwrap();
            instance.output_objective = Some(OutputObjective::new(
                Sense::Minimize,
                Function::from(linear!(dependent)),
                true,
            ));
            instance
        };

        let left = make_instance(1.0);
        let right = make_instance(2.0);

        assert!(!left.abs_diff_eq(&right, crate::ATol::default()));
    }
}
