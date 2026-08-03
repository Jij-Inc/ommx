//! Checked promotion of canonical SOS1 Big-M formulations.
//!
//! The witness accepted here is deliberately untrusted. It identifies stable
//! OMMX IDs and their claimed roles; the current [`Instance`] remains the sole
//! source of truth for domains and row contents. A successful promotion
//! compresses
//!
//! `Base(x) AND CanonicalSos1BigM(x, z)`
//!
//! into
//!
//! `Base(x) AND SOS1(x)`
//!
//! while retaining the complete formulation history in the same [`Instance`].
//! Verified Big-M rows move to the removed collection, and fresh selectors
//! become dependent variables reconstructed by a [`crate::DependentExpr`].
//!
//! The reusable proof plan remains private and is applied only to a staged
//! clone. This keeps witness rejection atomic and prevents a checked plan from
//! being reused after the instance changes.

use super::{
    sos1::{
        canonical_sos1_big_m_cardinality, canonical_sos1_big_m_lower_link,
        canonical_sos1_big_m_upper_link,
    },
    Instance,
};
use crate::{
    Bound, Constraint, ConstraintContext, ConstraintID, DependentExpr, Equality, Function, Kind,
    RemovedReason, Sos1Constraint, Sos1ConstraintID, VariableID, VariableIDSet,
};
use std::collections::{BTreeMap, BTreeSet};

/// Claimed selector role for one member of a canonical SOS1 Big-M formulation.
///
/// This is witness data, not a verified fact. [`Instance::promote_sos1_big_m`]
/// checks the role against the current member domain and exact regular-row
/// contents before changing the instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sos1BigMSelectorWitness {
    /// The member itself is claimed to be a full-domain binary selector.
    Reused,
    /// A separate private binary selector is claimed for the member.
    Fresh {
        /// Private binary selector associated with the member.
        selector: VariableID,
        /// Canonical row `member - upper * selector <= 0`, when required.
        upper_link: Option<ConstraintID>,
        /// Canonical row `lower * selector - member <= 0`, when required.
        lower_link: Option<ConstraintID>,
    },
}

/// Untrusted stable-ID witness for one canonical SOS1 Big-M promotion.
///
/// Map keys are the intended SOS1 members. Each value claims whether the
/// member is reused as its selector or linked to a fresh selector. The final
/// regular row is identified separately because it must be the exact
/// cardinality constraint over all claimed selectors.
///
/// Bounds and coefficients are intentionally absent: validation always reads
/// them from the current [`Instance`]. The witness is runtime-only and has no
/// serialization contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sos1BigMPromotionWitness {
    selector_roles: BTreeMap<VariableID, Sos1BigMSelectorWitness>,
    cardinality_constraint: ConstraintID,
}

impl Sos1BigMPromotionWitness {
    /// Create untrusted witness data. No instance-dependent validation occurs
    /// until [`Instance::promote_sos1_big_m`] is called.
    pub fn new(
        selector_roles: BTreeMap<VariableID, Sos1BigMSelectorWitness>,
        cardinality_constraint: ConstraintID,
    ) -> Self {
        Self {
            selector_roles,
            cardinality_constraint,
        }
    }

    /// Claimed member-to-selector roles, keyed by intended SOS1 member ID.
    pub fn selector_roles(&self) -> &BTreeMap<VariableID, Sos1BigMSelectorWitness> {
        &self.selector_roles
    }

    /// Claimed canonical selector-cardinality row.
    pub fn cardinality_constraint(&self) -> ConstraintID {
        self.cardinality_constraint
    }
}

/// Result of one checked SOS1 Big-M promotion.
///
/// State reconstruction is owned by the mutated [`Instance`]: each fresh
/// selector remains registered and is assigned a [`DependentExpr`] evaluated
/// by [`Instance::populate_state`]. This result is therefore informational and
/// does not represent an external project/lift boundary.
#[must_use = "the result identifies the promoted constraint and retained history"]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sos1BigMPromotion {
    sos1_constraint_id: Sos1ConstraintID,
    members: VariableIDSet,
    fresh_selectors: BTreeMap<VariableID, VariableID>,
    relaxed_constraint_ids: BTreeSet<ConstraintID>,
}

impl Sos1BigMPromotion {
    /// ID allocated to the promoted SOS1 constraint.
    pub fn sos1_constraint_id(&self) -> Sos1ConstraintID {
        self.sos1_constraint_id
    }

    /// Members of the promoted SOS1 constraint.
    pub fn members(&self) -> &VariableIDSet {
        &self.members
    }

    /// Verified fresh selectors, keyed by their associated SOS1 member.
    pub fn fresh_selectors(&self) -> &BTreeMap<VariableID, VariableID> {
        &self.fresh_selectors
    }

    /// Canonical formulation rows moved from active to removed.
    pub fn relaxed_constraint_ids(&self) -> &BTreeSet<ConstraintID> {
        &self.relaxed_constraint_ids
    }
}

#[derive(Debug)]
struct Sos1BigMPromotionPlan {
    result: Sos1BigMPromotion,
}

impl Instance {
    /// Promote one exact canonical Big-M selector formulation to SOS1.
    ///
    /// The witness is untrusted. This method validates all of the following
    /// against the current instance before mutation:
    ///
    /// - a non-empty member set with finite supported domains;
    /// - exact agreement between full binary members and reused-selector roles;
    /// - distinct full binary fresh selectors outside the member set;
    /// - exact canonical upper/lower links and cardinality row, without a
    ///   tolerance or scalar-multiple relaxation;
    /// - absence of every fresh selector from current active solver input,
    ///   except for the claimed formulation rows.
    ///
    /// Rust-side concepts not modeled by the initial Lean semantics are
    /// handled conservatively. Selected semi variables, fixed or dependent
    /// members, fixed binary member bounds, and already fixed/dependent fresh
    /// selectors are rejected. Removed constraints, named functions,
    /// dependency RHS expressions, row context, and selector labels remain
    /// valid history and are preserved unchanged. Unrelated nonlinear
    /// expressions and unrelated special constraints are preserved unchanged.
    /// Calling this family-specific method is the explicit request to add the
    /// SOS1 capability to the instance; witness rejection means only that the
    /// claimed formulation is outside this conservative checker.
    ///
    /// The correctness contract uses exact algebraic feasibility. It does not
    /// claim equality of [`crate::Evaluate`] results for a positive [`crate::ATol`],
    /// because regular rows and SOS1 constraints apply different tolerance
    /// classifiers near zero.
    ///
    /// On success the verified formulation rows are relaxed, fresh selectors
    /// remain registered as dependent variables, and a new active SOS1
    /// constraint is inserted. The operation is atomic: every change is
    /// applied to a staged clone before replacing `self`.
    pub fn promote_sos1_big_m(
        &mut self,
        witness: &Sos1BigMPromotionWitness,
    ) -> crate::Result<Sos1BigMPromotion> {
        let plan = self.plan_sos1_big_m_promotion(witness)?;
        let mut staged = self.clone();
        for &id in &plan.result.relaxed_constraint_ids {
            staged.constraint_collection.relax(
                id,
                RemovedReason {
                    reason: "promoted canonical SOS1 Big-M formulation".to_string(),
                    parameters: [(
                        "sos1_constraint_id".to_string(),
                        plan.result.sos1_constraint_id.to_string(),
                    )]
                    .into_iter()
                    .collect(),
                },
            )?;
        }

        let mut dependencies = staged
            .decision_variable_dependency
            .iter()
            .map(|(&id, expr)| (id, expr.clone()))
            .collect::<Vec<_>>();
        dependencies.extend(
            plan.result
                .fresh_selectors
                .iter()
                .map(|(&member, &selector)| {
                    (
                        selector,
                        DependentExpr::nonzero_indicator(Function::from(crate::linear!(
                            member.into_inner()
                        ))),
                    )
                }),
        );
        staged.decision_variable_dependency =
            crate::DecisionVariableDependencies::new(dependencies)?;
        staged
            .sos1_constraint_collection
            .insert_active_with_context(
                plan.result.sos1_constraint_id,
                Sos1Constraint::new(plan.result.members.clone())?,
                ConstraintContext::default(),
            )?;

        *self = staged;
        Ok(plan.result)
    }

    fn plan_sos1_big_m_promotion(
        &self,
        witness: &Sos1BigMPromotionWitness,
    ) -> crate::Result<Sos1BigMPromotionPlan> {
        if witness.selector_roles.is_empty() {
            crate::bail!("SOS1 Big-M promotion witness must contain at least one member");
        }

        let members = witness
            .selector_roles
            .keys()
            .copied()
            .collect::<VariableIDSet>();
        let mut fresh_selectors = BTreeMap::new();
        let mut fresh_selector_ids = VariableIDSet::new();
        let mut relaxed_constraint_ids = BTreeSet::new();

        for (&member, &role) in &witness.selector_roles {
            let variable = self.decision_variables().get(&member).ok_or_else(|| {
                crate::error!(
                    { ?member },
                    "SOS1 Big-M promotion member {member:?} is not registered"
                )
            })?;
            let bound = variable.bound();
            if !bound.is_finite() {
                crate::bail!(
                    { ?member, ?bound },
                    "SOS1 Big-M promotion member {member:?} does not have finite bounds"
                );
            }
            if matches!(variable.kind(), Kind::SemiContinuous | Kind::SemiInteger) {
                crate::bail!(
                    { ?member, kind = ?variable.kind() },
                    "SOS1 Big-M promotion member {member:?} has an unsupported semi-variable kind"
                );
            }
            if self.fixed_decision_variable_values().contains_key(&member) {
                crate::bail!(
                    { ?member },
                    "SOS1 Big-M promotion member {member:?} is fixed"
                );
            }
            if self.decision_variable_dependency.get(&member).is_some() {
                crate::bail!(
                    { ?member },
                    "SOS1 Big-M promotion member {member:?} is a dependency target"
                );
            }

            let is_full_binary = variable.kind() == Kind::Binary && bound == Bound::of_binary();
            match role {
                Sos1BigMSelectorWitness::Reused => {
                    if !is_full_binary {
                        crate::bail!(
                            { ?member, kind = ?variable.kind(), ?bound },
                            "Reused SOS1 selector {member:?} does not have the full binary domain [0, 1]"
                        );
                    }
                }
                Sos1BigMSelectorWitness::Fresh {
                    selector,
                    upper_link,
                    lower_link,
                } => {
                    if variable.kind() == Kind::Binary {
                        crate::bail!(
                            { ?member, ?bound },
                            "Binary SOS1 member {member:?} must have the full [0, 1] domain and be reused"
                        );
                    }
                    if members.contains(&selector) {
                        crate::bail!(
                            { ?member, ?selector },
                            "Fresh SOS1 selector {selector:?} collides with a promoted member"
                        );
                    }
                    if !fresh_selector_ids.insert(selector) {
                        crate::bail!(
                            { ?member, ?selector },
                            "Fresh SOS1 selector {selector:?} is assigned to more than one member"
                        );
                    }
                    let selector_variable =
                        self.decision_variables().get(&selector).ok_or_else(|| {
                            crate::error!(
                                { ?member, ?selector },
                                "Fresh SOS1 selector {selector:?} is not registered"
                            )
                        })?;
                    if selector_variable.kind() != Kind::Binary
                        || selector_variable.bound() != Bound::of_binary()
                    {
                        crate::bail!(
                            {
                                ?member,
                                ?selector,
                                kind = ?selector_variable.kind(),
                                bound = ?selector_variable.bound()
                            },
                            "Fresh SOS1 selector {selector:?} does not have the full binary domain [0, 1]"
                        );
                    }
                    self.validate_optional_sos1_link(
                        member,
                        selector,
                        upper_link,
                        bound.upper() > 0.0,
                        "upper",
                        || canonical_sos1_big_m_upper_link(member, selector, bound.upper()),
                        &mut relaxed_constraint_ids,
                    )?;
                    self.validate_optional_sos1_link(
                        member,
                        selector,
                        lower_link,
                        bound.lower() < 0.0,
                        "lower",
                        || canonical_sos1_big_m_lower_link(member, selector, bound.lower()),
                        &mut relaxed_constraint_ids,
                    )?;
                    fresh_selectors.insert(member, selector);
                }
            }
        }

        if !relaxed_constraint_ids.insert(witness.cardinality_constraint) {
            crate::bail!(
                { cardinality = ?witness.cardinality_constraint },
                "SOS1 cardinality constraint is also claimed as a member link"
            );
        }
        let selector_ids = witness
            .selector_roles
            .iter()
            .map(|(&member, role)| match role {
                Sos1BigMSelectorWitness::Reused => member,
                Sos1BigMSelectorWitness::Fresh { selector, .. } => *selector,
            });
        let expected_cardinality = canonical_sos1_big_m_cardinality(selector_ids)?;
        self.ensure_exact_sos1_formulation_row(
            witness.cardinality_constraint,
            &expected_cardinality,
            "cardinality",
        )?;

        self.ensure_variables_isolated_for_sos1_promotion(
            &fresh_selector_ids,
            &relaxed_constraint_ids,
        )?;

        let sos1_constraint_id = self.next_sos1_constraint_id()?;

        Ok(Sos1BigMPromotionPlan {
            result: Sos1BigMPromotion {
                sos1_constraint_id,
                members,
                fresh_selectors,
                relaxed_constraint_ids,
            },
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn validate_optional_sos1_link(
        &self,
        member: VariableID,
        selector: VariableID,
        actual_id: Option<ConstraintID>,
        required: bool,
        side: &'static str,
        expected: impl FnOnce() -> crate::Result<Constraint>,
        relaxed: &mut BTreeSet<ConstraintID>,
    ) -> crate::Result<()> {
        match (required, actual_id) {
            (false, None) => Ok(()),
            (false, Some(id)) => crate::bail!(
                { ?member, ?selector, ?id, side },
                "SOS1 member {member:?} has a {side} link even though its bound makes that canonical link unnecessary"
            ),
            (true, None) => crate::bail!(
                { ?member, ?selector, side },
                "SOS1 member {member:?} is missing its canonical {side} link"
            ),
            (true, Some(id)) => {
                if !relaxed.insert(id) {
                    crate::bail!(
                        { ?member, ?selector, ?id, side },
                        "Regular constraint {id:?} is claimed for more than one SOS1 formulation role"
                    );
                }
                self.ensure_exact_sos1_formulation_row(id, &expected()?, side)
            }
        }
    }

    fn ensure_exact_sos1_formulation_row(
        &self,
        id: ConstraintID,
        expected: &Constraint,
        role: &'static str,
    ) -> crate::Result<()> {
        let actual = self.constraints().get(&id).ok_or_else(|| {
            crate::error!(
                { ?id, role },
                "Claimed SOS1 {role} constraint {id:?} is not active"
            )
        })?;
        if !same_linear_constraint(actual, expected) {
            crate::bail!(
                { ?id, role },
                "Claimed SOS1 {role} constraint {id:?} does not match the canonical row exactly"
            );
        }
        Ok(())
    }

    /// Prove that fresh selectors occur in current active solver input only in
    /// the formulation rows claimed by the witness.
    ///
    /// Removed rows, named functions, and dependency RHS expressions are not
    /// solver input. They are intentionally allowed to retain references and
    /// observe the canonical dependent-selector value during evaluation.
    fn ensure_variables_isolated_for_sos1_promotion(
        &self,
        private_ids: &VariableIDSet,
        relaxed_regular_rows: &BTreeSet<ConstraintID>,
    ) -> crate::Result<()> {
        let usage = self.decision_variable_usage();
        for &id in private_ids {
            if let Some(entry) = usage.get(id) {
                if entry.used_in_objective() {
                    crate::bail!({ ?id }, "Fresh SOS1 selector {id:?} is used by the objective");
                }
                if let Some(row) = entry
                    .used_in_regular_constraints()
                    .difference(relaxed_regular_rows)
                    .next()
                {
                    crate::bail!(
                        { ?id, ?row },
                        "Fresh SOS1 selector {id:?} is used by retained active regular constraint {row:?}"
                    );
                }
                if let Some(row) = entry.used_in_indicator_constraints().iter().next() {
                    crate::bail!(
                        { ?id, ?row },
                        "Fresh SOS1 selector {id:?} is used by active Indicator constraint {row:?}"
                    );
                }
                if let Some(row) = entry.used_in_one_hot_constraints().iter().next() {
                    crate::bail!(
                        { ?id, ?row },
                        "Fresh SOS1 selector {id:?} is used by active OneHot constraint {row:?}"
                    );
                }
                if let Some(row) = entry.used_in_sos1_constraints().iter().next() {
                    crate::bail!(
                        { ?id, ?row },
                        "Fresh SOS1 selector {id:?} is used by active SOS1 constraint {row:?}"
                    );
                }
            }

            if self.decision_variable_dependency.get(&id).is_some() {
                crate::bail!(
                    { ?id },
                    "Fresh SOS1 selector {id:?} is a dependency target"
                );
            }
            if self.fixed_decision_variable_values().contains_key(&id) {
                crate::bail!({ ?id }, "Fresh SOS1 selector {id:?} is fixed");
            }
        }
        Ok(())
    }

    fn next_sos1_constraint_id(&self) -> crate::Result<Sos1ConstraintID> {
        let max_id = self
            .sos1_constraints()
            .keys()
            .chain(self.removed_sos1_constraints().keys())
            .map(|id| id.into_inner())
            .max();
        match max_id {
            None => Ok(Sos1ConstraintID::from(0)),
            Some(max_id) => max_id
                .checked_add(1)
                .map(Sos1ConstraintID::from)
                .ok_or_else(|| crate::error!("SOS1 constraint ID space is exhausted")),
        }
    }
}

fn same_linear_constraint(actual: &Constraint, expected: &Constraint) -> bool {
    if actual.equality != Equality::LessThanOrEqualToZero
        || expected.equality != Equality::LessThanOrEqualToZero
    {
        return false;
    }
    let Some(actual) = actual.function().as_linear() else {
        return false;
    };
    let Some(expected) = expected.function().as_linear() else {
        return false;
    };
    actual.as_ref() == expected.as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, linear, quadratic, ATol, AcyclicAssignments, DecisionVariable, Evaluate, Function,
        IndicatorConstraint, ModelingLabel, NamedFunction, OneHotConstraint, RemovedReason, Sense,
    };

    fn member_binary_id() -> VariableID {
        VariableID::from(0)
    }

    fn member_integer_id() -> VariableID {
        VariableID::from(1)
    }

    fn selector_id() -> VariableID {
        VariableID::from(10)
    }

    fn unrelated_id() -> VariableID {
        VariableID::from(20)
    }

    fn upper_row_id() -> ConstraintID {
        ConstraintID::from(100)
    }

    fn lower_row_id() -> ConstraintID {
        ConstraintID::from(101)
    }

    fn cardinality_row_id() -> ConstraintID {
        ConstraintID::from(102)
    }

    fn integer(lower: f64, upper: f64) -> DecisionVariable {
        DecisionVariable::new(
            Kind::Integer,
            Bound::new(lower, upper).unwrap(),
            ATol::default(),
        )
        .unwrap()
    }

    fn mixed_instance() -> (Instance, Sos1BigMPromotionWitness) {
        let constraints = BTreeMap::from([
            (
                upper_row_id(),
                canonical_sos1_big_m_upper_link(member_integer_id(), selector_id(), 3.0).unwrap(),
            ),
            (
                lower_row_id(),
                canonical_sos1_big_m_lower_link(member_integer_id(), selector_id(), -2.0).unwrap(),
            ),
            (
                cardinality_row_id(),
                canonical_sos1_big_m_cardinality([member_binary_id(), selector_id()]).unwrap(),
            ),
        ]);
        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([
                (member_binary_id(), DecisionVariable::binary()),
                (member_integer_id(), integer(-2.0, 3.0)),
                (selector_id(), DecisionVariable::binary()),
                (unrelated_id(), DecisionVariable::continuous()),
            ]))
            .constraints(constraints)
            .build()
            .unwrap();
        let witness = Sos1BigMPromotionWitness::new(
            BTreeMap::from([
                (member_binary_id(), Sos1BigMSelectorWitness::Reused),
                (
                    member_integer_id(),
                    Sos1BigMSelectorWitness::Fresh {
                        selector: selector_id(),
                        upper_link: Some(upper_row_id()),
                        lower_link: Some(lower_row_id()),
                    },
                ),
            ]),
            cardinality_row_id(),
        );
        (instance, witness)
    }

    fn fresh_instance(
        member: DecisionVariable,
        upper_link: Option<ConstraintID>,
        lower_link: Option<ConstraintID>,
    ) -> (Instance, Sos1BigMPromotionWitness) {
        let bound = member.bound();
        let mut constraints = BTreeMap::new();
        if let Some(id) = upper_link {
            constraints.insert(
                id,
                canonical_sos1_big_m_upper_link(member_integer_id(), selector_id(), bound.upper())
                    .unwrap(),
            );
        }
        if let Some(id) = lower_link {
            constraints.insert(
                id,
                canonical_sos1_big_m_lower_link(member_integer_id(), selector_id(), bound.lower())
                    .unwrap(),
            );
        }
        constraints.insert(
            cardinality_row_id(),
            canonical_sos1_big_m_cardinality([selector_id()]).unwrap(),
        );
        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([
                (member_integer_id(), member),
                (selector_id(), DecisionVariable::binary()),
            ]))
            .constraints(constraints)
            .build()
            .unwrap();
        let witness = Sos1BigMPromotionWitness::new(
            BTreeMap::from([(
                member_integer_id(),
                Sos1BigMSelectorWitness::Fresh {
                    selector: selector_id(),
                    upper_link,
                    lower_link,
                },
            )]),
            cardinality_row_id(),
        );
        (instance, witness)
    }

    fn assert_atomic_rejection(
        mut instance: Instance,
        witness: &Sos1BigMPromotionWitness,
        expected: &str,
    ) {
        let before = instance.clone();
        let error = instance.promote_sos1_big_m(witness).unwrap_err();
        assert!(
            error.to_string().contains(expected),
            "expected {expected:?} in error, got: {error:#}"
        );
        assert_eq!(instance, before);
    }

    #[test]
    fn promotes_mixed_canonical_formulation_and_retains_history() {
        let (mut instance, witness) = mixed_instance();
        let promotion = instance.promote_sos1_big_m(&witness).unwrap();

        assert_eq!(promotion.sos1_constraint_id(), Sos1ConstraintID::from(0));
        assert_eq!(
            promotion.members(),
            &VariableIDSet::from([member_binary_id(), member_integer_id()])
        );
        assert_eq!(
            promotion.fresh_selectors(),
            &BTreeMap::from([(member_integer_id(), selector_id())])
        );
        assert_eq!(
            promotion.relaxed_constraint_ids(),
            &BTreeSet::from([upper_row_id(), lower_row_id(), cardinality_row_id()])
        );
        assert!(instance.decision_variables().contains_key(&selector_id()));
        assert!(instance.constraints().is_empty());
        assert_eq!(instance.removed_constraints().len(), 3);
        assert!(instance
            .removed_constraints()
            .values()
            .all(|(_, reason)| reason.reason == "promoted canonical SOS1 Big-M formulation"));
        assert_eq!(
            &instance.sos1_constraints()[&Sos1ConstraintID::from(0)].variables,
            promotion.members()
        );
        assert!(instance.decision_variables().contains_key(&unrelated_id()));
        assert!(matches!(
            instance.decision_variable_dependency().get(&selector_id()),
            Some(DependentExpr::NonzeroIndicator(_))
        ));

        let zero = instance
            .populate_state(
                crate::v1::State::from_iter([(0, 0.0), (1, 0.0)]),
                ATol::default(),
            )
            .unwrap();
        assert_eq!(zero.entries[&10], 0.0);
        let nonzero = instance
            .populate_state(
                crate::v1::State::from_iter([(0, 0.0), (1, -2.0)]),
                ATol::default(),
            )
            .unwrap();
        assert_eq!(nonzero.entries[&10], 1.0);
    }

    #[test]
    fn accepts_finite_member_bounds_that_exclude_zero() {
        let (mut positive, witness) = fresh_instance(integer(1.0, 3.0), Some(upper_row_id()), None);
        let _promotion = positive.promote_sos1_big_m(&witness).unwrap();
        assert_eq!(positive.sos1_constraints().len(), 1);

        let (mut negative, witness) =
            fresh_instance(integer(-3.0, -1.0), None, Some(lower_row_id()));
        let _promotion = negative.promote_sos1_big_m(&witness).unwrap();
        assert_eq!(negative.sos1_constraints().len(), 1);
    }

    #[test]
    fn all_reused_members_need_no_dependency() {
        let variables = BTreeMap::from([
            (VariableID::from(3), DecisionVariable::binary()),
            (VariableID::from(4), DecisionVariable::binary()),
        ]);
        let cardinality = ConstraintID::from(8);
        let mut instance = Instance::new(
            Sense::Minimize,
            Function::Zero,
            variables,
            BTreeMap::from([(
                cardinality,
                canonical_sos1_big_m_cardinality([VariableID::from(3), VariableID::from(4)])
                    .unwrap(),
            )]),
        )
        .unwrap();
        let witness = Sos1BigMPromotionWitness::new(
            BTreeMap::from([
                (VariableID::from(3), Sos1BigMSelectorWitness::Reused),
                (VariableID::from(4), Sos1BigMSelectorWitness::Reused),
            ]),
            cardinality,
        );
        let promotion = instance.promote_sos1_big_m(&witness).unwrap();
        assert!(promotion.fresh_selectors().is_empty());
        assert!(instance.decision_variable_dependency().is_empty());
        assert_eq!(instance.decision_variables().len(), 2);
        assert!(instance.constraints().is_empty());
        assert!(instance.removed_constraints().contains_key(&cardinality));
    }

    #[test]
    fn rejects_one_ulp_row_change_without_mutation() {
        let (mut instance, witness) = mixed_instance();
        let changed_upper = f64::from_bits(3.0f64.to_bits() + 1);
        instance
            .constraint_collection
            .replace_active_row(
                upper_row_id(),
                canonical_sos1_big_m_upper_link(member_integer_id(), selector_id(), changed_upper)
                    .unwrap(),
            )
            .unwrap();
        assert_atomic_rejection(instance, &witness, "does not match");
    }

    #[test]
    fn rejects_nonlinear_payload_with_canonical_looking_linear_terms() {
        let (mut instance, witness) = mixed_instance();
        let linear_terms = (quadratic!(1) + (coeff!(-3.0) * quadratic!(10)).unwrap()).unwrap();
        let nonlinear = Function::from((linear_terms + quadratic!(1, 10)).unwrap());
        instance
            .constraint_collection
            .replace_active_row(
                upper_row_id(),
                Constraint::less_than_or_equal_to_zero(nonlinear),
            )
            .unwrap();

        assert_atomic_rejection(instance, &witness, "does not match");
    }

    #[test]
    fn rejects_unmodeled_member_domains_without_mutation() {
        let fixed_binary =
            DecisionVariable::new(Kind::Binary, Bound::new(1.0, 1.0).unwrap(), ATol::default())
                .unwrap();
        let (instance, witness) = fresh_instance(fixed_binary, Some(upper_row_id()), None);
        assert_atomic_rejection(instance, &witness, "Binary SOS1 member");

        let semi = DecisionVariable::new(
            Kind::SemiContinuous,
            Bound::new(-2.0, 3.0).unwrap(),
            ATol::default(),
        )
        .unwrap();
        let (instance, witness) = fresh_instance(semi, Some(upper_row_id()), Some(lower_row_id()));
        assert_atomic_rejection(instance, &witness, "semi-variable");
    }

    #[test]
    fn rejects_structurally_invalid_witnesses_without_mutation() {
        let mut empty = Instance::default();
        let empty_witness = Sos1BigMPromotionWitness::new(BTreeMap::new(), ConstraintID::from(0));
        let before = empty.clone();
        assert!(empty
            .promote_sos1_big_m(&empty_witness)
            .unwrap_err()
            .to_string()
            .contains("at least one member"));
        assert_eq!(empty, before);

        let (instance, mut collision) = mixed_instance();
        collision.selector_roles.insert(
            member_integer_id(),
            Sos1BigMSelectorWitness::Fresh {
                selector: member_binary_id(),
                upper_link: Some(upper_row_id()),
                lower_link: Some(lower_row_id()),
            },
        );
        assert_atomic_rejection(instance, &collision, "collides");

        let (instance, mut duplicate_row) = mixed_instance();
        duplicate_row.cardinality_constraint = upper_row_id();
        assert_atomic_rejection(instance, &duplicate_row, "also claimed");
    }

    #[test]
    fn preserves_selector_labels_and_relaxed_row_context() {
        let (mut instance, witness) = mixed_instance();
        instance
            .set_variable_label(
                selector_id(),
                ModelingLabel {
                    name: Some("user_selector".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();
        instance
            .set_constraint_context(
                upper_row_id(),
                ConstraintContext {
                    label: ModelingLabel {
                        name: Some("user_link".to_string()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .unwrap();
        let _ = instance.promote_sos1_big_m(&witness).unwrap();

        assert_eq!(
            instance.variable_labels().name(selector_id()),
            Some("user_selector")
        );
        assert_eq!(
            instance.constraint_context().name(upper_row_id()),
            Some("user_link")
        );
        assert!(instance.removed_constraints().contains_key(&upper_row_id()));
    }

    #[test]
    fn fresh_selector_isolation_rejects_retained_active_solver_usage() {
        let (base, witness) = mixed_instance();

        let mut instance = base.clone();
        instance.set_objective(Function::from(linear!(10))).unwrap();
        assert_atomic_rejection(instance, &witness, "the objective");

        let mut instance = base.clone();
        instance
            .add_constraint(
                Constraint::less_than_or_equal_to_zero(Function::from(linear!(10))),
                Default::default(),
            )
            .unwrap();
        assert_atomic_rejection(instance, &witness, "retained active regular");

        let mut instance = base.clone();
        instance
            .add_indicator_constraint(
                IndicatorConstraint::new(
                    selector_id(),
                    Equality::LessThanOrEqualToZero,
                    Function::Zero,
                ),
                Default::default(),
            )
            .unwrap();
        assert_atomic_rejection(instance, &witness, "active Indicator");

        let mut instance = base.clone();
        instance
            .add_one_hot_constraint(
                OneHotConstraint::new(BTreeSet::from([selector_id()])).unwrap(),
                Default::default(),
            )
            .unwrap();
        assert_atomic_rejection(instance, &witness, "active OneHot");

        let mut instance = base.clone();
        instance
            .add_sos1_constraint(
                Sos1Constraint::new(BTreeSet::from([selector_id()])).unwrap(),
                Default::default(),
            )
            .unwrap();
        assert_atomic_rejection(instance, &witness, "active SOS1");

        let mut instance = base.clone();
        instance.decision_variable_dependency =
            AcyclicAssignments::new([(selector_id(), Function::Zero)])
                .unwrap()
                .into();
        assert_atomic_rejection(instance, &witness, "dependency target");

        let mut instance = base;
        instance
            .decision_variables
            .set_fixed_value(selector_id(), 0.0, ATol::default())
            .unwrap();
        assert_atomic_rejection(instance, &witness, "is fixed");
    }

    #[test]
    fn removed_named_and_dependency_rhs_references_are_preserved() {
        let (mut instance, witness) = mixed_instance();

        let regular_id = instance
            .add_constraint(
                Constraint::less_than_or_equal_to_zero(Function::from(linear!(10))),
                Default::default(),
            )
            .unwrap();
        instance
            .relax_constraint(regular_id, "test".to_string(), [])
            .unwrap();

        let indicator_id = instance
            .add_indicator_constraint(
                IndicatorConstraint::new(
                    selector_id(),
                    Equality::LessThanOrEqualToZero,
                    Function::Zero,
                ),
                Default::default(),
            )
            .unwrap();
        instance
            .indicator_constraint_collection
            .relax(indicator_id, test_removed_reason())
            .unwrap();

        let one_hot_id = instance
            .add_one_hot_constraint(
                OneHotConstraint::new(BTreeSet::from([selector_id()])).unwrap(),
                Default::default(),
            )
            .unwrap();
        instance
            .one_hot_constraint_collection
            .relax(one_hot_id, test_removed_reason())
            .unwrap();

        let removed_sos1_id = instance
            .add_sos1_constraint(
                Sos1Constraint::new(BTreeSet::from([selector_id()])).unwrap(),
                Default::default(),
            )
            .unwrap();
        instance
            .sos1_constraint_collection
            .relax(removed_sos1_id, test_removed_reason())
            .unwrap();

        instance
            .named_functions
            .insert(
                crate::NamedFunctionID::from(0),
                NamedFunction {
                    function: Function::from(linear!(10)),
                },
                Default::default(),
            )
            .unwrap();
        instance.decision_variable_dependency =
            AcyclicAssignments::new([(unrelated_id(), Function::from(linear!(10)))])
                .unwrap()
                .into();

        let _ = instance.promote_sos1_big_m(&witness).unwrap();

        assert!(instance.removed_constraints().contains_key(&regular_id));
        assert!(instance
            .removed_indicator_constraints()
            .contains_key(&indicator_id));
        assert!(instance
            .removed_one_hot_constraints()
            .contains_key(&one_hot_id));
        assert!(instance
            .removed_sos1_constraints()
            .contains_key(&removed_sos1_id));
        assert!(instance
            .named_functions()
            .contains_key(&crate::NamedFunctionID::from(0)));
        assert_eq!(instance.decision_variable_dependency().len(), 2);

        let solution = instance
            .evaluate(
                &crate::v1::State::from_iter([(0, 0.0), (1, 0.0)]),
                ATol::default(),
            )
            .unwrap();
        assert_eq!(solution.state().entries[&selector_id().into_inner()], 0.0);
        assert_eq!(solution.state().entries[&unrelated_id().into_inner()], 0.0);
    }

    #[test]
    fn dependent_selector_reconstruction_validates_input_state() {
        let (mut instance, witness) = mixed_instance();
        let _ = instance.promote_sos1_big_m(&witness).unwrap();

        let inconsistent = instance
            .populate_state(
                crate::v1::State::from_iter([(0, 0.0), (1, 0.0), (10, 1.0)]),
                ATol::default(),
            )
            .unwrap_err();
        assert!(inconsistent.is::<crate::InconsistentDependentValue>());
        assert!(instance
            .populate_state(
                crate::v1::State::from_iter([(0, 0.0), (1, f64::NAN)]),
                ATol::default(),
            )
            .unwrap_err()
            .to_string()
            .contains("must be finite"));
    }

    fn test_removed_reason() -> RemovedReason {
        RemovedReason {
            reason: "test".to_string(),
            parameters: Default::default(),
        }
    }
}
