//! Private root-owned machinery shared by proof-carrying reductions.
//!
//! Nothing in this module is an SDK commitment. Family-specific inverse
//! lowering verifies semantics and selector isolation before constructing an
//! [`AssignmentMap`] or applying the narrow table effects defined elsewhere.

#![allow(dead_code)]

use super::Instance;
use crate::{v1, ConstraintID, Evaluate, Sos1ConstraintID, VariableID, VariableIDSet};
use std::collections::{BTreeMap, BTreeSet};

/// A composable map between the complete variable spaces of two Instance
/// representations.
///
/// Projection steps run in reduction order. Lifting runs the same steps in
/// reverse so eliminated private coordinates are reconstructed before an
/// earlier step depends on them. This transforms raw, unevaluated states;
/// callers must evaluate the result against the corresponding Instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AssignmentMap {
    source_ids: VariableIDSet,
    target_ids: VariableIDSet,
    steps: Vec<AssignmentStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AssignmentStep {
    Sos1Selectors(Sos1SelectorStep),
}

/// Complete member-to-selector roles for one checked SOS1 lowering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SelectorRole {
    /// The binary member itself was reused as its selector.
    Reused,
    /// A private binary selector was introduced for this member.
    Fresh(VariableID),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Sos1SelectorStep {
    source: Sos1ConstraintID,
    selectors: BTreeMap<VariableID, SelectorRole>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(super) enum AssignmentMapError {
    #[error("SOS1 constraint {constraint_id:?} has no member-to-selector roles")]
    EmptySos1Roles { constraint_id: Sos1ConstraintID },
    #[error("SOS1 member {member:?} is absent from the source variable space")]
    MissingSos1Member { member: VariableID },
    #[error("fresh SOS1 selector {selector:?} is absent from the source variable space")]
    MissingFreshSelector { selector: VariableID },
    #[error("fresh SOS1 selector {selector:?} collides with an SOS1 member")]
    SelectorMemberCollision { selector: VariableID },
    #[error("fresh SOS1 selector {selector:?} is assigned to more than one member")]
    DuplicateFreshSelector { selector: VariableID },
    #[error("assignment maps have different intermediate variable spaces")]
    IntermediateVariableSpaceMismatch,
    #[error("{side} state has a different variable-ID set from its assignment map")]
    StateVariableSpaceMismatch { side: &'static str },
    #[error("state value for variable {id:?} is not finite")]
    NonFiniteStateValue { id: VariableID },
    #[error("assignment-map step expected variable {id:?} to be present")]
    MissingIntermediateVariable { id: VariableID },
    #[error("assignment-map step would overwrite variable {id:?}")]
    IntermediateVariableCollision { id: VariableID },
}

impl AssignmentMap {
    pub(super) fn identity(ids: VariableIDSet) -> Self {
        Self {
            source_ids: ids.clone(),
            target_ids: ids,
            steps: Vec::new(),
        }
    }

    pub(super) fn sos1_selectors(
        source_ids: VariableIDSet,
        source: Sos1ConstraintID,
        selectors: BTreeMap<VariableID, SelectorRole>,
    ) -> Result<Self, AssignmentMapError> {
        if selectors.is_empty() {
            return Err(AssignmentMapError::EmptySos1Roles {
                constraint_id: source,
            });
        }

        let members = selectors.keys().copied().collect::<VariableIDSet>();
        let mut fresh = BTreeSet::new();
        for (&member, &role) in &selectors {
            if !source_ids.contains(&member) {
                return Err(AssignmentMapError::MissingSos1Member { member });
            }
            let SelectorRole::Fresh(selector) = role else {
                continue;
            };
            if !source_ids.contains(&selector) {
                return Err(AssignmentMapError::MissingFreshSelector { selector });
            }
            if members.contains(&selector) {
                return Err(AssignmentMapError::SelectorMemberCollision { selector });
            }
            if !fresh.insert(selector) {
                return Err(AssignmentMapError::DuplicateFreshSelector { selector });
            }
        }

        if fresh.is_empty() {
            return Ok(Self::identity(source_ids));
        }
        let target_ids = source_ids.difference(&fresh).copied().collect();
        Ok(Self {
            source_ids,
            target_ids,
            steps: vec![AssignmentStep::Sos1Selectors(Sos1SelectorStep {
                source,
                selectors,
            })],
        })
    }

    /// Compose `self : P -> Q` with `next : Q -> R`.
    pub(super) fn then(mut self, next: Self) -> Result<Self, AssignmentMapError> {
        if self.target_ids != next.source_ids {
            return Err(AssignmentMapError::IntermediateVariableSpaceMismatch);
        }
        self.target_ids = next.target_ids;
        self.steps.extend(next.steps);
        Ok(self)
    }

    pub(super) fn source_ids(&self) -> &VariableIDSet {
        &self.source_ids
    }

    pub(super) fn target_ids(&self) -> &VariableIDSet {
        &self.target_ids
    }

    pub(super) fn project_state(
        &self,
        source: &v1::State,
    ) -> Result<v1::State, AssignmentMapError> {
        validate_state(source, &self.source_ids, "source")?;
        let mut entries = source.entries.clone();
        for step in &self.steps {
            match step {
                AssignmentStep::Sos1Selectors(step) => step.project(&mut entries)?,
            }
        }
        let target = v1::State { entries };
        validate_state(&target, &self.target_ids, "target")?;
        Ok(target)
    }

    pub(super) fn lift_state(&self, target: &v1::State) -> Result<v1::State, AssignmentMapError> {
        validate_state(target, &self.target_ids, "target")?;
        let mut entries = target.entries.clone();
        for step in self.steps.iter().rev() {
            match step {
                AssignmentStep::Sos1Selectors(step) => step.lift(&mut entries)?,
            }
        }
        let source = v1::State { entries };
        validate_state(&source, &self.source_ids, "source")?;
        Ok(source)
    }
}

impl Sos1SelectorStep {
    fn project(
        &self,
        entries: &mut std::collections::HashMap<u64, f64>,
    ) -> Result<(), AssignmentMapError> {
        for (&member, &role) in &self.selectors {
            if !entries.contains_key(&member.into_inner()) {
                return Err(AssignmentMapError::MissingIntermediateVariable { id: member });
            }
            if let SelectorRole::Fresh(selector) = role {
                if entries.remove(&selector.into_inner()).is_none() {
                    return Err(AssignmentMapError::MissingIntermediateVariable { id: selector });
                }
            }
        }
        Ok(())
    }

    fn lift(
        &self,
        entries: &mut std::collections::HashMap<u64, f64>,
    ) -> Result<(), AssignmentMapError> {
        for (&member, &role) in &self.selectors {
            let value = entries
                .get(&member.into_inner())
                .copied()
                .ok_or(AssignmentMapError::MissingIntermediateVariable { id: member })?;
            if let SelectorRole::Fresh(selector) = role {
                let selector_value = if value == 0.0 { 0.0 } else { 1.0 };
                if entries
                    .insert(selector.into_inner(), selector_value)
                    .is_some()
                {
                    return Err(AssignmentMapError::IntermediateVariableCollision { id: selector });
                }
            }
        }
        Ok(())
    }
}

fn validate_state(
    state: &v1::State,
    expected_ids: &VariableIDSet,
    side: &'static str,
) -> Result<(), AssignmentMapError> {
    let actual_ids = state
        .entries
        .keys()
        .copied()
        .map(VariableID::from)
        .collect::<VariableIDSet>();
    if &actual_ids != expected_ids {
        return Err(AssignmentMapError::StateVariableSpaceMismatch { side });
    }
    for (&id, &value) in &state.entries {
        if !value.is_finite() {
            return Err(AssignmentMapError::NonFiniteStateValue {
                id: VariableID::from(id),
            });
        }
    }
    Ok(())
}

impl Instance {
    /// Verify that root-owned variable coordinates can be removed after the
    /// listed active regular rows are consumed.
    ///
    /// This is intentionally exhaustive rather than using
    /// `DecisionVariableUsage`, whose index covers only current solver input.
    /// Metadata labels are not semantic uses and are removed with their rows.
    pub(super) fn ensure_variables_isolated_for_removal(
        &self,
        private_ids: &VariableIDSet,
        consumed_regular_rows: &BTreeSet<ConstraintID>,
    ) -> crate::Result<()> {
        for id in private_ids {
            if !self.decision_variables.contains_key(id) {
                crate::bail!({ ?id }, "Private decision variable {id:?} is not registered");
            }
        }
        for id in consumed_regular_rows {
            if !self.constraints().contains_key(id) {
                crate::bail!({ ?id }, "Consumed regular constraint {id:?} is not active");
            }
        }

        reject_required_ids(private_ids, &self.objective.required_ids(), "the objective")?;

        for (id, constraint) in self.constraints() {
            if !consumed_regular_rows.contains(id) {
                reject_required_ids(
                    private_ids,
                    &constraint.required_ids(),
                    &format!("active regular constraint {id:?}"),
                )?;
            }
        }
        for (id, (constraint, _)) in self.removed_constraints() {
            reject_required_ids(
                private_ids,
                &constraint.required_ids(),
                &format!("removed regular constraint {id:?}"),
            )?;
        }
        for (id, constraint) in self.indicator_constraints() {
            reject_required_ids(
                private_ids,
                &constraint.required_ids(),
                &format!("active Indicator constraint {id:?}"),
            )?;
        }
        for (id, (constraint, _)) in self.removed_indicator_constraints() {
            reject_required_ids(
                private_ids,
                &constraint.required_ids(),
                &format!("removed Indicator constraint {id:?}"),
            )?;
        }
        for (id, constraint) in self.one_hot_constraints() {
            reject_required_ids(
                private_ids,
                &constraint.required_ids(),
                &format!("active OneHot constraint {id:?}"),
            )?;
        }
        for (id, (constraint, _)) in self.removed_one_hot_constraints() {
            reject_required_ids(
                private_ids,
                &constraint.required_ids(),
                &format!("removed OneHot constraint {id:?}"),
            )?;
        }
        for (id, constraint) in self.sos1_constraints() {
            reject_required_ids(
                private_ids,
                &constraint.required_ids(),
                &format!("active SOS1 constraint {id:?}"),
            )?;
        }
        for (id, (constraint, _)) in self.removed_sos1_constraints() {
            reject_required_ids(
                private_ids,
                &constraint.required_ids(),
                &format!("removed SOS1 constraint {id:?}"),
            )?;
        }
        for (id, named) in self.named_functions() {
            reject_required_ids(
                private_ids,
                &named.function.required_ids(),
                &format!("named function {id:?}"),
            )?;
        }
        for (id, function) in self.decision_variable_dependency().iter() {
            if private_ids.contains(id) {
                crate::bail!(
                    { ?id },
                    "Private decision variable {id:?} is a dependency target"
                );
            }
            reject_required_ids(
                private_ids,
                &function.required_ids(),
                &format!("decision-variable dependency {id:?}"),
            )?;
        }
        if let Some(id) = private_ids
            .iter()
            .find(|id| self.fixed_decision_variable_values().contains_key(id))
        {
            crate::bail!({ ?id }, "Private decision variable {id:?} is fixed");
        }
        Ok(())
    }
}

fn reject_required_ids(
    private_ids: &VariableIDSet,
    required_ids: &VariableIDSet,
    location: &str,
) -> crate::Result<()> {
    if let Some(id) = private_ids.intersection(required_ids).next() {
        crate::bail!({ ?id, location }, "Private decision variable {id:?} is used by {location}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        linear, AcyclicAssignments, Constraint, DecisionVariable, Equality, Function,
        IndicatorConstraint, Instance, NamedFunction, NamedFunctionID, OneHotConstraint, Sense,
        Sos1Constraint,
    };
    use std::collections::{BTreeMap, HashMap};

    fn ids(values: impl IntoIterator<Item = u64>) -> VariableIDSet {
        values.into_iter().map(VariableID::from).collect()
    }

    fn state(entries: impl IntoIterator<Item = (u64, f64)>) -> v1::State {
        v1::State {
            entries: entries.into_iter().collect::<HashMap<_, _>>(),
        }
    }

    #[test]
    fn identity_map_preserves_the_complete_state() {
        let map = AssignmentMap::identity(ids([1, 2]));
        let original = state([(1, -0.0), (2, 3.0)]);
        let projected = map.project_state(&original).unwrap();
        let lifted = map.lift_state(&projected).unwrap();
        assert_eq!(projected.entries[&1].to_bits(), (-0.0f64).to_bits());
        assert_eq!(lifted.entries[&1].to_bits(), (-0.0f64).to_bits());
        assert_eq!(lifted, original);
    }

    fn mixed_sos1_map() -> AssignmentMap {
        AssignmentMap::sos1_selectors(
            ids([1, 2, 10]),
            Sos1ConstraintID::from(7),
            BTreeMap::from([
                (
                    VariableID::from(1),
                    SelectorRole::Fresh(VariableID::from(10)),
                ),
                (VariableID::from(2), SelectorRole::Reused),
            ]),
        )
        .unwrap()
    }

    #[test]
    fn sos1_projection_removes_only_fresh_selectors() {
        let map = mixed_sos1_map();
        let source = state([(1, 0.0), (2, 0.0), (10, 1.0)]);
        let projected = map.project_state(&source).unwrap();
        assert_eq!(projected, state([(1, 0.0), (2, 0.0)]));

        let canonical = map.lift_state(&projected).unwrap();
        assert_eq!(canonical.entries[&10], 0.0);
        assert_ne!(canonical, source);
    }

    #[test]
    fn sos1_lift_uses_exact_zero_without_becoming_a_feasibility_oracle() {
        let map = mixed_sos1_map();
        for (member, expected) in [
            (0.0, 0.0),
            (-0.0, 0.0),
            (f64::from_bits(1), 1.0),
            (-f64::from_bits(1), 1.0),
        ] {
            let lifted = map.lift_state(&state([(1, member), (2, 0.0)])).unwrap();
            assert_eq!(lifted.entries[&10], expected);
        }

        // The map is total on finite complete states. Feasibility is checked by
        // evaluating the lifted state against the lowered Instance.
        let multiple = map.lift_state(&state([(1, 2.0), (2, 1.0)])).unwrap();
        assert_eq!(multiple.entries[&10], 1.0);
    }

    #[test]
    fn sos1_map_satisfies_the_projection_section_law() {
        let map = mixed_sos1_map();
        for target in [
            state([(1, 0.0), (2, 0.0)]),
            state([(1, 2.0), (2, 0.0)]),
            state([(1, 2.0), (2, 1.0)]),
        ] {
            assert_eq!(
                map.project_state(&map.lift_state(&target).unwrap())
                    .unwrap(),
                target
            );
        }
    }

    #[test]
    fn assignment_map_rejects_stale_or_nonfinite_states() {
        let map = mixed_sos1_map();
        assert!(matches!(
            map.project_state(&state([(1, 0.0), (2, 0.0)])),
            Err(AssignmentMapError::StateVariableSpaceMismatch { side: "source" })
        ));
        assert!(matches!(
            map.lift_state(&state([(1, 0.0), (2, 0.0), (10, 0.0)])),
            Err(AssignmentMapError::StateVariableSpaceMismatch { side: "target" })
        ));
        assert!(matches!(
            map.lift_state(&state([(1, f64::NAN), (2, 0.0)])),
            Err(AssignmentMapError::NonFiniteStateValue { id }) if id == VariableID::from(1)
        ));
        assert!(matches!(
            map.project_state(&state([(1, 0.0), (2, 0.0), (10, f64::INFINITY)])),
            Err(AssignmentMapError::NonFiniteStateValue { id }) if id == VariableID::from(10)
        ));
    }

    #[test]
    fn assignment_map_validates_selector_roles() {
        let source = ids([1, 2, 10]);
        assert!(matches!(
            AssignmentMap::sos1_selectors(
                source.clone(),
                Sos1ConstraintID::from(7),
                BTreeMap::new(),
            ),
            Err(AssignmentMapError::EmptySos1Roles { .. })
        ));
        assert!(matches!(
            AssignmentMap::sos1_selectors(
                source.clone(),
                Sos1ConstraintID::from(7),
                BTreeMap::from([(VariableID::from(99), SelectorRole::Reused)]),
            ),
            Err(AssignmentMapError::MissingSos1Member { member })
                if member == VariableID::from(99)
        ));
        assert!(matches!(
            AssignmentMap::sos1_selectors(
                source.clone(),
                Sos1ConstraintID::from(7),
                BTreeMap::from([(
                    VariableID::from(1),
                    SelectorRole::Fresh(VariableID::from(99)),
                )]),
            ),
            Err(AssignmentMapError::MissingFreshSelector { selector })
                if selector == VariableID::from(99)
        ));
        assert!(matches!(
            AssignmentMap::sos1_selectors(
                source.clone(),
                Sos1ConstraintID::from(7),
                BTreeMap::from([(
                    VariableID::from(1),
                    SelectorRole::Fresh(VariableID::from(2)),
                )]),
            ),
            Ok(_)
        ));
        assert!(matches!(
            AssignmentMap::sos1_selectors(
                source.clone(),
                Sos1ConstraintID::from(7),
                BTreeMap::from([
                    (
                        VariableID::from(1),
                        SelectorRole::Fresh(VariableID::from(10))
                    ),
                    (
                        VariableID::from(2),
                        SelectorRole::Fresh(VariableID::from(10))
                    ),
                ]),
            ),
            Err(AssignmentMapError::DuplicateFreshSelector { .. })
        ));
        assert!(matches!(
            AssignmentMap::sos1_selectors(
                source,
                Sos1ConstraintID::from(7),
                BTreeMap::from([
                    (
                        VariableID::from(1),
                        SelectorRole::Fresh(VariableID::from(2))
                    ),
                    (VariableID::from(2), SelectorRole::Reused),
                ]),
            ),
            Err(AssignmentMapError::SelectorMemberCollision { .. })
        ));

        let all_reused = AssignmentMap::sos1_selectors(
            ids([1, 2]),
            Sos1ConstraintID::from(7),
            BTreeMap::from([
                (VariableID::from(1), SelectorRole::Reused),
                (VariableID::from(2), SelectorRole::Reused),
            ]),
        )
        .unwrap();
        assert_eq!(all_reused, AssignmentMap::identity(ids([1, 2])));
    }

    #[test]
    fn assignment_maps_compose_projection_forward_and_lift_backward() {
        let first = AssignmentMap::sos1_selectors(
            ids([1, 2, 3]),
            Sos1ConstraintID::from(1),
            BTreeMap::from([(
                VariableID::from(1),
                SelectorRole::Fresh(VariableID::from(2)),
            )]),
        )
        .unwrap();
        let second = AssignmentMap::sos1_selectors(
            ids([1, 3]),
            Sos1ConstraintID::from(2),
            BTreeMap::from([(
                VariableID::from(3),
                SelectorRole::Fresh(VariableID::from(1)),
            )]),
        )
        .unwrap();
        let composed = first.then(second).unwrap();

        let target = state([(3, 4.0)]);
        let lifted = composed.lift_state(&target).unwrap();
        assert_eq!(lifted, state([(1, 1.0), (2, 1.0), (3, 4.0)]));
        assert_eq!(composed.project_state(&lifted).unwrap(), target);
    }

    #[test]
    fn identity_composition_is_neutral_and_spaces_must_match() {
        let map = mixed_sos1_map();
        assert_eq!(
            AssignmentMap::identity(map.source_ids().clone())
                .then(map.clone())
                .unwrap(),
            map
        );
        assert_eq!(
            map.clone()
                .then(AssignmentMap::identity(map.target_ids().clone()))
                .unwrap(),
            map
        );
        assert!(matches!(
            map.then(AssignmentMap::identity(ids([999]))),
            Err(AssignmentMapError::IntermediateVariableSpaceMismatch)
        ));
    }

    fn isolation_base() -> Instance {
        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::zero())
            .decision_variables(BTreeMap::from([
                (VariableID::from(1), DecisionVariable::continuous()),
                (VariableID::from(50), DecisionVariable::continuous()),
                (VariableID::from(99), DecisionVariable::binary()),
            ]))
            .constraints(BTreeMap::from([(
                ConstraintID::from(10),
                Constraint::less_than_or_equal_to_zero(Function::from(linear!(99))),
            )]))
            .build()
            .unwrap()
    }

    #[test]
    fn isolation_allows_uses_only_in_consumed_regular_rows() {
        isolation_base()
            .ensure_variables_isolated_for_removal(
                &ids([99]),
                &BTreeSet::from([ConstraintID::from(10)]),
            )
            .unwrap();
    }

    #[test]
    fn isolation_rejects_objective_regular_named_and_dependency_uses() {
        let private = ids([99]);
        let consumed = BTreeSet::from([ConstraintID::from(10)]);

        let mut objective = isolation_base();
        objective.objective = Function::from(linear!(99));
        assert!(objective
            .ensure_variables_isolated_for_removal(&private, &consumed)
            .is_err());

        let mut regular = isolation_base();
        regular
            .constraint_collection
            .insert_active_with_context(
                ConstraintID::from(11),
                Constraint::less_than_or_equal_to_zero(Function::from(linear!(99))),
                Default::default(),
            )
            .unwrap();
        assert!(regular
            .ensure_variables_isolated_for_removal(&private, &consumed)
            .is_err());

        let mut named = isolation_base();
        named.named_functions = crate::NamedFunctionTable::from_entries(BTreeMap::from([(
            NamedFunctionID::from(1),
            NamedFunction {
                function: Function::from(linear!(99)),
            },
        )]));
        assert!(named
            .ensure_variables_isolated_for_removal(&private, &consumed)
            .is_err());

        let mut dependency = isolation_base();
        dependency.decision_variable_dependency =
            AcyclicAssignments::new([(VariableID::from(50), Function::from(linear!(99)))]).unwrap();
        assert!(dependency
            .ensure_variables_isolated_for_removal(&private, &consumed)
            .is_err());

        let mut dependency_target = isolation_base();
        dependency_target.decision_variable_dependency =
            AcyclicAssignments::new([(VariableID::from(99), Function::zero())]).unwrap();
        assert!(dependency_target
            .ensure_variables_isolated_for_removal(&private, &consumed)
            .is_err());
    }

    #[test]
    fn isolation_rejects_removed_regular_and_fixed_uses() {
        let private = ids([99]);
        let consumed = BTreeSet::from([ConstraintID::from(10)]);

        let mut removed = isolation_base();
        removed
            .constraint_collection
            .insert_active_with_context(
                ConstraintID::from(11),
                Constraint::less_than_or_equal_to_zero(Function::from(linear!(99))),
                Default::default(),
            )
            .unwrap();
        removed
            .constraint_collection
            .relax(
                ConstraintID::from(11),
                crate::constraint::RemovedReason {
                    reason: "test".to_string(),
                    parameters: Default::default(),
                },
            )
            .unwrap();
        assert!(removed
            .ensure_variables_isolated_for_removal(&private, &consumed)
            .is_err());

        let mut fixed = isolation_base();
        fixed
            .decision_variables
            .set_fixed_value(VariableID::from(99), 0.0, crate::ATol::default())
            .unwrap();
        assert!(fixed
            .ensure_variables_isolated_for_removal(&private, &consumed)
            .is_err());
    }

    #[test]
    fn isolation_rejects_an_unconsumed_or_unknown_regular_row() {
        let instance = isolation_base();
        assert!(instance
            .ensure_variables_isolated_for_removal(
                &ids([404]),
                &BTreeSet::from([ConstraintID::from(10)]),
            )
            .is_err());
        assert!(instance
            .ensure_variables_isolated_for_removal(&ids([99]), &BTreeSet::new())
            .is_err());
        assert!(instance
            .ensure_variables_isolated_for_removal(
                &ids([99]),
                &BTreeSet::from([ConstraintID::from(404)]),
            )
            .is_err());
    }

    fn removed_reason() -> crate::constraint::RemovedReason {
        crate::constraint::RemovedReason {
            reason: "test".to_string(),
            parameters: Default::default(),
        }
    }

    #[test]
    fn isolation_rejects_active_and_removed_special_constraint_uses() {
        let private = ids([99]);
        let consumed = BTreeSet::from([ConstraintID::from(10)]);

        let mut active_indicator = isolation_base();
        active_indicator
            .add_indicator_constraint(
                IndicatorConstraint::new(
                    VariableID::from(99),
                    Equality::LessThanOrEqualToZero,
                    Function::zero(),
                ),
                Default::default(),
            )
            .unwrap();
        assert!(active_indicator
            .ensure_variables_isolated_for_removal(&private, &consumed)
            .is_err());

        let mut removed_indicator = isolation_base();
        let id = removed_indicator
            .add_indicator_constraint(
                IndicatorConstraint::new(
                    VariableID::from(99),
                    Equality::LessThanOrEqualToZero,
                    Function::zero(),
                ),
                Default::default(),
            )
            .unwrap();
        removed_indicator
            .indicator_constraint_collection
            .relax(id, removed_reason())
            .unwrap();
        assert!(removed_indicator
            .ensure_variables_isolated_for_removal(&private, &consumed)
            .is_err());

        let mut active_one_hot = isolation_base();
        active_one_hot
            .add_one_hot_constraint(
                OneHotConstraint::new(BTreeSet::from([VariableID::from(99)])).unwrap(),
                Default::default(),
            )
            .unwrap();
        assert!(active_one_hot
            .ensure_variables_isolated_for_removal(&private, &consumed)
            .is_err());

        let mut removed_one_hot = isolation_base();
        let id = removed_one_hot
            .add_one_hot_constraint(
                OneHotConstraint::new(BTreeSet::from([VariableID::from(99)])).unwrap(),
                Default::default(),
            )
            .unwrap();
        removed_one_hot
            .one_hot_constraint_collection
            .relax(id, removed_reason())
            .unwrap();
        assert!(removed_one_hot
            .ensure_variables_isolated_for_removal(&private, &consumed)
            .is_err());

        let mut active_sos1 = isolation_base();
        active_sos1
            .add_sos1_constraint(
                Sos1Constraint::new(BTreeSet::from([VariableID::from(99)])).unwrap(),
                Default::default(),
            )
            .unwrap();
        assert!(active_sos1
            .ensure_variables_isolated_for_removal(&private, &consumed)
            .is_err());

        let mut removed_sos1 = isolation_base();
        let id = removed_sos1
            .add_sos1_constraint(
                Sos1Constraint::new(BTreeSet::from([VariableID::from(99)])).unwrap(),
                Default::default(),
            )
            .unwrap();
        removed_sos1
            .sos1_constraint_collection
            .relax(id, removed_reason())
            .unwrap();
        assert!(removed_sos1
            .ensure_variables_isolated_for_removal(&private, &consumed)
            .is_err());
    }
}
