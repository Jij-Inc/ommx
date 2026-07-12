use super::{
    reduction::AssignmentMap, AdditionalCapability, Capabilities, Instance,
    ONE_HOT_GENERATED_CONSTRAINT_ID_PARAMETER, ONE_HOT_LOWERING_REASON,
};
use crate::{
    coeff,
    constraint::{ConstraintID, Provenance, RemovedReason},
    linear,
    one_hot_constraint::OneHotConstraintID,
    Constraint, Function, Linear, VariableIDSet,
};
use anyhow::{bail, Context, Result};
use std::collections::BTreeSet;

/// Private one-shot plan for an exact checked inverse lowering.
///
/// OneHot lowering does not change the decision-variable space, so its
/// assignment map is always the identity. All fallible proof and storage work
/// is nevertheless applied to `staged`, matching the other family inverses.
struct OneHotInversePlan {
    staged: Instance,
    assignment_map: AssignmentMap,
}

impl OneHotInversePlan {
    fn commit(self, target: &mut Instance) -> AssignmentMap {
        *target = self.staged;
        self.assignment_map
    }
}

impl Instance {
    /// Restore one OneHot constraint that this Instance previously lowered,
    /// after exact V1 content and context verification.
    pub(super) fn restore_one_hot_from_lowering_checked(
        &mut self,
        id: OneHotConstraintID,
        permitted_additions: &Capabilities,
    ) -> Result<AssignmentMap> {
        let plan = self.plan_one_hot_inverse(id, permitted_additions)?;
        Ok(plan.commit(self))
    }

    fn plan_one_hot_inverse(
        &self,
        id: OneHotConstraintID,
        permitted_additions: &Capabilities,
    ) -> Result<OneHotInversePlan> {
        if !permitted_additions.contains(&AdditionalCapability::OneHot) {
            bail!(
                "Restoring OneHot constraint {id:?} requires explicit OneHot capability permission"
            );
        }

        let verified = crate::proof::verify_one_hot_v1(self, id)?;
        debug_assert_eq!(verified.source_id(), id);
        for &member in &verified.source().variables {
            if !self.decision_variables.contains_key(&member) {
                bail!(
                    "Cannot restore OneHot constraint {id:?}: member {member:?} is not registered"
                );
            }
            if self.decision_variable_dependency.get(&member).is_some() {
                bail!(
                    "Cannot restore OneHot constraint {id:?}: member {member:?} is a dependency target"
                );
            }
            if self.fixed_decision_variable_values().contains_key(&member) {
                bail!("Cannot restore OneHot constraint {id:?}: member {member:?} is fixed");
            }
        }

        let generated_rows = BTreeSet::from([verified.generated_row()]);
        let assignment_map =
            AssignmentMap::identity(self.decision_variables.keys().copied().collect());
        let mut staged = self.clone();
        staged
            .constraint_collection
            .consume_active_rows(&generated_rows)?;
        staged
            .one_hot_constraint_collection
            .restore_removed_row(id, verified.source().clone())?;
        let staged_variable_ids = staged
            .decision_variables
            .keys()
            .copied()
            .collect::<VariableIDSet>();
        debug_assert_eq!(&staged_variable_ids, assignment_map.target_ids());
        debug_assert!(staged.constraint_collection.validate_context_ids().is_ok());
        debug_assert!(staged
            .one_hot_constraint_collection
            .validate_context_ids()
            .is_ok());
        debug_assert!(staged
            .required_capabilities()
            .contains(&AdditionalCapability::OneHot));

        Ok(OneHotInversePlan {
            staged,
            assignment_map,
        })
    }

    #[cfg_attr(doc, katexit::katexit)]
    /// Convert a one-hot constraint to a regular equality constraint.
    ///
    /// A one-hot constraint over variables $\{x_1, \ldots, x_n\}$ is mathematically
    /// equivalent to the linear equality
    ///
    /// $$
    /// \sum_{i=1}^{n} x_i - 1 = 0.
    /// $$
    ///
    /// This method inserts that equality as a new [`Constraint`] and moves the
    /// original one-hot constraint into `removed_one_hot_constraints` with
    /// `reason = "ommx.Instance.convert_one_hot_to_constraint"` and a
    /// `constraint_id` parameter pointing to the new regular constraint.
    ///
    /// Returns the [`ConstraintID`] of the newly created regular constraint.
    pub fn convert_one_hot_to_constraint(
        &mut self,
        id: OneHotConstraintID,
    ) -> Result<ConstraintID> {
        let one_hot = self
            .one_hot_constraint_collection
            .active()
            .get(&id)
            .with_context(|| format!("OneHot constraint with ID {id:?} not found"))?
            .clone();

        let new_id = self.constraint_collection.unused_id();

        let sum = one_hot
            .variables
            .iter()
            .try_fold(Linear::zero(), |acc, v| acc + linear!(v.into_inner()))?;
        let function = Function::from((sum + Linear::from(coeff!(-1.0)))?);

        let new_constraint = Constraint::equal_to_zero(function);
        // Carry over the one-hot's context into the new regular constraint,
        // appending the OneHot promotion to provenance.
        let mut new_context = self.one_hot_constraint_collection.context().collect_for(id);
        new_context
            .provenance
            .push(Provenance::OneHotConstraint(id));
        self.constraint_collection.insert_active_with_context(
            new_id,
            new_constraint,
            new_context,
        )?;

        let mut parameters = fnv::FnvHashMap::default();
        parameters.insert(
            ONE_HOT_GENERATED_CONSTRAINT_ID_PARAMETER.to_string(),
            new_id.into_inner().to_string(),
        );
        self.one_hot_constraint_collection.relax(
            id,
            RemovedReason {
                reason: ONE_HOT_LOWERING_REASON.to_string(),
                parameters,
            },
        )?;

        Ok(new_id)
    }

    /// Convert every active one-hot constraint to a regular equality constraint.
    ///
    /// See [`Self::convert_one_hot_to_constraint`] for the conversion rule.
    /// Returns the IDs of the newly created regular constraints in ascending
    /// order of the original one-hot constraint IDs.
    pub fn convert_all_one_hots_to_constraints(&mut self) -> Result<Vec<ConstraintID>> {
        let ids: Vec<_> = self
            .one_hot_constraint_collection
            .active()
            .keys()
            .copied()
            .collect();
        ids.into_iter()
            .map(|id| self.convert_one_hot_to_constraint(id))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        constraint::Equality, one_hot_constraint::OneHotConstraint, DecisionVariable,
        ModelingLabel, Sense, Sos1ConstraintID, VariableID,
    };
    use std::collections::{BTreeMap, BTreeSet};

    /// Build an instance with two binary variables and a single one-hot constraint on them.
    fn instance_with_one_one_hot() -> Instance {
        let mut decision_variables = BTreeMap::new();
        for id in [1u64, 2] {
            decision_variables.insert(VariableID::from(id), DecisionVariable::binary());
        }
        let vars: BTreeSet<_> = [1u64, 2].into_iter().map(VariableID::from).collect();
        let one_hot = OneHotConstraint::new(vars).unwrap();

        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(1) + linear!(2)))
            .decision_variables(decision_variables)
            .constraints(BTreeMap::new())
            .one_hot_constraints(BTreeMap::from([(OneHotConstraintID::from(7), one_hot)]))
            .build()
            .unwrap()
    }

    #[test]
    fn converts_single_one_hot_to_equality() {
        let mut instance = instance_with_one_one_hot();
        let new_id = instance
            .convert_one_hot_to_constraint(OneHotConstraintID::from(7))
            .unwrap();

        // Original one-hot moved to removed with the expected reason pointing at the new constraint.
        assert!(instance.one_hot_constraints().is_empty());
        let removed = instance
            .removed_one_hot_constraints()
            .get(&OneHotConstraintID::from(7))
            .expect("original one-hot should be retained as removed");
        assert_eq!(
            removed.1.reason,
            "ommx.Instance.convert_one_hot_to_constraint"
        );
        assert_eq!(
            removed
                .1
                .parameters
                .get("constraint_id")
                .map(String::as_str),
            Some(new_id.into_inner().to_string().as_str())
        );

        // Newly inserted regular constraint is an equality `x1 + x2 - 1 == 0`.
        let new_constraint = instance.constraints().get(&new_id).unwrap();
        assert_eq!(new_constraint.equality, Equality::EqualToZero);
        let expected = Function::from(
            ((linear!(1) + linear!(2)).unwrap() + Linear::from(coeff!(-1.0))).unwrap(),
        );
        use ::approx::assert_abs_diff_eq;
        assert_abs_diff_eq!(new_constraint.function(), &expected);

        // The conversion step is recorded in the new constraint's provenance.
        assert_eq!(
            instance
                .constraint_collection()
                .context()
                .provenance(new_id),
            &[Provenance::OneHotConstraint(OneHotConstraintID::from(7))],
        );
    }

    #[test]
    fn missing_id_errors_without_mutating_state() {
        // Unknown IDs return Err and leave both collections untouched.
        let mut instance = instance_with_one_one_hot();
        let before_one_hots = instance.one_hot_constraints().clone();
        let before_constraints = instance.constraints().clone();

        let err = instance
            .convert_one_hot_to_constraint(OneHotConstraintID::from(999))
            .unwrap_err();
        assert!(err.to_string().contains("999"));

        assert_eq!(instance.one_hot_constraints(), &before_one_hots);
        assert_eq!(instance.constraints(), &before_constraints);
    }

    #[test]
    fn bulk_conversion_drains_all_active_one_hots() {
        // Two disjoint one-hots → both converted, none left active.
        let mut decision_variables = BTreeMap::new();
        for id in [1u64, 2, 3, 4] {
            decision_variables.insert(VariableID::from(id), DecisionVariable::binary());
        }
        let a =
            OneHotConstraint::new([1u64, 2].into_iter().map(VariableID::from).collect()).unwrap();
        let b =
            OneHotConstraint::new([3u64, 4].into_iter().map(VariableID::from).collect()).unwrap();

        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(1) + linear!(3)))
            .decision_variables(decision_variables)
            .constraints(BTreeMap::new())
            .one_hot_constraints(BTreeMap::from([
                (OneHotConstraintID::from(1), a),
                (OneHotConstraintID::from(2), b),
            ]))
            .build()
            .unwrap();

        let new_ids = instance.convert_all_one_hots_to_constraints().unwrap();
        assert_eq!(new_ids.len(), 2);
        assert!(instance.one_hot_constraints().is_empty());
        assert_eq!(instance.removed_one_hot_constraints().len(), 2);
        for id in new_ids {
            assert!(instance.constraints().contains_key(&id));
        }
    }

    #[test]
    fn checked_inverse_restores_exact_one_hot_and_context() {
        let id = OneHotConstraintID::from(7);
        let mut instance = instance_with_one_one_hot();
        instance
            .set_one_hot_constraint_context(
                id,
                crate::ConstraintContext {
                    label: ModelingLabel {
                        name: Some("choose".to_string()),
                        subscripts: vec![4],
                        ..Default::default()
                    },
                    provenance: vec![Provenance::Sos1Constraint(Sos1ConstraintID::from(9))],
                },
            )
            .unwrap();
        let original = instance.clone();
        let original_bytes = instance.to_v2_bytes();
        instance.convert_one_hot_to_constraint(id).unwrap();
        let lowered_ids = instance
            .decision_variables()
            .keys()
            .copied()
            .collect::<crate::VariableIDSet>();

        let map = instance
            .restore_one_hot_from_lowering_checked(
                id,
                &Capabilities::from([AdditionalCapability::OneHot]),
            )
            .unwrap();

        assert_eq!(instance, original);
        assert_eq!(instance.to_v2_bytes(), original_bytes);
        assert_eq!(map.source_ids(), &lowered_ids);
        assert_eq!(map.target_ids(), &lowered_ids);
        let state = crate::v1::State::from_iter([(1, 1.0), (2, 0.0)]);
        assert_eq!(map.project_state(&state).unwrap(), state);
        assert_eq!(map.lift_state(&state).unwrap(), state);
    }

    #[test]
    fn checked_inverse_requires_permission_and_is_atomic_on_changed_row() {
        let id = OneHotConstraintID::from(7);
        let mut instance = instance_with_one_one_hot();
        let generated = instance.convert_one_hot_to_constraint(id).unwrap();
        let lowered = instance.clone();

        let error = instance
            .restore_one_hot_from_lowering_checked(id, &Capabilities::new())
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("explicit OneHot capability permission"));
        assert_eq!(instance, lowered);

        instance
            .insert_constraint(
                generated,
                Constraint::equal_to_zero(Function::from(
                    ((linear!(1) + linear!(2)).unwrap() + coeff!(-2.0)).unwrap(),
                )),
            )
            .unwrap();
        let before = instance.clone();
        let before_bytes = instance.to_v2_bytes();
        let error = instance
            .restore_one_hot_from_lowering_checked(
                id,
                &Capabilities::from([AdditionalCapability::OneHot]),
            )
            .unwrap_err();
        assert!(error.to_string().contains("canonical V1 equality exactly"));
        assert_eq!(instance, before);
        assert_eq!(instance.to_v2_bytes(), before_bytes);
    }
}
