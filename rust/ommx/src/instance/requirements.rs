use super::{Instance, Sense};
use crate::{
    ConstraintID, Degree, Equality, IndicatorConstraintID, Kind, OneHotConstraintID,
    Sos1ConstraintID, VariableIDSet,
};
use std::collections::{BTreeMap, BTreeSet};

/// Function shape required by one active regular or indicator constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstraintRequirement {
    relation: Equality,
    degree: Degree,
}

impl ConstraintRequirement {
    pub fn relation(&self) -> Equality {
        self.relation
    }

    pub fn degree(&self) -> Degree {
        self.degree
    }
}

/// Portable shape of the active solver input owned by an [`Instance`].
///
/// This is derived data, not another mutable source of truth. It includes the
/// objective and every active constraint family, and groups only solver-used
/// decision variables by [`Kind`]. Fixed, dependent, irrelevant,
/// removed-constraint-only, and named-function-only variables are excluded.
///
/// Serialization features are intentionally excluded. Wire-level
/// `ommx.v2.Feature` requirements and solver compatibility are separate domain
/// boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceRequirements {
    sense: Sense,
    used_variables_by_kind: BTreeMap<Kind, VariableIDSet>,
    objective_degree: Degree,
    regular_constraints: BTreeMap<ConstraintID, ConstraintRequirement>,
    indicator_constraints: BTreeMap<IndicatorConstraintID, ConstraintRequirement>,
    one_hot_constraint_ids: BTreeSet<OneHotConstraintID>,
    sos1_constraint_ids: BTreeSet<Sos1ConstraintID>,
}

impl InstanceRequirements {
    pub fn sense(&self) -> Sense {
        self.sense
    }

    pub fn used_variables_by_kind(&self) -> &BTreeMap<Kind, VariableIDSet> {
        &self.used_variables_by_kind
    }

    pub fn used_variable_ids(&self) -> VariableIDSet {
        self.used_variables_by_kind
            .values()
            .flat_map(|ids| ids.iter().copied())
            .collect()
    }

    pub fn objective_degree(&self) -> Degree {
        self.objective_degree
    }

    pub fn regular_constraints(&self) -> &BTreeMap<ConstraintID, ConstraintRequirement> {
        &self.regular_constraints
    }

    pub fn indicator_constraints(&self) -> &BTreeMap<IndicatorConstraintID, ConstraintRequirement> {
        &self.indicator_constraints
    }

    pub fn one_hot_constraint_ids(&self) -> &BTreeSet<OneHotConstraintID> {
        &self.one_hot_constraint_ids
    }

    pub fn sos1_constraint_ids(&self) -> &BTreeSet<Sos1ConstraintID> {
        &self.sos1_constraint_ids
    }
}

impl From<&Instance> for InstanceRequirements {
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
                        ConstraintRequirement {
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
                        ConstraintRequirement {
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

impl Instance {
    /// Derive the portable requirements of this instance's active solver input.
    ///
    /// The result is recomputed on every call so an explicit preparation or
    /// lowering step is always reflected by the next compatibility check.
    pub fn solver_requirements(&self) -> InstanceRequirements {
        self.into()
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
        fn requirements_match_every_active_solver_input_component(
            instance in any_with::<Instance>(InstanceParameters::full_v3())
        ) {
            let requirements = instance.solver_requirements();

            prop_assert_eq!(requirements.sense(), instance.sense());
            prop_assert_eq!(requirements.objective_degree(), instance.objective().degree());
            prop_assert_eq!(
                requirements.used_variable_ids(),
                instance.used_decision_variable_ids()
            );
            for (id, variable) in instance.used_decision_variables() {
                prop_assert!(requirements
                    .used_variables_by_kind()
                    .get(&variable.kind())
                    .is_some_and(|ids| ids.contains(&id)));
            }

            prop_assert_eq!(
                requirements.regular_constraints().keys().copied().collect::<BTreeSet<_>>(),
                instance.constraints().keys().copied().collect::<BTreeSet<_>>()
            );
            for (id, constraint) in instance.constraints() {
                let requirement = requirements.regular_constraints().get(id).unwrap();
                prop_assert_eq!(requirement.relation(), constraint.equality);
                prop_assert_eq!(requirement.degree(), constraint.function().degree());
            }

            prop_assert_eq!(
                requirements.indicator_constraints().keys().copied().collect::<BTreeSet<_>>(),
                instance.indicator_constraints().keys().copied().collect::<BTreeSet<_>>()
            );
            for (id, constraint) in instance.indicator_constraints() {
                let requirement = requirements.indicator_constraints().get(id).unwrap();
                prop_assert_eq!(requirement.relation(), constraint.equality);
                prop_assert_eq!(requirement.degree(), constraint.function().degree());
            }

            prop_assert_eq!(
                requirements.one_hot_constraint_ids(),
                &instance.one_hot_constraints().keys().copied().collect()
            );
            prop_assert_eq!(
                requirements.sos1_constraint_ids(),
                &instance.sos1_constraints().keys().copied().collect()
            );
        }
    }

    #[test]
    fn requirements_are_recomputed_after_explicit_lowering() {
        let x = VariableID::from(1);
        let y = VariableID::from(2);
        let one_hot_id = OneHotConstraintID::from(7);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from((linear!(x) + linear!(y)).unwrap()))
            .decision_variables(BTreeMap::from([
                (x, crate::DecisionVariable::binary()),
                (y, crate::DecisionVariable::binary()),
            ]))
            .constraints(BTreeMap::new())
            .one_hot_constraints(BTreeMap::from([(
                one_hot_id,
                OneHotConstraint::new(BTreeSet::from([x, y])).unwrap(),
            )]))
            .build()
            .unwrap();

        let before = instance.solver_requirements();
        assert_eq!(
            before.one_hot_constraint_ids(),
            &BTreeSet::from([one_hot_id])
        );
        assert!(before.regular_constraints().is_empty());

        let regular_id = instance.convert_one_hot_to_constraint(one_hot_id).unwrap();
        let after = instance.solver_requirements();
        assert!(after.one_hot_constraint_ids().is_empty());
        assert_eq!(
            after.regular_constraints().get(&regular_id),
            Some(&ConstraintRequirement {
                relation: Equality::EqualToZero,
                degree: 1.into(),
            })
        );
        assert_eq!(after.used_variable_ids(), before.used_variable_ids());
    }

    #[test]
    fn only_active_solver_input_variables_constrain_supported_kinds() {
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

        let requirements = instance.solver_requirements();
        assert_eq!(requirements.used_variable_ids(), BTreeSet::from([used]));
        assert_eq!(
            requirements.used_variables_by_kind(),
            &BTreeMap::from([(Kind::Binary, BTreeSet::from([used]))])
        );
        assert!(requirements.regular_constraints().is_empty());

        let binary_only = crate::CapabilityProfile::new(
            "binary-only",
            BTreeSet::from([Kind::Binary]),
            crate::DegreeLimit::Any,
            BTreeSet::from([Sense::Minimize]),
        )
        .unwrap();
        let report = crate::AdapterCapabilities::new(vec![binary_only])
            .unwrap()
            .check_compatibility(&requirements);
        assert!(report.is_compatible(), "{report}");
    }
}
