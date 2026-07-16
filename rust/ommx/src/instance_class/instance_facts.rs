use crate::{
    ConstraintID, Degree, Equality, IndicatorConstraintID, Instance, Kind, OneHotConstraintID,
    Sense, Sos1ConstraintID, VariableIDSet,
};
use std::collections::{BTreeMap, BTreeSet};

/// Membership-relevant facts for one active regular or indicator constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstraintFacts {
    relation: Equality,
    degree: Degree,
}

impl ConstraintFacts {
    pub fn relation(&self) -> Equality {
        self.relation
    }

    pub fn degree(&self) -> Degree {
        self.degree
    }
}

/// Facts derived from the active mathematical content of an [`Instance`].
///
/// This private value is diagnostic evidence for instance-class membership,
/// not another mutable source of truth. It includes the objective and every
/// active constraint family, and groups only used decision variables by
/// [`Kind`]. Fixed, dependent, irrelevant, removed-constraint-only, and
/// named-function-only variables are excluded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceFacts {
    sense: Sense,
    used_variables_by_kind: BTreeMap<Kind, VariableIDSet>,
    objective_degree: Degree,
    regular_constraints: BTreeMap<ConstraintID, ConstraintFacts>,
    indicator_constraints: BTreeMap<IndicatorConstraintID, ConstraintFacts>,
    one_hot_constraint_ids: BTreeSet<OneHotConstraintID>,
    sos1_constraint_ids: BTreeSet<Sos1ConstraintID>,
}

impl InstanceFacts {
    pub fn sense(&self) -> Sense {
        self.sense
    }

    pub fn used_variables_by_kind(&self) -> &BTreeMap<Kind, VariableIDSet> {
        &self.used_variables_by_kind
    }

    #[cfg(test)]
    pub fn used_variable_ids(&self) -> VariableIDSet {
        self.used_variables_by_kind
            .values()
            .flat_map(|ids| ids.iter().copied())
            .collect()
    }

    pub fn objective_degree(&self) -> Degree {
        self.objective_degree
    }

    pub fn regular_constraints(&self) -> &BTreeMap<ConstraintID, ConstraintFacts> {
        &self.regular_constraints
    }

    pub fn indicator_constraints(&self) -> &BTreeMap<IndicatorConstraintID, ConstraintFacts> {
        &self.indicator_constraints
    }

    pub fn one_hot_constraint_ids(&self) -> &BTreeSet<OneHotConstraintID> {
        &self.one_hot_constraint_ids
    }

    pub fn sos1_constraint_ids(&self) -> &BTreeSet<Sos1ConstraintID> {
        &self.sos1_constraint_ids
    }
}

impl From<&Instance> for InstanceFacts {
    fn from(instance: &Instance) -> Self {
        let mut used_variables_by_kind = BTreeMap::<Kind, VariableIDSet>::new();
        for (id, variable) in instance.used_decision_variables() {
            used_variables_by_kind
                .entry(variable.kind())
                .or_default()
                .insert(id);
        }

        Self {
            sense: instance.sense(),
            used_variables_by_kind,
            objective_degree: instance.objective().degree(),
            regular_constraints: instance
                .constraints()
                .iter()
                .map(|(id, constraint)| {
                    (
                        *id,
                        ConstraintFacts {
                            relation: constraint.equality,
                            degree: constraint.function().degree(),
                        },
                    )
                })
                .collect(),
            indicator_constraints: instance
                .indicator_constraints()
                .iter()
                .map(|(id, constraint)| {
                    (
                        *id,
                        ConstraintFacts {
                            relation: constraint.equality,
                            degree: constraint.function().degree(),
                        },
                    )
                })
                .collect(),
            one_hot_constraint_ids: instance.one_hot_constraints().keys().copied().collect(),
            sos1_constraint_ids: instance.sos1_constraints().keys().copied().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        linear, AcyclicAssignments, Constraint, DecisionVariable, Function, InstanceParameters,
        NamedFunction, NamedFunctionID, OneHotConstraint, OneHotConstraintID, VariableID,
    };
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn facts_match_every_active_instance_component(
            instance in any_with::<Instance>(InstanceParameters::full_v3())
        ) {
            let facts = InstanceFacts::from(&instance);

            prop_assert_eq!(facts.sense(), instance.sense());
            prop_assert_eq!(facts.objective_degree(), instance.objective().degree());
            prop_assert_eq!(facts.used_variable_ids(), instance.used_decision_variable_ids());
            for (id, variable) in instance.used_decision_variables() {
                prop_assert!(facts
                    .used_variables_by_kind()
                    .get(&variable.kind())
                    .is_some_and(|ids| ids.contains(&id)));
            }

            prop_assert_eq!(
                facts.regular_constraints().keys().copied().collect::<BTreeSet<_>>(),
                instance.constraints().keys().copied().collect::<BTreeSet<_>>()
            );
            for (id, constraint) in instance.constraints() {
                let fact = facts.regular_constraints().get(id).unwrap();
                prop_assert_eq!(fact.relation(), constraint.equality);
                prop_assert_eq!(fact.degree(), constraint.function().degree());
            }

            prop_assert_eq!(
                facts.indicator_constraints().keys().copied().collect::<BTreeSet<_>>(),
                instance.indicator_constraints().keys().copied().collect::<BTreeSet<_>>()
            );
            for (id, constraint) in instance.indicator_constraints() {
                let fact = facts.indicator_constraints().get(id).unwrap();
                prop_assert_eq!(fact.relation(), constraint.equality);
                prop_assert_eq!(fact.degree(), constraint.function().degree());
            }

            prop_assert_eq!(
                facts.one_hot_constraint_ids(),
                &instance.one_hot_constraints().keys().copied().collect()
            );
            prop_assert_eq!(
                facts.sos1_constraint_ids(),
                &instance.sos1_constraints().keys().copied().collect()
            );
        }
    }

    #[test]
    fn facts_are_recomputed_after_explicit_lowering() {
        let x = VariableID::from(1);
        let y = VariableID::from(2);
        let one_hot_id = OneHotConstraintID::from(7);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from((linear!(x) + linear!(y)).unwrap()))
            .decision_variables(BTreeMap::from([
                (x, DecisionVariable::binary()),
                (y, DecisionVariable::binary()),
            ]))
            .constraints(BTreeMap::new())
            .one_hot_constraints(BTreeMap::from([(
                one_hot_id,
                OneHotConstraint::new(BTreeSet::from([x, y])).unwrap(),
            )]))
            .build()
            .unwrap();

        let before = InstanceFacts::from(&instance);
        assert_eq!(
            before.one_hot_constraint_ids(),
            &BTreeSet::from([one_hot_id])
        );
        assert!(before.regular_constraints().is_empty());

        let regular_id = instance.convert_one_hot_to_constraint(one_hot_id).unwrap();
        let after = InstanceFacts::from(&instance);
        assert!(after.one_hot_constraint_ids().is_empty());
        assert_eq!(
            after.regular_constraints().get(&regular_id),
            Some(&ConstraintFacts {
                relation: Equality::EqualToZero,
                degree: 1.into(),
            })
        );
        assert_eq!(after.used_variable_ids(), before.used_variable_ids());
    }

    #[test]
    fn only_active_instance_variables_are_observed() {
        let used = VariableID::from(1);
        let fixed = VariableID::from(2);
        let dependent = VariableID::from(3);
        let dependency_source = VariableID::from(4);
        let named_only = VariableID::from(5);
        let removed_only = VariableID::from(6);
        let removed_reason = crate::constraint::RemovedReason {
            reason: "test".to_string(),
            parameters: Default::default(),
        };
        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(used)))
            .decision_variables(BTreeMap::from([
                (used, DecisionVariable::binary()),
                (fixed, DecisionVariable::semi_integer()),
                (dependent, DecisionVariable::semi_continuous()),
                (dependency_source, DecisionVariable::integer()),
                (named_only, DecisionVariable::continuous()),
                (removed_only, DecisionVariable::semi_integer()),
            ]))
            .fixed_decision_variable_values(BTreeMap::from([(fixed, 0.0)]))
            .constraints(BTreeMap::new())
            .removed_constraints(BTreeMap::from([(
                ConstraintID::from(10),
                (
                    Constraint::equal_to_zero(Function::from(linear!(removed_only))),
                    removed_reason,
                ),
            )]))
            .named_functions(BTreeMap::from([(
                NamedFunctionID::from(20),
                NamedFunction {
                    function: Function::from(linear!(named_only)),
                },
            )]))
            .decision_variable_dependency(
                AcyclicAssignments::new(vec![(
                    dependent,
                    Function::from(linear!(dependency_source)),
                )])
                .unwrap(),
            )
            .build()
            .unwrap();

        let facts = InstanceFacts::from(&instance);
        assert_eq!(facts.used_variable_ids(), BTreeSet::from([used]));
        assert_eq!(
            facts.used_variables_by_kind(),
            &BTreeMap::from([(Kind::Binary, BTreeSet::from([used]))])
        );
        assert!(facts.regular_constraints().is_empty());
    }
}
