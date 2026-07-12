//! Crate-private end-to-end coordination for checked inverse lowering.
//!
//! The generic SDK surface remains deliberately unfrozen. This module proves
//! that the family-specific Indicator and SOS1 consumers compose at the root
//! `Instance` boundary without making the result/map vocabulary public.
//! General lifecycle reactivation such as `restore_indicator_constraint`
//! remains a distinct, potentially semantics-changing operation and is not an
//! alias for this proof-preserving inverse.

#![allow(dead_code)]

use super::{
    reduction::AssignmentMap, AdditionalCapability, Capabilities, Instance,
    INDICATOR_LOWERING_REASON, SOS1_LOWERING_REASON,
};
use crate::{v1, VariableIDSet};

/// Result of one checked, root-owned inverse-lowering batch.
///
/// The map transforms complete raw states from the Instance representation
/// before the call to the representation after the call. It is an exact
/// mathematical state map, not a tolerance-based feasibility classifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct InverseLoweringResult {
    restored_capabilities: Capabilities,
    assignment_map: AssignmentMap,
}

impl InverseLoweringResult {
    pub(super) fn restored_capabilities(&self) -> &Capabilities {
        &self.restored_capabilities
    }

    pub(super) fn before_variable_ids(&self) -> &VariableIDSet {
        self.assignment_map.source_ids()
    }

    pub(super) fn after_variable_ids(&self) -> &VariableIDSet {
        self.assignment_map.target_ids()
    }

    /// Project a complete finite state of the pre-inverse lowered Instance to
    /// the post-inverse restored Instance.
    pub(super) fn project_state(&self, before: &v1::State) -> crate::Result<v1::State> {
        Ok(self.assignment_map.project_state(before)?)
    }

    /// Lift a complete finite state of the post-inverse restored Instance to
    /// the pre-inverse lowered Instance.
    ///
    /// Fresh SOS1 selectors use mathematical exact zero. Callers must evaluate
    /// the returned state against the corresponding Instance; this method does
    /// not promise preservation of every `Evaluate(ATol)` classification.
    pub(super) fn lift_state(&self, after: &v1::State) -> crate::Result<v1::State> {
        Ok(self.assignment_map.lift_state(after)?)
    }
}

impl Instance {
    /// Restore every current OMMX V1 Indicator/SOS1 lowering history in the
    /// requested families, or leave the Instance entirely unchanged.
    ///
    /// `requested` is both a family filter and explicit permission to add that
    /// semantic capability. Ordinary lifecycle removals with other reasons are
    /// ignored. Once an exact OMMX lowering reason selects a history, any
    /// malformed parameter, row, provenance, selector, or use is a hard error
    /// for the complete batch rather than a skipped candidate.
    ///
    /// OneHot is intentionally unsupported by this private V1 integration and
    /// is rejected before any work. Public generic naming, serialized receipts,
    /// and Python bindings remain deferred.
    pub(super) fn restore_lowered_capabilities_checked(
        &mut self,
        requested: &Capabilities,
    ) -> crate::Result<InverseLoweringResult> {
        if requested.contains(&AdditionalCapability::OneHot) {
            crate::bail!("Checked inverse lowering does not yet support the OneHot capability");
        }

        let mut indicator_ids = if requested.contains(&AdditionalCapability::Indicator) {
            self.removed_indicator_constraints()
                .iter()
                .filter_map(|(&id, (_, reason))| {
                    (reason.reason == INDICATOR_LOWERING_REASON).then_some(id)
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let mut sos1_ids = if requested.contains(&AdditionalCapability::Sos1) {
            self.removed_sos1_constraints()
                .iter()
                .filter_map(|(&id, (_, reason))| {
                    (reason.reason == SOS1_LOWERING_REASON).then_some(id)
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        let mut assignment_map =
            AssignmentMap::identity(self.decision_variables.keys().copied().collect());
        if indicator_ids.is_empty() && sos1_ids.is_empty() {
            return Ok(InverseLoweringResult {
                restored_capabilities: Capabilities::new(),
                assignment_map,
            });
        }

        // `reduce_capabilities` lowers Indicator before SOS1, and each family
        // lowers IDs in ascending order. Undo that deterministic stack in
        // reverse. Every family operation commits only to this staged clone;
        // the caller-visible root is replaced once after the whole batch.
        indicator_ids.reverse();
        sos1_ids.reverse();
        let mut staged = self.clone();
        let mut restored_capabilities = Capabilities::new();

        for id in sos1_ids {
            let step = staged.restore_sos1_from_lowering_checked(id, requested)?;
            assignment_map = assignment_map.then(step)?;
            restored_capabilities.insert(AdditionalCapability::Sos1);
        }
        for id in indicator_ids {
            let step = staged.restore_indicator_from_lowering_checked(id, requested)?;
            assignment_map = assignment_map.then(step)?;
            restored_capabilities.insert(AdditionalCapability::Indicator);
        }

        let staged_variable_ids = staged
            .decision_variables
            .keys()
            .copied()
            .collect::<VariableIDSet>();
        debug_assert_eq!(assignment_map.target_ids(), &staged_variable_ids);
        *self = staged;
        Ok(InverseLoweringResult {
            restored_capabilities,
            assignment_map,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, linear, Bound, Constraint, DecisionVariable, Equality, Evaluate, Function,
        IndicatorConstraint, Kind, OneHotConstraint, Sense, Sos1Constraint, VariableID,
    };
    use maplit::btreemap;
    use std::collections::{BTreeMap, BTreeSet};

    fn requested(values: impl IntoIterator<Item = AdditionalCapability>) -> Capabilities {
        values.into_iter().collect()
    }

    fn combined_instance() -> Instance {
        let bounded = DecisionVariable::new(
            Kind::Continuous,
            Bound::new(0.0, 5.0).unwrap(),
            crate::ATol::default(),
        )
        .unwrap();
        let integer = DecisionVariable::new(
            Kind::Integer,
            Bound::new(-2.0, 3.0).unwrap(),
            crate::ATol::default(),
        )
        .unwrap();
        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(
                ((linear!(1) + linear!(2)).unwrap() + linear!(3)).unwrap(),
            ))
            .decision_variables(btreemap! {
                VariableID::from(0) => DecisionVariable::binary(),
                VariableID::from(1) => bounded,
                VariableID::from(2) => DecisionVariable::binary(),
                VariableID::from(3) => integer,
            })
            .constraints(BTreeMap::new())
            .indicator_constraints(BTreeMap::from([(
                crate::IndicatorConstraintID::from(7),
                IndicatorConstraint::new(
                    VariableID::from(0),
                    Equality::LessThanOrEqualToZero,
                    Function::from((linear!(1) + coeff!(-2.0)).unwrap()),
                ),
            )]))
            .sos1_constraints(BTreeMap::from([(
                crate::Sos1ConstraintID::from(9),
                Sos1Constraint::new(BTreeSet::from([VariableID::from(2), VariableID::from(3)]))
                    .unwrap(),
            )]))
            .build()
            .unwrap()
    }

    fn first_recorded_constraint_id(reason: &crate::RemovedReason) -> crate::ConstraintID {
        let first = reason.parameters[super::super::GENERATED_CONSTRAINT_IDS_PARAMETER]
            .split(',')
            .next()
            .expect("current lowering records at least one generated row")
            .parse::<u64>()
            .unwrap();
        crate::ConstraintID::from(first)
    }

    #[test]
    fn restores_reduce_capabilities_indicator_and_sos1_end_to_end() {
        let mut instance = combined_instance();
        let original = instance.clone();
        let original_bytes = instance.to_v2_bytes();
        assert_eq!(
            instance.reduce_capabilities(&Capabilities::new()).unwrap(),
            requested([AdditionalCapability::Indicator, AdditionalCapability::Sos1,])
        );
        let before_variable_ids = instance
            .decision_variables()
            .keys()
            .copied()
            .collect::<VariableIDSet>();
        let lowered = instance.clone();
        let selector = *before_variable_ids
            .difference(&original.decision_variables().keys().copied().collect())
            .next()
            .expect("SOS1 lowering introduced one fresh selector");

        let result = instance
            .restore_lowered_capabilities_checked(&requested([
                AdditionalCapability::Indicator,
                AdditionalCapability::Sos1,
            ]))
            .unwrap();

        assert_eq!(instance, original);
        assert_eq!(instance.to_v2_bytes(), original_bytes);
        assert_eq!(
            result.restored_capabilities(),
            &requested([AdditionalCapability::Indicator, AdditionalCapability::Sos1,])
        );
        assert_eq!(result.before_variable_ids(), &before_variable_ids);
        assert_eq!(
            result.after_variable_ids(),
            &instance
                .decision_variables()
                .keys()
                .copied()
                .collect::<VariableIDSet>()
        );

        let before = v1::State::from_iter([
            (0, 1.0),
            (1, 2.0),
            (2, 0.0),
            (3, 1.0),
            (selector.into_inner(), 1.0),
        ]);
        let after = result.project_state(&before).unwrap();
        assert_eq!(after.entries.get(&selector.into_inner()), None);
        assert_eq!(result.lift_state(&after).unwrap(), before);
        assert_eq!(
            result
                .project_state(&result.lift_state(&after).unwrap())
                .unwrap(),
            after
        );
        let before_solution = lowered.evaluate(&before, crate::ATol::default()).unwrap();
        let after_solution = instance.evaluate(&after, crate::ATol::default()).unwrap();
        assert_eq!(before_solution.feasible(), after_solution.feasible());
        assert_eq!(before_solution.objective(), after_solution.objective());
    }

    #[test]
    fn requested_family_filter_leaves_other_lowering_untouched() {
        let mut instance = combined_instance();
        instance.reduce_capabilities(&Capabilities::new()).unwrap();
        let removed_indicator = instance.removed_indicator_constraints().clone();

        let result = instance
            .restore_lowered_capabilities_checked(&requested([AdditionalCapability::Sos1]))
            .unwrap();

        assert_eq!(
            result.restored_capabilities(),
            &requested([AdditionalCapability::Sos1])
        );
        assert_eq!(instance.removed_indicator_constraints(), &removed_indicator);
        assert!(instance.indicator_constraints().is_empty());
        assert_eq!(
            instance.required_capabilities(),
            requested([AdditionalCapability::Sos1])
        );
    }

    #[test]
    fn cross_family_failure_rolls_back_earlier_staged_restoration() {
        let mut instance = combined_instance();
        instance.reduce_capabilities(&Capabilities::new()).unwrap();
        let (_, reason) =
            &instance.removed_indicator_constraints()[&crate::IndicatorConstraintID::from(7)];
        let indicator_row = first_recorded_constraint_id(reason);
        instance
            .relax_constraint(indicator_row, "test corruption".to_string(), [])
            .unwrap();
        let before = instance.clone();
        let before_bytes = instance.to_v2_bytes();

        let error = instance
            .restore_lowered_capabilities_checked(&requested([
                AdditionalCapability::Indicator,
                AdditionalCapability::Sos1,
            ]))
            .unwrap_err();

        assert!(error.to_string().contains("is not active"));
        assert_eq!(instance, before);
        assert_eq!(instance.to_v2_bytes(), before_bytes);
    }

    #[test]
    fn two_sos1_maps_compose_in_reverse_lowering_order() {
        let variables = btreemap! {
            VariableID::from(0) => DecisionVariable::new(
                Kind::Continuous,
                Bound::new(-1.0, 2.0).unwrap(),
                crate::ATol::default(),
            ).unwrap(),
            VariableID::from(1) => DecisionVariable::new(
                Kind::Continuous,
                Bound::new(-3.0, 4.0).unwrap(),
                crate::ATol::default(),
            ).unwrap(),
        };
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from((linear!(0) + linear!(1)).unwrap()))
            .decision_variables(variables)
            .constraints(BTreeMap::new())
            .sos1_constraints(BTreeMap::from([
                (
                    crate::Sos1ConstraintID::from(1),
                    Sos1Constraint::new(BTreeSet::from([VariableID::from(0)])).unwrap(),
                ),
                (
                    crate::Sos1ConstraintID::from(2),
                    Sos1Constraint::new(BTreeSet::from([VariableID::from(1)])).unwrap(),
                ),
            ]))
            .build()
            .unwrap();
        let original = instance.clone();
        instance.reduce_capabilities(&Capabilities::new()).unwrap();
        let source_ids = instance
            .decision_variables()
            .keys()
            .copied()
            .collect::<VariableIDSet>();
        let selectors = source_ids
            .difference(&original.decision_variables().keys().copied().collect())
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(selectors.len(), 2);

        let result = instance
            .restore_lowered_capabilities_checked(&requested([AdditionalCapability::Sos1]))
            .unwrap();
        assert_eq!(instance, original);

        let after = v1::State::from_iter([(0, 2.0), (1, 0.0)]);
        let before = result.lift_state(&after).unwrap();
        assert_eq!(before.entries[&selectors[0].into_inner()], 1.0);
        assert_eq!(before.entries[&selectors[1].into_inner()], 0.0);
        assert_eq!(result.project_state(&before).unwrap(), after);
    }

    #[test]
    fn empty_unmatched_and_unsupported_requests_are_atomic() {
        let mut instance = combined_instance();
        let before = instance.clone();
        let before_bytes = instance.to_v2_bytes();

        let empty = instance
            .restore_lowered_capabilities_checked(&Capabilities::new())
            .unwrap();
        assert!(empty.restored_capabilities().is_empty());
        let state = v1::State::from_iter([(0, 1.0), (1, 2.0), (2, 0.0), (3, 1.0)]);
        assert_eq!(empty.project_state(&state).unwrap(), state);
        assert_eq!(instance, before);
        assert_eq!(instance.to_v2_bytes(), before_bytes);

        let error = instance
            .restore_lowered_capabilities_checked(&requested([AdditionalCapability::OneHot]))
            .unwrap_err();
        assert!(error.to_string().contains("does not yet support"));
        assert_eq!(instance, before);
        assert_eq!(instance.to_v2_bytes(), before_bytes);
    }

    #[test]
    fn manual_lifecycle_removal_is_not_an_inverse_lowering_candidate() {
        let mut instance = combined_instance();
        instance
            .relax_indicator_constraint(
                crate::IndicatorConstraintID::from(7),
                "manual lifecycle removal".to_string(),
                [],
            )
            .unwrap();
        let before = instance.clone();
        let result = instance
            .restore_lowered_capabilities_checked(&requested([AdditionalCapability::Indicator]))
            .unwrap();

        assert!(result.restored_capabilities().is_empty());
        assert_eq!(instance, before);
    }

    #[test]
    fn lowered_one_hot_is_not_silently_restored() {
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from((linear!(0) + linear!(1)).unwrap()))
            .decision_variables(btreemap! {
                VariableID::from(0) => DecisionVariable::binary(),
                VariableID::from(1) => DecisionVariable::binary(),
            })
            .constraints(BTreeMap::new())
            .one_hot_constraints(BTreeMap::from([(
                crate::OneHotConstraintID::from(4),
                OneHotConstraint::new(BTreeSet::from([VariableID::from(0), VariableID::from(1)]))
                    .unwrap(),
            )]))
            .build()
            .unwrap();
        instance.reduce_capabilities(&Capabilities::new()).unwrap();
        let before = instance.clone();

        let result = instance
            .restore_lowered_capabilities_checked(&requested([
                AdditionalCapability::Indicator,
                AdditionalCapability::Sos1,
            ]))
            .unwrap();

        assert!(result.restored_capabilities().is_empty());
        assert_eq!(instance, before);
        assert!(instance.one_hot_constraints().is_empty());
        assert_eq!(instance.removed_one_hot_constraints().len(), 1);
    }

    #[test]
    fn unrecorded_row_with_sos1_provenance_is_a_hard_batch_error() {
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from((linear!(0) + linear!(1)).unwrap()))
            .decision_variables(btreemap! {
                VariableID::from(0) => DecisionVariable::binary(),
                VariableID::from(1) => DecisionVariable::binary(),
            })
            .constraints(BTreeMap::new())
            .sos1_constraints(BTreeMap::from([(
                crate::Sos1ConstraintID::from(5),
                Sos1Constraint::new(BTreeSet::from([VariableID::from(0), VariableID::from(1)]))
                    .unwrap(),
            )]))
            .build()
            .unwrap();
        instance.reduce_capabilities(&Capabilities::new()).unwrap();
        let extra_id = instance
            .add_constraint(
                Constraint::less_than_or_equal_to_zero(Function::zero()),
                crate::ConstraintContext {
                    label: Default::default(),
                    provenance: vec![crate::Provenance::Sos1Constraint(
                        crate::Sos1ConstraintID::from(5),
                    )],
                },
            )
            .unwrap();
        let before = instance.clone();
        let before_bytes = instance.to_v2_bytes();

        let error = instance
            .restore_lowered_capabilities_checked(&requested([AdditionalCapability::Sos1]))
            .unwrap_err();

        assert!(error.to_string().contains(&format!("{extra_id:?}")));
        assert!(error.to_string().contains("not recorded"));
        assert_eq!(instance, before);
        assert_eq!(instance.to_v2_bytes(), before_bytes);
    }
}
