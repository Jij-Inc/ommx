use super::Instance;
use crate::{
    coeff,
    constraint::{ConstraintID, Provenance, RemovedReason},
    linear,
    one_hot_constraint::OneHotConstraintID,
    Constraint, Function, Linear,
};
use anyhow::{Context, Result};
use num::Zero;

impl Instance {
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
            .fold(Linear::zero(), |acc, v| acc + linear!(v.into_inner()));
        let function = Function::from(sum + Linear::from(coeff!(-1.0)));

        let new_constraint = Constraint::equal_to_zero(function);
        // Carry over the one-hot's metadata into the new regular constraint,
        // appending the OneHot promotion to provenance.
        let mut new_metadata = self
            .one_hot_constraint_collection
            .metadata()
            .collect_for(id);
        new_metadata
            .provenance
            .push(Provenance::OneHotConstraint(id));
        self.constraint_collection
            .insert_with(new_id, new_constraint, new_metadata);

        let mut parameters = fnv::FnvHashMap::default();
        parameters.insert("constraint_id".to_string(), new_id.into_inner().to_string());
        self.one_hot_constraint_collection.relax(
            id,
            RemovedReason {
                reason: "ommx.Instance.convert_one_hot_to_constraint".to_string(),
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
        constraint::Equality, one_hot_constraint::OneHotConstraint, DecisionVariable, Sense,
        VariableID,
    };
    use std::collections::{BTreeMap, BTreeSet};

    /// Build an instance with two binary variables and a single one-hot constraint on them.
    fn instance_with_one_one_hot() -> Instance {
        let mut decision_variables = BTreeMap::new();
        for id in [1u64, 2] {
            decision_variables.insert(
                VariableID::from(id),
                DecisionVariable::binary(VariableID::from(id)),
            );
        }
        let vars: BTreeSet<_> = [1u64, 2].into_iter().map(VariableID::from).collect();
        let one_hot = OneHotConstraint::new(vars);

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
        let expected = Function::from(linear!(1) + linear!(2) + Linear::from(coeff!(-1.0)));
        use ::approx::assert_abs_diff_eq;
        assert_abs_diff_eq!(new_constraint.function(), &expected);

        // The conversion step is recorded in the new constraint's provenance.
        assert_eq!(
            instance
                .constraint_collection()
                .metadata()
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
            decision_variables.insert(
                VariableID::from(id),
                DecisionVariable::binary(VariableID::from(id)),
            );
        }
        let a = OneHotConstraint::new([1u64, 2].into_iter().map(VariableID::from).collect());
        let b = OneHotConstraint::new([3u64, 4].into_iter().map(VariableID::from).collect());

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
}
