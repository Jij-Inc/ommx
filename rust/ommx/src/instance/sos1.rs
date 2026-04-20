use super::Instance;
use crate::{
    coeff,
    constraint::{ConstraintID, Provenance, RemovedReason},
    linear,
    sos1_constraint::Sos1ConstraintID,
    Bound, Coefficient, Constraint, Function, Kind, Linear, LinearMonomial, VariableID,
};
use anyhow::{bail, Context, Result};
use num::Zero;
use std::collections::BTreeMap;

/// Plan for each SOS1 variable: reuse it as its own indicator, or allocate a fresh one.
#[derive(Debug)]
enum IndicatorPlan {
    /// Variable is binary with bound `[0, 1]` — reuse it as its own indicator.
    Reuse,
    /// Variable requires a fresh binary indicator and Big-M constraints using these bounds.
    Fresh { bound: Bound },
}

impl Instance {
    #[cfg_attr(doc, katexit::katexit)]
    /// Convert a SOS1 constraint to regular constraints using the Big-M method.
    ///
    /// A SOS1 constraint over $\{x_1, \ldots, x_n\}$ with each $x_i \in [l_i, u_i]$ asserts
    /// that at most one $x_i$ is non-zero. This method encodes it with binary indicator
    /// variables $y_i$ and the constraints
    ///
    /// $$
    /// x_i \leq u_i y_i, \quad l_i y_i \leq x_i, \quad \sum_i y_i \leq 1.
    /// $$
    ///
    /// Per SOS1 variable $x_i$:
    /// - If $x_i$ is binary with bound $[0, 1]$, $y_i \coloneqq x_i$ is reused (no new
    ///   variable, no Big-M pair — only the cardinality sum references it).
    /// - Otherwise, a fresh binary $y_i$ is introduced with the upper and lower Big-M
    ///   constraints. Trivial bounds $u_i = 0$ (upper) or $l_i = 0$ (lower) are skipped.
    ///
    /// Errors if any $x_i$ has a non-binary bound that is not finite, or whose domain
    /// excludes $0$ (so that $y_i = 0 \Rightarrow x_i = 0$ would be infeasible).
    /// All validation happens before any mutation, so a failed call leaves the instance
    /// unchanged.
    ///
    /// The original SOS1 constraint is moved to [`Instance::removed_sos1_constraints`]
    /// with `reason = "ommx.Instance.convert_sos1_to_constraints"` and a
    /// `constraint_ids` parameter listing the new regular constraint IDs
    /// (comma-separated in insertion order).
    ///
    /// Returns the [`ConstraintID`]s of the newly created regular constraints in
    /// insertion order: Big-M upper/lower pairs per non-binary variable (sorted by
    /// variable ID), followed by the single cardinality sum constraint.
    pub fn convert_sos1_to_constraints(
        &mut self,
        id: Sos1ConstraintID,
    ) -> Result<Vec<ConstraintID>> {
        let sos1 = self
            .sos1_constraint_collection
            .active()
            .get(&id)
            .with_context(|| format!("SOS1 constraint with ID {id:?} not found"))?
            .clone();

        // Phase 1: plan and validate without mutation.
        let mut plans: Vec<(VariableID, IndicatorPlan)> = Vec::with_capacity(sos1.variables.len());
        for &var_id in &sos1.variables {
            let dv = self.decision_variables.get(&var_id).with_context(|| {
                format!(
                    "Decision variable {var_id:?} referenced by SOS1 constraint {id:?} not found"
                )
            })?;
            let bound = dv.bound();
            if dv.kind() == Kind::Binary && bound == Bound::of_binary() {
                plans.push((var_id, IndicatorPlan::Reuse));
                continue;
            }
            if !bound.is_finite() {
                bail!(
                    "Cannot convert SOS1 constraint {id:?} with Big-M: variable {var_id:?} has non-finite bound {bound:?}"
                );
            }
            if bound.lower() > 0.0 || bound.upper() < 0.0 {
                bail!(
                    "Cannot convert SOS1 constraint {id:?} with Big-M: variable {var_id:?} bound {bound:?} excludes 0"
                );
            }
            plans.push((var_id, IndicatorPlan::Fresh { bound }));
        }

        // Phase 2: mutate.
        //
        // Allocate fresh binary indicators first.
        let mut indicators: BTreeMap<VariableID, VariableID> = BTreeMap::new();
        for (x_id, plan) in &plans {
            let y_id = match plan {
                IndicatorPlan::Reuse => *x_id,
                IndicatorPlan::Fresh { .. } => {
                    let y = self.new_binary();
                    let y_id = y.id();
                    y.metadata.name = Some("ommx.sos1_indicator".to_string());
                    y.metadata.subscripts = vec![id.into_inner() as i64, x_id.into_inner() as i64];
                    y_id
                }
            };
            indicators.insert(*x_id, y_id);
        }

        // Big-M pair per Fresh variable (plans iterated in SOS1-variable-ID order
        // because `sos1.variables` is a `BTreeSet`).
        let mut new_constraint_ids: Vec<ConstraintID> = Vec::new();
        for (x_id, plan) in &plans {
            let IndicatorPlan::Fresh { bound } = plan else {
                continue;
            };
            let y_id = indicators[x_id];

            // Upper Big-M: x_i - u_i y_i <= 0. Skip when u_i == 0 (trivial with l_i <= 0).
            if bound.upper() > 0.0 {
                let neg_u = Coefficient::try_from(-bound.upper())
                    .context("Upper Big-M coefficient must be finite and non-zero")?;
                let f = Linear::zero()
                    + linear!(x_id.into_inner())
                    + Linear::single_term(LinearMonomial::Variable(y_id), neg_u);
                let new_id = self.insert_sos1_generated_constraint(
                    id,
                    Constraint::less_than_or_equal_to_zero(Function::from(f)),
                );
                new_constraint_ids.push(new_id);
            }

            // Lower Big-M: l_i y_i - x_i <= 0. Skip when l_i == 0 (trivial with u_i >= 0).
            if bound.lower() < 0.0 {
                let l = Coefficient::try_from(bound.lower())
                    .context("Lower Big-M coefficient must be finite and non-zero")?;
                let f = Linear::single_term(LinearMonomial::Variable(y_id), l)
                    + Linear::single_term(LinearMonomial::Variable(*x_id), coeff!(-1.0));
                let new_id = self.insert_sos1_generated_constraint(
                    id,
                    Constraint::less_than_or_equal_to_zero(Function::from(f)),
                );
                new_constraint_ids.push(new_id);
            }
        }

        // Cardinality sum: sum_i y_i - 1 <= 0.
        let sum = indicators
            .values()
            .fold(Linear::zero(), |acc, v| acc + linear!(v.into_inner()));
        let cardinality = Function::from(sum + Linear::from(coeff!(-1.0)));
        let new_id = self.insert_sos1_generated_constraint(
            id,
            Constraint::less_than_or_equal_to_zero(cardinality),
        );
        new_constraint_ids.push(new_id);

        // Move SOS1 to removed with a listing of the new constraint IDs.
        let mut parameters = fnv::FnvHashMap::default();
        let constraint_ids_str = new_constraint_ids
            .iter()
            .map(|id| id.into_inner().to_string())
            .collect::<Vec<_>>()
            .join(",");
        parameters.insert("constraint_ids".to_string(), constraint_ids_str);
        self.sos1_constraint_collection.relax(
            id,
            RemovedReason {
                reason: "ommx.Instance.convert_sos1_to_constraints".to_string(),
                parameters,
            },
        )?;

        Ok(new_constraint_ids)
    }

    /// Convert every active SOS1 constraint to regular constraints using Big-M.
    ///
    /// See [`Self::convert_sos1_to_constraints`] for the conversion rule. Returns a
    /// map from each original [`Sos1ConstraintID`] to the IDs of the regular
    /// constraints it produced.
    pub fn convert_all_sos1_to_constraints(
        &mut self,
    ) -> Result<BTreeMap<Sos1ConstraintID, Vec<ConstraintID>>> {
        let ids: Vec<_> = self
            .sos1_constraint_collection
            .active()
            .keys()
            .copied()
            .collect();
        let mut result = BTreeMap::new();
        for id in ids {
            let new_ids = self.convert_sos1_to_constraints(id)?;
            result.insert(id, new_ids);
        }
        Ok(result)
    }

    fn insert_sos1_generated_constraint(
        &mut self,
        sos1_id: Sos1ConstraintID,
        mut constraint: Constraint,
    ) -> ConstraintID {
        let new_id = self.constraint_collection.unused_id();
        constraint
            .metadata
            .provenance
            .push(Provenance::Sos1Constraint(sos1_id));
        self.constraint_collection
            .active_mut()
            .insert(new_id, constraint);
        new_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        constraint::Equality, sos1_constraint::Sos1Constraint, ATol, DecisionVariable, Sense,
    };
    use ::approx::assert_abs_diff_eq;
    use maplit::btreemap;
    use std::collections::{BTreeMap, BTreeSet};

    /// Build an instance with binary x0, x1 and a SOS1 over {x0, x1}.
    fn binary_sos1_instance() -> Instance {
        let decision_variables = btreemap! {
            VariableID::from(0) => DecisionVariable::binary(VariableID::from(0)),
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
        };
        let vars: BTreeSet<_> = [0u64, 1].into_iter().map(VariableID::from).collect();
        let sos1 = Sos1Constraint::new(vars);

        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(0) + linear!(1)))
            .decision_variables(decision_variables)
            .constraints(BTreeMap::new())
            .sos1_constraints(BTreeMap::from([(Sos1ConstraintID::from(5), sos1)]))
            .build()
            .unwrap()
    }

    /// Build an instance with a single integer x0 in [-2, 3] and a SOS1 over just {x0}.
    fn integer_sos1_instance(lower: f64, upper: f64) -> Instance {
        let dv = DecisionVariable::new(
            VariableID::from(0),
            Kind::Integer,
            Bound::new(lower, upper).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();
        let vars: BTreeSet<_> = [VariableID::from(0)].into_iter().collect();
        let sos1 = Sos1Constraint::new(vars);
        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(0)))
            .decision_variables(btreemap! { VariableID::from(0) => dv })
            .constraints(BTreeMap::new())
            .sos1_constraints(BTreeMap::from([(Sos1ConstraintID::from(9), sos1)]))
            .build()
            .unwrap()
    }

    #[test]
    fn binary_sos1_reuses_variables_and_emits_only_cardinality() {
        // All-binary SOS1 reduces to `sum(x_i) - 1 <= 0` with no new variables and no Big-M pair.
        let mut instance = binary_sos1_instance();
        let before_var_count = instance.decision_variables.len();

        let new_ids = instance
            .convert_sos1_to_constraints(Sos1ConstraintID::from(5))
            .unwrap();

        assert_eq!(new_ids.len(), 1, "binary SOS1 should emit only cardinality");
        assert_eq!(
            instance.decision_variables.len(),
            before_var_count,
            "no new indicators for binary reuse"
        );

        let cardinality = instance.constraints().get(&new_ids[0]).unwrap();
        assert_eq!(cardinality.equality, Equality::LessThanOrEqualToZero);
        let expected = Function::from(linear!(0) + linear!(1) + Linear::from(coeff!(-1.0)));
        assert_abs_diff_eq!(cardinality.function(), &expected);
        assert_eq!(
            cardinality.metadata.provenance,
            vec![Provenance::Sos1Constraint(Sos1ConstraintID::from(5))]
        );

        // Original SOS1 is recorded as removed with the new constraint ID.
        assert!(instance.sos1_constraints().is_empty());
        let (_, reason) = instance
            .removed_sos1_constraints()
            .get(&Sos1ConstraintID::from(5))
            .expect("SOS1 should be retained as removed");
        assert_eq!(reason.reason, "ommx.Instance.convert_sos1_to_constraints");
        assert_eq!(
            reason.parameters.get("constraint_ids").map(String::as_str),
            Some(new_ids[0].into_inner().to_string().as_str())
        );
    }

    #[test]
    fn integer_sos1_generates_bigm_pair_and_cardinality() {
        // x0 in [-2, 3]: both upper (u=3) and lower (l=-2) Big-M emitted.
        // Expected order: upper, lower, cardinality.
        let mut instance = integer_sos1_instance(-2.0, 3.0);
        let new_ids = instance
            .convert_sos1_to_constraints(Sos1ConstraintID::from(9))
            .unwrap();
        assert_eq!(new_ids.len(), 3);

        // A fresh binary indicator was added. Its ID is the next available after x0.
        let y_id = VariableID::from(1);
        let y = instance
            .decision_variables
            .get(&y_id)
            .expect("fresh indicator should exist");
        assert_eq!(y.kind(), Kind::Binary);
        assert_eq!(y.metadata.name.as_deref(), Some("ommx.sos1_indicator"));

        // Upper Big-M: x0 - 3 y == x0 + (-3) y <= 0
        let upper = instance.constraints().get(&new_ids[0]).unwrap();
        let expected_upper = Function::from(
            Linear::zero()
                + linear!(0)
                + Linear::single_term(LinearMonomial::Variable(y_id), coeff!(-3.0)),
        );
        assert_abs_diff_eq!(upper.function(), &expected_upper);

        // Lower Big-M: -2 y - x0 <= 0
        let lower = instance.constraints().get(&new_ids[1]).unwrap();
        let expected_lower = Function::from(
            Linear::single_term(LinearMonomial::Variable(y_id), coeff!(-2.0))
                + Linear::single_term(LinearMonomial::Variable(VariableID::from(0)), coeff!(-1.0)),
        );
        assert_abs_diff_eq!(lower.function(), &expected_lower);

        // Cardinality: y - 1 <= 0
        let card = instance.constraints().get(&new_ids[2]).unwrap();
        let expected_card = Function::from(
            Linear::zero() + linear!(y_id.into_inner()) + Linear::from(coeff!(-1.0)),
        );
        assert_abs_diff_eq!(card.function(), &expected_card);

        // `constraint_ids` parameter on removed reason lists all three in insertion order.
        let (_, reason) = instance
            .removed_sos1_constraints()
            .get(&Sos1ConstraintID::from(9))
            .unwrap();
        let expected_ids = new_ids
            .iter()
            .map(|id| id.into_inner().to_string())
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(
            reason.parameters.get("constraint_ids").map(String::as_str),
            Some(expected_ids.as_str())
        );
    }

    #[test]
    fn trivial_bigm_sides_are_skipped() {
        // x0 in [0, 3]: lower l=0 skips the lower Big-M constraint; only upper + cardinality.
        let mut instance = integer_sos1_instance(0.0, 3.0);
        let new_ids = instance
            .convert_sos1_to_constraints(Sos1ConstraintID::from(9))
            .unwrap();
        assert_eq!(
            new_ids.len(),
            2,
            "l=0 should skip lower Big-M and emit only upper + cardinality"
        );
    }

    #[test]
    fn infinite_bound_is_rejected_without_mutation() {
        // Continuous x0 with default (infinite) bound cannot be Big-M converted.
        let dv = DecisionVariable::continuous(VariableID::from(0));
        let vars: BTreeSet<_> = [VariableID::from(0)].into_iter().collect();
        let sos1 = Sos1Constraint::new(vars);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(0)))
            .decision_variables(btreemap! { VariableID::from(0) => dv })
            .constraints(BTreeMap::new())
            .sos1_constraints(BTreeMap::from([(Sos1ConstraintID::from(9), sos1)]))
            .build()
            .unwrap();
        let before_vars = instance.decision_variables.clone();
        let before_constraints = instance.constraints().clone();

        let err = instance
            .convert_sos1_to_constraints(Sos1ConstraintID::from(9))
            .unwrap_err();
        assert!(err.to_string().contains("non-finite"));

        // State unchanged: no new variables or constraints were allocated on the failure path.
        assert_eq!(instance.decision_variables, before_vars);
        assert_eq!(instance.constraints(), &before_constraints);
        assert!(!instance.sos1_constraints().is_empty());
    }

    #[test]
    fn bound_excluding_zero_is_rejected() {
        // x0 in [1, 3]: y=0 -> x0=0 is infeasible; bail.
        let mut instance = integer_sos1_instance(1.0, 3.0);
        let err = instance
            .convert_sos1_to_constraints(Sos1ConstraintID::from(9))
            .unwrap_err();
        assert!(err.to_string().contains("excludes 0"));
    }

    #[test]
    fn missing_id_errors_without_mutating_state() {
        let mut instance = binary_sos1_instance();
        let before_sos1 = instance.sos1_constraints().clone();
        let before_constraints = instance.constraints().clone();

        let err = instance
            .convert_sos1_to_constraints(Sos1ConstraintID::from(999))
            .unwrap_err();
        assert!(err.to_string().contains("999"));

        assert_eq!(instance.sos1_constraints(), &before_sos1);
        assert_eq!(instance.constraints(), &before_constraints);
    }

    #[test]
    fn bulk_conversion_returns_per_sos1_ids() {
        // Two disjoint SOS1 constraints, both binary — bulk call produces one entry each.
        let decision_variables = btreemap! {
            VariableID::from(0) => DecisionVariable::binary(VariableID::from(0)),
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
            VariableID::from(3) => DecisionVariable::binary(VariableID::from(3)),
        };
        let a = Sos1Constraint::new(
            [VariableID::from(0), VariableID::from(1)]
                .into_iter()
                .collect(),
        );
        let b = Sos1Constraint::new(
            [VariableID::from(2), VariableID::from(3)]
                .into_iter()
                .collect(),
        );
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(0) + linear!(2)))
            .decision_variables(decision_variables)
            .constraints(BTreeMap::new())
            .sos1_constraints(BTreeMap::from([
                (Sos1ConstraintID::from(1), a),
                (Sos1ConstraintID::from(2), b),
            ]))
            .build()
            .unwrap();

        let result = instance.convert_all_sos1_to_constraints().unwrap();
        assert_eq!(result.len(), 2);
        for (_, new_ids) in &result {
            assert_eq!(new_ids.len(), 1); // all-binary: only cardinality
            assert!(instance.constraints().contains_key(&new_ids[0]));
        }
        assert!(instance.sos1_constraints().is_empty());
        assert_eq!(instance.removed_sos1_constraints().len(), 2);
    }
}
