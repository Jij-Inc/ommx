use ::approx::AbsDiffEq;

use super::*;
use crate::{DecisionVariableRole, Sense};
use std::{collections::BTreeMap, ops::Neg};

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

fn output_semantics(instance: &Instance) -> (Sense, &Function, bool) {
    instance.output_objective().map_or_else(
        || (instance.sense(), instance.objective(), true),
        |output| {
            (
                output.sense(),
                output.function(),
                output.preserves_optimality(),
            )
        },
    )
}

/// Collect every decision variable whose state-population rule can affect the
/// raw output objective.
fn output_population_closure(
    instance: &Instance,
    output: &Function,
    roles: &BTreeMap<VariableID, DecisionVariableRole>,
) -> VariableIDSet {
    let mut closure = VariableIDSet::new();
    let mut pending = output.required_ids().into_iter().collect::<Vec<_>>();
    while let Some(id) = pending.pop() {
        if !closure.insert(id) {
            continue;
        }
        if roles.get(&id) == Some(&DecisionVariableRole::Dependent) {
            let dependency = instance
                .decision_variable_dependency
                .get(&id)
                .expect("dependent role requires an Instance-owned assignment");
            pending.extend(dependency.required_ids());
        }
    }
    closure
}

impl AbsDiffEq for Instance {
    type Epsilon = crate::ATol;

    fn default_epsilon() -> Self::Epsilon {
        Function::default_epsilon()
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        // Compare the active objective function. Note that min f(x) and
        // max -f(x) are equivalent.
        if !objective_pairs_abs_diff_eq(
            self.sense,
            &self.objective,
            other.sense,
            &other.objective,
            epsilon,
        ) {
            return false;
        }

        // Compare raw output semantics rather than substituting fixed or
        // dependent assignments. State population accepts caller-provided
        // values within tolerance, so symbolic substitution could amplify an
        // accepted difference and produce a false positive here. Structural
        // absence is semantically the active pair with optimality transport.
        let (self_output_sense, self_output, self_preserves_optimality) = output_semantics(self);
        let (other_output_sense, other_output, other_preserves_optimality) =
            output_semantics(other);
        if !objective_pairs_abs_diff_eq(
            self_output_sense,
            self_output,
            other_output_sense,
            other_output,
            epsilon,
        ) {
            return false;
        }
        if self_preserves_optimality != other_preserves_optimality {
            return false;
        }

        let self_used_ids = self.used_decision_variable_ids();
        let other_used_ids = other.used_decision_variable_ids();
        if self_used_ids != other_used_ids {
            return false;
        }

        // Output-only variables are not solver-used, but their definitions and
        // state-population rules still affect evaluation. Follow dependent RHS
        // expressions transitively and require the two population environments
        // to agree. This is deliberately a sufficient, conservative comparison:
        // algebraically equivalent raw outputs with different population
        // structures may compare unequal, but accepted state tolerances cannot
        // create a false positive.
        let self_roles = self.decision_variable_roles();
        let other_roles = other.decision_variable_roles();
        let self_output_ids = output_population_closure(self, self_output, &self_roles);
        let other_output_ids = output_population_closure(other, other_output, &other_roles);
        if self_output_ids != other_output_ids {
            return false;
        }

        for id in &self_output_ids {
            let Some(self_role) = self_roles.get(id) else {
                return false;
            };
            let Some(other_role) = other_roles.get(id) else {
                return false;
            };
            if self_role != other_role {
                return false;
            }
            match self_role {
                DecisionVariableRole::Fixed => {
                    let self_value = self
                        .fixed_decision_variable_values()
                        .get(id)
                        .expect("fixed role requires an Instance-owned value");
                    let other_value = other
                        .fixed_decision_variable_values()
                        .get(id)
                        .expect("fixed role requires an Instance-owned value");
                    if (*self_value - *other_value).abs() > epsilon.into_inner() {
                        return false;
                    }
                }
                DecisionVariableRole::Dependent => {
                    let self_dependency = self
                        .decision_variable_dependency
                        .get(id)
                        .expect("dependent role requires an Instance-owned assignment");
                    let other_dependency = other
                        .decision_variable_dependency
                        .get(id)
                        .expect("dependent role requires an Instance-owned assignment");
                    if !self_dependency.abs_diff_eq(other_dependency, epsilon) {
                        return false;
                    }
                }
                DecisionVariableRole::Used | DecisionVariableRole::Irrelevant => {}
            }
        }

        let mut compared_ids = self_used_ids;
        compared_ids.extend(self_output_ids);
        for id in compared_ids {
            let Some(self_var) = self.decision_variables.get(&id) else {
                return false;
            };
            let Some(other_var) = other.decision_variables.get(&id) else {
                return false;
            };
            if !self_var.abs_diff_eq(other_var, epsilon) {
                return false;
            }
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
    use crate::{coeff, linear, v1::State, DecisionVariable};
    use std::collections::{BTreeMap, HashMap};

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
    fn absent_and_equivalent_explicit_output_are_semantically_equal() {
        let left = instance();
        let mut right = left.clone();
        right.output_objective = Some(OutputObjective::new(
            right.sense,
            right.objective.clone(),
            true,
        ));

        assert!(left.abs_diff_eq(&right, crate::ATol::default()));
    }

    #[test]
    fn output_pairs_use_min_max_negation_equivalence() {
        let mut left = instance();
        left.output_objective = Some(OutputObjective::new(
            Sense::Maximize,
            Function::from(linear!(1)),
            true,
        ));
        let mut right = left.clone();
        right.output_objective = Some(OutputObjective::new(
            Sense::Minimize,
            Function::from((coeff!(-1.0) * linear!(1)).unwrap()),
            true,
        ));

        assert!(left.abs_diff_eq(&right, crate::ATol::default()));
    }

    #[test]
    fn different_forced_output_values_are_not_equal() {
        let make_instance = |value| {
            let id = VariableID::from(1);
            let mut instance = Instance::builder()
                .sense(Sense::Minimize)
                .objective(Function::Zero)
                .decision_variables(BTreeMap::from([(id, DecisionVariable::binary())]))
                .fixed_decision_variable_values(BTreeMap::from([(id, value)]))
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

        assert!(!make_instance(0.0).abs_diff_eq(&make_instance(1.0), crate::ATol::default()));
    }

    #[test]
    fn different_output_optimality_transport_is_not_equal() {
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
    fn caller_supplied_output_only_variable_remains_semantically_free() {
        let id = VariableID::from(1);
        let make_instance = |output| {
            let mut instance = Instance::new(
                Sense::Minimize,
                Function::Zero,
                BTreeMap::from([(id, DecisionVariable::binary())]),
                BTreeMap::new(),
            )
            .unwrap();
            instance.output_objective = Some(OutputObjective::new(Sense::Minimize, output, true));
            instance
        };
        let output_variable = make_instance(Function::from(linear!(1)));
        let output_zero = make_instance(Function::Zero);
        let state = State::from(HashMap::from([(id.into_inner(), 1.0)]));

        assert_eq!(
            *output_variable
                .evaluate(&state, crate::ATol::default())
                .unwrap()
                .objective(),
            1.0,
        );
        assert_eq!(
            *output_zero
                .evaluate(&state, crate::ATol::default())
                .unwrap()
                .objective(),
            0.0,
        );
        assert!(!output_variable.abs_diff_eq(&output_zero, crate::ATol::default()));
    }

    #[test]
    fn output_only_variable_definitions_are_compared() {
        let id = VariableID::from(1);
        let make_instance = |variable| {
            let mut instance = Instance::new(
                Sense::Minimize,
                Function::Zero,
                BTreeMap::from([(id, variable)]),
                BTreeMap::new(),
            )
            .unwrap();
            instance.output_objective = Some(OutputObjective::new(
                Sense::Minimize,
                Function::from(linear!(1)),
                true,
            ));
            instance
        };

        assert!(!make_instance(DecisionVariable::binary()).abs_diff_eq(
            &make_instance(DecisionVariable::continuous()),
            crate::ATol::default(),
        ));
    }

    #[test]
    fn different_dependent_output_population_is_not_equal() {
        let free = VariableID::from(1);
        let dependent = VariableID::from(2);
        let make_instance = |dependency| {
            let mut instance = Instance::builder()
                .sense(Sense::Minimize)
                .objective(Function::Zero)
                .decision_variables(BTreeMap::from([
                    (free, DecisionVariable::binary()),
                    (dependent, DecisionVariable::binary()),
                ]))
                .constraints(BTreeMap::new())
                .decision_variable_dependency(
                    AcyclicAssignments::new([(dependent, dependency)]).unwrap(),
                )
                .build()
                .unwrap();
            instance.output_objective = Some(OutputObjective::new(
                Sense::Minimize,
                Function::from(linear!(2)),
                true,
            ));
            instance
        };
        let direct = make_instance(Function::from(linear!(1)));
        let complemented = make_instance(Function::from(
            (coeff!(1.0) + (coeff!(-1.0) * linear!(1)).unwrap()).unwrap(),
        ));

        assert!(!direct.abs_diff_eq(&complemented, crate::ATol::default()));
    }

    #[test]
    fn fixed_population_tolerance_is_not_symbolically_substituted() {
        let id = VariableID::from(1);
        let make_instance = |output| {
            let mut instance = Instance::builder()
                .sense(Sense::Minimize)
                .objective(Function::Zero)
                .decision_variables(BTreeMap::from([(id, DecisionVariable::binary())]))
                .fixed_decision_variable_values(BTreeMap::from([(id, 1.0)]))
                .constraints(BTreeMap::new())
                .build()
                .unwrap();
            instance.output_objective = Some(OutputObjective::new(Sense::Minimize, output, true));
            instance
        };
        let scaled = make_instance(Function::from((coeff!(4.0) * linear!(1)).unwrap()));
        let constant = make_instance(Function::try_from(4.0).unwrap());
        let epsilon = crate::ATol::default();
        let state = State::from(HashMap::from([(
            id.into_inner(),
            1.0 + epsilon.into_inner() / 2.0,
        )]));
        let scaled_value = *scaled.evaluate(&state, epsilon).unwrap().objective();
        let constant_value = *constant.evaluate(&state, epsilon).unwrap().objective();

        assert!(
            (scaled_value - constant_value).abs() > epsilon.into_inner(),
            "the accepted fixed-value perturbation must expose the old false positive",
        );
        assert!(!scaled.abs_diff_eq(&constant, epsilon));
    }
}
