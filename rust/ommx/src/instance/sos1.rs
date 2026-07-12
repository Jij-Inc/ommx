use super::{
    reduction::{AssignmentMap, SelectorRole},
    AdditionalCapability, Capabilities, Instance, GENERATED_CONSTRAINT_IDS_PARAMETER,
    SOS1_LOWERING_REASON,
};
use crate::{
    coeff,
    constraint::{ConstraintContext, ConstraintID, Provenance, RemovedReason},
    linear,
    sos1_constraint::Sos1ConstraintID,
    Bound, Coefficient, Constraint, Function, Kind, Linear, LinearMonomial, VariableID,
};
use anyhow::{bail, Context, Result};
use std::collections::{BTreeMap, BTreeSet};

/// Plan for each SOS1 variable: reuse it as its own indicator, or allocate a fresh one.
#[derive(Debug)]
enum IndicatorPlan {
    /// Variable is binary with bound `[0, 1]` — reuse it as its own indicator.
    Reuse,
    /// Variable requires a fresh binary indicator and Big-M constraints using these bounds.
    Fresh { bound: Bound },
}

/// Private one-shot plan for an exact checked inverse lowering.
///
/// All fallible proof, isolation, assignment-map, and storage work is applied
/// to `staged`. Committing the plan is one infallible replacement of the root
/// Instance.
#[allow(dead_code)] // Used by the stacked public inverse-lowering facade.
struct Sos1InversePlan {
    staged: Instance,
    assignment_map: AssignmentMap,
}

#[allow(dead_code)] // Used by the stacked public inverse-lowering facade.
impl Sos1InversePlan {
    fn commit(self, target: &mut Instance) -> AssignmentMap {
        *target = self.staged;
        self.assignment_map
    }
}

impl Instance {
    /// Restore one SOS1 that this Instance previously lowered, after exact V1
    /// content verification and complete isolation of fresh selectors.
    ///
    /// Private while the common reduction result and public capability request
    /// API are still being exercised by the first family consumers.
    /// `permitted_additions` is an explicit permission for the semantic
    /// capability introduced by this operation; it is not a declaration of
    /// every capability already present in the Instance.
    #[allow(dead_code)] // Used by the stacked public inverse-lowering facade.
    pub(super) fn restore_sos1_from_lowering_checked(
        &mut self,
        id: Sos1ConstraintID,
        permitted_additions: &Capabilities,
    ) -> Result<AssignmentMap> {
        let plan = self.plan_sos1_inverse(id, permitted_additions)?;
        Ok(plan.commit(self))
    }

    #[allow(dead_code)] // Used by the checked inverse entry point above.
    fn plan_sos1_inverse(
        &self,
        id: Sos1ConstraintID,
        permitted_additions: &Capabilities,
    ) -> Result<Sos1InversePlan> {
        if !permitted_additions.contains(&AdditionalCapability::Sos1) {
            bail!("Restoring SOS1 constraint {id:?} requires explicit SOS1 capability permission");
        }

        let verified = crate::proof::verify_sos1_big_m_v1(self, id)?;
        debug_assert_eq!(verified.source_id(), id);
        for &member in &verified.source().variables {
            if !self.decision_variables.contains_key(&member) {
                bail!("Cannot restore SOS1 constraint {id:?}: member {member:?} is not registered");
            }
            if self.decision_variable_dependency.get(&member).is_some() {
                bail!(
                    "Cannot restore SOS1 constraint {id:?}: member {member:?} is a dependency target"
                );
            }
            if self.fixed_decision_variable_values().contains_key(&member) {
                bail!("Cannot restore SOS1 constraint {id:?}: member {member:?} is fixed");
            }
        }

        let selector_roles = verified
            .selectors()
            .iter()
            .map(|(&member, &selector)| {
                let role = if member == selector {
                    SelectorRole::Reused
                } else {
                    SelectorRole::Fresh(selector)
                };
                (member, role)
            })
            .collect::<BTreeMap<_, _>>();
        let fresh_selectors = selector_roles
            .values()
            .filter_map(|role| match role {
                SelectorRole::Reused => None,
                SelectorRole::Fresh(selector) => Some(*selector),
            })
            .collect::<BTreeSet<_>>();
        let generated_rows = verified
            .generated_rows()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();

        self.ensure_variables_isolated_for_removal(&fresh_selectors, &generated_rows)?;
        let assignment_map = AssignmentMap::sos1_selectors(
            self.decision_variables.keys().copied().collect(),
            id,
            selector_roles,
        )?;

        let mut staged = self.clone();
        staged
            .constraint_collection
            .consume_active_rows(&generated_rows)?;
        staged
            .decision_variables
            .remove_unfixed_rows(&fresh_selectors)?;
        staged
            .sos1_constraint_collection
            .restore_removed_row(id, verified.source().clone())?;
        let staged_variable_ids = staged
            .decision_variables
            .keys()
            .copied()
            .collect::<crate::VariableIDSet>();
        debug_assert_eq!(&staged_variable_ids, assignment_map.target_ids());
        debug_assert!(staged.constraint_collection.validate_context_ids().is_ok());
        debug_assert!(staged
            .sos1_constraint_collection
            .validate_context_ids()
            .is_ok());
        debug_assert!(staged
            .required_capabilities()
            .contains(&AdditionalCapability::Sos1));

        Ok(Sos1InversePlan {
            staged,
            assignment_map,
        })
    }

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
    /// Errors if any $x_i$ has a non-binary bound that is not finite, whose domain
    /// excludes $0$ (so that $y_i = 0 \Rightarrow x_i = 0$ would be infeasible), or whose
    /// kind is [`Kind::SemiInteger`] or [`Kind::SemiContinuous`] (the split domain
    /// $\{0\} \cup [l, u]$ is not uniformly implemented across the codebase, so Big-M
    /// conversion of these kinds is not supported yet).
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
        let plans = self.plan_sos1_conversion(id)?;
        self.apply_sos1_conversion(id, plans)
    }

    /// Convert every active SOS1 constraint to regular constraints using Big-M.
    ///
    /// See [`Self::convert_sos1_to_constraints`] for the conversion rule.
    ///
    /// This is atomic: every active SOS1 is validated up front, and only once all
    /// validations succeed are the conversions applied. If any SOS1 fails
    /// validation (unsupported kind, non-finite bound, domain excludes 0, etc.),
    /// no mutation happens and the instance is left untouched.
    ///
    /// Returns a map from each original [`Sos1ConstraintID`] to the IDs of the
    /// regular constraints it produced.
    pub fn convert_all_sos1_to_constraints(
        &mut self,
    ) -> Result<BTreeMap<Sos1ConstraintID, Vec<ConstraintID>>> {
        let ids: Vec<_> = self
            .sos1_constraint_collection
            .active()
            .keys()
            .copied()
            .collect();
        // Phase 1: plan every SOS1 up front. Bail on the first validation failure
        // before any mutation has happened.
        let mut all_plans: Vec<(Sos1ConstraintID, Vec<(VariableID, IndicatorPlan)>)> =
            Vec::with_capacity(ids.len());
        for id in ids {
            let plans = self.plan_sos1_conversion(id)?;
            all_plans.push((id, plans));
        }
        // Phase 2: apply. Planned state only references variables that existed at
        // plan time; `apply_sos1_conversion` only adds fresh variables / constraints
        // and relaxes its own SOS1, so earlier applications cannot invalidate later
        // plans.
        let mut result = BTreeMap::new();
        for (id, plans) in all_plans {
            result.insert(id, self.apply_sos1_conversion(id, plans)?);
        }
        Ok(result)
    }

    /// Validate a single SOS1 and build its per-variable conversion plan.
    ///
    /// Read-only: never mutates `self`. Errors before producing any plan if the
    /// SOS1 references an unknown variable, or if a variable has an unsupported
    /// kind (semi-*), a non-finite bound, or a bound that excludes 0.
    fn plan_sos1_conversion(
        &self,
        id: Sos1ConstraintID,
    ) -> Result<Vec<(VariableID, IndicatorPlan)>> {
        let sos1 = self
            .sos1_constraint_collection
            .active()
            .get(&id)
            .with_context(|| format!("SOS1 constraint with ID {id:?} not found"))?;

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
            // Semi-continuous / semi-integer variables carry a split domain `{0} ∪ [l, u]`
            // that the rest of the codebase does not yet treat uniformly (the bound field
            // does not always contain 0, while `check_value_consistency` and many
            // transformations assume it does). Converting them via Big-M would silently
            // paper over that inconsistency, so reject them until the semi semantics are
            // resolved project-wide.
            if matches!(dv.kind(), Kind::SemiInteger | Kind::SemiContinuous) {
                bail!(
                    "Cannot convert SOS1 constraint {id:?} with Big-M: variable {var_id:?} has kind {:?}; semi-continuous / semi-integer variables are not supported",
                    dv.kind()
                );
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
        Ok(plans)
    }

    fn apply_sos1_conversion(
        &mut self,
        id: Sos1ConstraintID,
        plans: Vec<(VariableID, IndicatorPlan)>,
    ) -> Result<Vec<ConstraintID>> {
        // Allocate fresh binary indicators first.
        let mut indicators: BTreeMap<VariableID, VariableID> = BTreeMap::new();
        for (x_id, plan) in &plans {
            let y_id = match plan {
                IndicatorPlan::Reuse => *x_id,
                IndicatorPlan::Fresh { .. } => self.new_decision_variable_with_label(
                    Kind::Binary,
                    Bound::of_binary(),
                    crate::ModelingLabel {
                        name: Some("ommx.sos1_indicator".to_string()),
                        subscripts: vec![id.into_inner() as i64, x_id.into_inner() as i64],
                        ..Default::default()
                    },
                    None,
                    crate::ATol::default(),
                )?,
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
                    .expect("planner guaranteed finite non-zero upper bound");
                let f = (Linear::zero() + linear!(x_id.into_inner()))?;
                let f = (f + Linear::single_term(LinearMonomial::Variable(y_id), neg_u))?;
                let new_id = self.insert_sos1_generated_constraint(
                    id,
                    Constraint::less_than_or_equal_to_zero(Function::from(f)),
                );
                new_constraint_ids.push(new_id);
            }

            // Lower Big-M: l_i y_i - x_i <= 0. Skip when l_i == 0 (trivial with u_i >= 0).
            if bound.lower() < 0.0 {
                let l = Coefficient::try_from(bound.lower())
                    .expect("planner guaranteed finite non-zero lower bound");
                let f = (Linear::single_term(LinearMonomial::Variable(y_id), l)
                    + Linear::single_term(LinearMonomial::Variable(*x_id), coeff!(-1.0)))?;
                let new_id = self.insert_sos1_generated_constraint(
                    id,
                    Constraint::less_than_or_equal_to_zero(Function::from(f)),
                );
                new_constraint_ids.push(new_id);
            }
        }

        // Cardinality sum: sum_i y_i - 1 <= 0.
        //
        // Skip emitting it for an empty SOS1, since `0 + (-1) <= 0` is the trivially
        // satisfied tautology `-1 <= 0` and would only inflate the constraint count.
        // Empty SOS1 constraints are rejected at `Instance::build` time, so this
        // branch is defensive: it covers callers that bypass the builder (e.g.
        // future preprocessing that shrinks a SOS1 to empty).
        if !indicators.is_empty() {
            let sum = indicators
                .values()
                .try_fold(Linear::zero(), |acc, v| acc + linear!(v.into_inner()))?;
            let cardinality = Function::from((sum + Linear::from(coeff!(-1.0)))?);
            let new_id = self.insert_sos1_generated_constraint(
                id,
                Constraint::less_than_or_equal_to_zero(cardinality),
            );
            new_constraint_ids.push(new_id);
        }

        // Move SOS1 to removed with a listing of the new constraint IDs.
        let mut parameters = fnv::FnvHashMap::default();
        let constraint_ids_str = new_constraint_ids
            .iter()
            .map(|id| id.into_inner().to_string())
            .collect::<Vec<_>>()
            .join(",");
        parameters.insert(
            GENERATED_CONSTRAINT_IDS_PARAMETER.to_string(),
            constraint_ids_str,
        );
        self.sos1_constraint_collection
            .relax(
                id,
                RemovedReason {
                    reason: SOS1_LOWERING_REASON.to_string(),
                    parameters,
                },
            )
            .expect("SOS1 id was present when the plan was built and hasn't been touched since");

        Ok(new_constraint_ids)
    }

    fn insert_sos1_generated_constraint(
        &mut self,
        sos1_id: Sos1ConstraintID,
        constraint: Constraint,
    ) -> ConstraintID {
        let new_id = self.constraint_collection.unused_id();
        let context = ConstraintContext {
            provenance: vec![Provenance::Sos1Constraint(sos1_id)],
            ..Default::default()
        };
        self.constraint_collection
            .insert_active_with_context(new_id, constraint, context)
            .expect("new_id was allocated from this collection");
        new_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        constraint::Equality, sos1_constraint::Sos1Constraint, DecisionVariable, Evaluate,
        IndicatorConstraint, ModelingLabel, Sense, Substitute,
    };
    use ::approx::assert_abs_diff_eq;
    use maplit::btreemap;
    use std::collections::{BTreeMap, BTreeSet};

    /// Build an instance with binary x0, x1 and a SOS1 over {x0, x1}.
    fn binary_sos1_instance() -> Instance {
        let decision_variables = btreemap! {
            VariableID::from(0) => DecisionVariable::binary(),
            VariableID::from(1) => DecisionVariable::binary(),
        };
        let vars: BTreeSet<_> = [0u64, 1].into_iter().map(VariableID::from).collect();
        let sos1 = Sos1Constraint::new(vars).unwrap();

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
            Kind::Integer,
            Bound::new(lower, upper).unwrap(),
            crate::ATol::default(),
        )
        .unwrap();
        let vars: BTreeSet<_> = [VariableID::from(0)].into_iter().collect();
        let sos1 = Sos1Constraint::new(vars).unwrap();
        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(0)))
            .decision_variables(btreemap! { VariableID::from(0) => dv })
            .constraints(BTreeMap::new())
            .sos1_constraints(BTreeMap::from([(Sos1ConstraintID::from(9), sos1)]))
            .build()
            .unwrap()
    }

    fn sos1_capability() -> Capabilities {
        [AdditionalCapability::Sos1].into_iter().collect()
    }

    fn mixed_sos1_instance() -> Instance {
        let integer = DecisionVariable::new(
            Kind::Integer,
            Bound::new(-2.0, 3.0).unwrap(),
            crate::ATol::default(),
        )
        .unwrap();
        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from((linear!(0) + linear!(1)).unwrap()))
            .decision_variables(btreemap! {
                VariableID::from(0) => DecisionVariable::binary(),
                VariableID::from(1) => integer,
            })
            .constraints(BTreeMap::new())
            .sos1_constraints(BTreeMap::from([(
                Sos1ConstraintID::from(9),
                Sos1Constraint::new(BTreeSet::from([VariableID::from(0), VariableID::from(1)]))
                    .unwrap(),
            )]))
            .build()
            .unwrap()
    }

    fn generated_selector(
        instance: &Instance,
        sos1_id: Sos1ConstraintID,
        member: VariableID,
    ) -> VariableID {
        instance
            .decision_variables()
            .keys()
            .copied()
            .find(|id| {
                instance.variable_labels().collect_for(*id)
                    == ModelingLabel {
                        name: Some("ommx.sos1_indicator".to_string()),
                        subscripts: vec![sos1_id.into_inner() as i64, member.into_inner() as i64],
                        ..Default::default()
                    }
            })
            .expect("current SOS1 lowerer generated the selector")
    }

    fn assert_checked_inverse_failure_unchanged(
        instance: &mut Instance,
        id: Sos1ConstraintID,
        expected_message: &str,
    ) {
        let before = instance.clone();
        let before_bytes = instance.to_v2_bytes();
        let error = instance
            .restore_sos1_from_lowering_checked(id, &sos1_capability())
            .unwrap_err();
        assert!(
            error.to_string().contains(expected_message),
            "expected {expected_message:?}, got {error:#}"
        );
        assert_eq!(instance, &before);
        assert_eq!(instance.to_v2_bytes(), before_bytes);
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
        let expected = Function::from(
            ((linear!(0) + linear!(1)).unwrap() + Linear::from(coeff!(-1.0))).unwrap(),
        );
        assert_abs_diff_eq!(cardinality.function(), &expected);
        assert_eq!(
            instance
                .constraint_collection()
                .context()
                .provenance(new_ids[0]),
            &[Provenance::Sos1Constraint(Sos1ConstraintID::from(5))]
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
        assert_eq!(
            instance.variable_labels().name(y_id),
            Some("ommx.sos1_indicator")
        );

        // Upper Big-M: x0 - 3 y == x0 + (-3) y <= 0
        let upper = instance.constraints().get(&new_ids[0]).unwrap();
        let expected_upper = Function::from(
            ((Linear::zero() + linear!(0)).unwrap()
                + Linear::single_term(LinearMonomial::Variable(y_id), coeff!(-3.0)))
            .unwrap(),
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
            ((Linear::zero() + linear!(y_id.into_inner())).unwrap() + Linear::from(coeff!(-1.0)))
                .unwrap(),
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
    fn checked_inverse_restores_mixed_sos1_and_removes_only_fresh_selectors() {
        let mut instance = mixed_sos1_instance();
        instance
            .set_sos1_constraint_context(
                Sos1ConstraintID::from(9),
                ConstraintContext {
                    label: ModelingLabel {
                        name: Some("source-sos1".to_string()),
                        ..Default::default()
                    },
                    provenance: Vec::new(),
                },
            )
            .unwrap();
        let original = instance.clone();
        let original_bytes = instance.to_v2_bytes();
        let generated = instance
            .convert_sos1_to_constraints(Sos1ConstraintID::from(9))
            .unwrap();
        let selector =
            generated_selector(&instance, Sos1ConstraintID::from(9), VariableID::from(1));
        let lowered = instance.clone();

        let map = instance
            .restore_sos1_from_lowering_checked(Sos1ConstraintID::from(9), &sos1_capability())
            .unwrap();

        assert_eq!(instance, original);
        assert_eq!(instance.to_v2_bytes(), original_bytes);
        assert!(!instance.decision_variables().contains_key(&selector));
        assert_eq!(
            instance.variable_labels().collect_for(selector),
            Default::default()
        );
        for id in generated {
            assert!(!instance.constraints().contains_key(&id));
            assert!(!instance.removed_constraints().contains_key(&id));
            assert!(!instance.constraint_context().contains(id));
        }
        assert_eq!(
            instance
                .sos1_constraint_context()
                .name(Sos1ConstraintID::from(9)),
            Some("source-sos1")
        );
        assert_eq!(
            instance.required_capabilities(),
            [AdditionalCapability::Sos1].into_iter().collect()
        );

        let full = crate::v1::State::from_iter([(0, 0.0), (1, 2.0), (selector.into_inner(), 1.0)]);
        let projected = map.project_state(&full).unwrap();
        assert_eq!(projected, crate::v1::State::from_iter([(0, 0.0), (1, 2.0)]));
        assert_eq!(map.lift_state(&projected).unwrap(), full);
        assert!(lowered
            .evaluate(&full, crate::ATol::default())
            .unwrap()
            .feasible());
        assert!(instance
            .evaluate(&projected, crate::ATol::default())
            .unwrap()
            .feasible());

        // On values separated from every tolerance boundary, the checked
        // exact correspondence also agrees with the runtime classifier and
        // preserves the objective for both feasible and infeasible states.
        for binary in [0.0, 1.0] {
            for integer in -2..=3 {
                let target = crate::v1::State::from_iter([(0, binary), (1, integer as f64)]);
                let source = map.lift_state(&target).unwrap();
                assert_eq!(map.project_state(&source).unwrap(), target);
                let restored_solution = instance.evaluate(&target, crate::ATol::default()).unwrap();
                let lowered_solution = lowered.evaluate(&source, crate::ATol::default()).unwrap();
                assert_eq!(lowered_solution.feasible(), restored_solution.feasible());
                assert_eq!(lowered_solution.objective(), restored_solution.objective());
            }
        }

        // Lift uses mathematical exact zero, independently of evaluation
        // tolerance: both signed zeros map to selector 0 and any finite
        // nonzero value, including a subnormal, maps to 1.
        for (member, expected_selector) in [
            (0.0, 0.0),
            (-0.0, 0.0),
            (f64::from_bits(1), 1.0),
            (-f64::from_bits(1), 1.0),
        ] {
            let target = crate::v1::State::from_iter([(0, 0.0), (1, member)]);
            let lifted = map.lift_state(&target).unwrap();
            assert_eq!(lifted.entries[&selector.into_inner()], expected_selector);
            assert_eq!(map.project_state(&lifted).unwrap(), target);
        }
    }

    #[test]
    fn checked_inverse_all_reused_sos1_has_identity_map() {
        let mut instance = binary_sos1_instance();
        let original = instance.clone();
        instance
            .convert_sos1_to_constraints(Sos1ConstraintID::from(5))
            .unwrap();

        let map = instance
            .restore_sos1_from_lowering_checked(Sos1ConstraintID::from(5), &sos1_capability())
            .unwrap();

        assert_eq!(instance, original);
        let state = crate::v1::State::from_iter([(0, 1.0), (1, 0.0)]);
        assert_eq!(map.project_state(&state).unwrap(), state);
        assert_eq!(map.lift_state(&state).unwrap(), state);
    }

    #[test]
    fn checked_inverse_requires_explicit_sos1_permission_only() {
        let mut denied = mixed_sos1_instance();
        denied
            .convert_sos1_to_constraints(Sos1ConstraintID::from(9))
            .unwrap();
        let before = denied.clone();
        let before_bytes = denied.to_v2_bytes();
        let error = denied
            .restore_sos1_from_lowering_checked(Sos1ConstraintID::from(9), &Capabilities::new())
            .unwrap_err();
        assert!(error.to_string().contains("explicit SOS1 capability"));
        assert_eq!(denied, before);
        assert_eq!(denied.to_v2_bytes(), before_bytes);

        let mut allowed = before;
        allowed
            .add_indicator_constraint(
                IndicatorConstraint::new(
                    VariableID::from(0),
                    Equality::LessThanOrEqualToZero,
                    Function::zero(),
                ),
                Default::default(),
            )
            .unwrap();
        allowed
            .restore_sos1_from_lowering_checked(Sos1ConstraintID::from(9), &sos1_capability())
            .unwrap();
        assert_eq!(
            allowed.required_capabilities(),
            [AdditionalCapability::Indicator, AdditionalCapability::Sos1]
                .into_iter()
                .collect()
        );
    }

    #[test]
    fn checked_inverse_rejects_partial_evaluated_and_substituted_histories() {
        let mut partial = mixed_sos1_instance();
        partial
            .convert_sos1_to_constraints(Sos1ConstraintID::from(9))
            .unwrap();
        partial
            .partial_evaluate(
                &crate::v1::State::from_iter([(1, 1.0)]),
                crate::ATol::default(),
            )
            .unwrap();
        assert_checked_inverse_failure_unchanged(
            &mut partial,
            Sos1ConstraintID::from(9),
            "canonical V1 content exactly",
        );

        let mut substituted = mixed_sos1_instance();
        substituted
            .convert_sos1_to_constraints(Sos1ConstraintID::from(9))
            .unwrap();
        substituted = substituted
            .substitute_one(VariableID::from(1), &Function::zero())
            .unwrap();
        assert_checked_inverse_failure_unchanged(
            &mut substituted,
            Sos1ConstraintID::from(9),
            "canonical V1 content exactly",
        );
    }

    #[test]
    fn checked_inverse_rejects_ulp_modified_link_row_without_mutation() {
        let mut instance = mixed_sos1_instance();
        let generated = instance
            .convert_sos1_to_constraints(Sos1ConstraintID::from(9))
            .unwrap();
        let selector =
            generated_selector(&instance, Sos1ConstraintID::from(9), VariableID::from(1));
        let altered_m = f64::from_bits((-3.0f64).to_bits() + 1);
        let altered = Constraint::less_than_or_equal_to_zero(Function::from(
            (Linear::single_term(LinearMonomial::Variable(VariableID::from(1)), coeff!(1.0))
                + Linear::single_term(LinearMonomial::Variable(selector), coeff!(altered_m)))
            .unwrap(),
        ));
        instance
            .constraint_collection
            .replace_active_rows(BTreeMap::from([(generated[0], altered)]))
            .unwrap();

        assert_checked_inverse_failure_unchanged(
            &mut instance,
            Sos1ConstraintID::from(9),
            "canonical V1 content exactly",
        );
    }

    #[test]
    fn checked_inverse_rejects_modified_selector_domain_without_mutation() {
        let mut instance = mixed_sos1_instance();
        instance
            .convert_sos1_to_constraints(Sos1ConstraintID::from(9))
            .unwrap();
        let selector =
            generated_selector(&instance, Sos1ConstraintID::from(9), VariableID::from(1));
        instance
            .clip_bounds(
                &crate::Bounds::from_iter([(selector, Bound::new(0.0, 0.0).unwrap())]),
                crate::ATol::default(),
            )
            .unwrap();

        assert_checked_inverse_failure_unchanged(
            &mut instance,
            Sos1ConstraintID::from(9),
            "canonical binary domain",
        );
    }

    #[test]
    fn checked_inverse_rejects_fixed_or_dependent_members_without_mutation() {
        let zero_bound_lowering = || {
            let mut instance = integer_sos1_instance(0.0, 0.0);
            assert_eq!(
                instance
                    .convert_sos1_to_constraints(Sos1ConstraintID::from(9))
                    .unwrap()
                    .len(),
                1
            );
            instance
        };

        let mut fixed = zero_bound_lowering();
        fixed
            .partial_evaluate(
                &crate::v1::State::from_iter([(0, 0.0)]),
                crate::ATol::default(),
            )
            .unwrap();
        assert_checked_inverse_failure_unchanged(&mut fixed, Sos1ConstraintID::from(9), "is fixed");

        let mut dependent = zero_bound_lowering()
            .substitute_one(VariableID::from(0), &Function::zero())
            .unwrap();
        assert_checked_inverse_failure_unchanged(
            &mut dependent,
            Sos1ConstraintID::from(9),
            "dependency target",
        );
    }

    #[test]
    fn checked_inverse_rejects_nonisolated_or_relabelled_selector_without_mutation() {
        let mut used = mixed_sos1_instance();
        used.convert_sos1_to_constraints(Sos1ConstraintID::from(9))
            .unwrap();
        let selector = generated_selector(&used, Sos1ConstraintID::from(9), VariableID::from(1));
        used.add_constraint(
            Constraint::less_than_or_equal_to_zero(Function::from(linear!(selector.into_inner()))),
            Default::default(),
        )
        .unwrap();
        assert_checked_inverse_failure_unchanged(
            &mut used,
            Sos1ConstraintID::from(9),
            "active regular constraint",
        );

        let mut relabelled = mixed_sos1_instance();
        relabelled
            .convert_sos1_to_constraints(Sos1ConstraintID::from(9))
            .unwrap();
        let selector =
            generated_selector(&relabelled, Sos1ConstraintID::from(9), VariableID::from(1));
        relabelled
            .set_variable_label(selector, ModelingLabel::default())
            .unwrap();
        assert_checked_inverse_failure_unchanged(
            &mut relabelled,
            Sos1ConstraintID::from(9),
            "exactly one canonical V1 selector",
        );
    }

    #[test]
    fn checked_inverse_rejects_removed_generated_row_without_mutation() {
        let mut instance = mixed_sos1_instance();
        let generated = instance
            .convert_sos1_to_constraints(Sos1ConstraintID::from(9))
            .unwrap();
        instance
            .relax_constraint(generated[0], "test preprocessing".to_string(), [])
            .unwrap();
        assert_checked_inverse_failure_unchanged(
            &mut instance,
            Sos1ConstraintID::from(9),
            "is not active",
        );
    }

    #[test]
    fn infinite_bound_is_rejected_without_mutation() {
        // Continuous x0 with default (infinite) bound cannot be Big-M converted.
        let dv = DecisionVariable::continuous();
        let vars: BTreeSet<_> = [VariableID::from(0)].into_iter().collect();
        let sos1 = Sos1Constraint::new(vars).unwrap();
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
    fn semi_variables_are_rejected() {
        // Kind::SemiInteger / SemiContinuous carry a split {0} ∪ [l, u] domain that
        // isn't uniformly implemented across the codebase; Big-M conversion is
        // explicitly not supported for them and must error before mutation.
        for dv in [
            DecisionVariable::semi_integer(),
            DecisionVariable::semi_continuous(),
        ] {
            let kind = dv.kind();
            let sos1 =
                Sos1Constraint::new([VariableID::from(0)].into_iter().collect::<BTreeSet<_>>())
                    .unwrap();
            let mut instance = Instance::builder()
                .sense(Sense::Minimize)
                .objective(Function::from(linear!(0)))
                .decision_variables(btreemap! { VariableID::from(0) => dv })
                .constraints(BTreeMap::new())
                .sos1_constraints(BTreeMap::from([(Sos1ConstraintID::from(9), sos1)]))
                .build()
                .unwrap();
            let before_constraints = instance.constraints().clone();

            let err = instance
                .convert_sos1_to_constraints(Sos1ConstraintID::from(9))
                .unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains("semi-continuous") && msg.contains("not supported"),
                "expected not-supported error for {kind:?}, got: {msg}"
            );
            // Error is raised before any mutation: active SOS1 still present, no new constraints.
            assert!(instance
                .sos1_constraints()
                .contains_key(&Sos1ConstraintID::from(9)));
            assert_eq!(instance.constraints(), &before_constraints);
        }
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
            VariableID::from(0) => DecisionVariable::binary(),
            VariableID::from(1) => DecisionVariable::binary(),
            VariableID::from(2) => DecisionVariable::binary(),
            VariableID::from(3) => DecisionVariable::binary(),
        };
        let a = Sos1Constraint::new(
            [VariableID::from(0), VariableID::from(1)]
                .into_iter()
                .collect(),
        )
        .unwrap();
        let b = Sos1Constraint::new(
            [VariableID::from(2), VariableID::from(3)]
                .into_iter()
                .collect(),
        )
        .unwrap();
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
        for new_ids in result.values() {
            assert_eq!(new_ids.len(), 1); // all-binary: only cardinality
            assert!(instance.constraints().contains_key(&new_ids[0]));
        }
        assert!(instance.sos1_constraints().is_empty());
        assert_eq!(instance.removed_sos1_constraints().len(), 2);
    }

    #[test]
    fn empty_sos1_constraint_is_rejected_at_build() {
        // An empty Sos1Constraint carries no variables to constrain, so the
        // Big-M cardinality constraint would degenerate to the tautology `-1 <= 0`.
        // The builder should reject empty SOS1 instead of letting it through.
        let dv = DecisionVariable::binary();
        let empty_sos1 = Sos1Constraint {
            variables: BTreeSet::new(),
            stage: crate::Sos1CreatedData,
        };
        let err = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(0)))
            .decision_variables(btreemap! { VariableID::from(0) => dv })
            .constraints(BTreeMap::new())
            .sos1_constraints(BTreeMap::from([(Sos1ConstraintID::from(42), empty_sos1)]))
            .build()
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("no variables") && msg.contains("42"),
            "expected EmptySos1Constraint error mentioning the id, got: {msg}"
        );
    }

    #[test]
    fn bulk_conversion_is_atomic_on_error() {
        // Two SOS1 constraints: the first valid (binary reuse), the second invalid
        // (continuous with default, infinite bound). The bulk call must fail
        // without applying the valid one either.
        let decision_variables = btreemap! {
            VariableID::from(0) => DecisionVariable::binary(),
            VariableID::from(1) => DecisionVariable::binary(),
            VariableID::from(2) => DecisionVariable::continuous(),
        };
        let valid = Sos1Constraint::new(
            [VariableID::from(0), VariableID::from(1)]
                .into_iter()
                .collect(),
        )
        .unwrap();
        let invalid =
            Sos1Constraint::new([VariableID::from(2)].into_iter().collect::<BTreeSet<_>>())
                .unwrap();
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(0) + linear!(2)))
            .decision_variables(decision_variables)
            .constraints(BTreeMap::new())
            .sos1_constraints(BTreeMap::from([
                (Sos1ConstraintID::from(1), valid),
                (Sos1ConstraintID::from(2), invalid),
            ]))
            .build()
            .unwrap();
        let before_sos1 = instance.sos1_constraints().clone();
        let before_vars = instance.decision_variables.clone();
        let before_constraints = instance.constraints().clone();

        let err = instance.convert_all_sos1_to_constraints().unwrap_err();
        assert!(err.to_string().contains("non-finite"));

        // Atomicity: the earlier valid SOS1 must not have been converted either.
        assert_eq!(instance.sos1_constraints(), &before_sos1);
        assert_eq!(instance.decision_variables, before_vars);
        assert_eq!(instance.constraints(), &before_constraints);
        assert!(instance.removed_sos1_constraints().is_empty());
    }
}
