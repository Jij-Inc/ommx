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
//! `Base(x) AND SOS1(x)`.
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
    v1, Bound, Constraint, ConstraintContext, ConstraintID, Equality, Evaluate, Kind,
    Sos1Constraint, Sos1ConstraintID, VariableID, VariableIDSet,
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

/// Result and exact raw-state map for one checked SOS1 Big-M promotion.
///
/// `project_state` removes only verified fresh-selector coordinates. The
/// canonical `lift_state` reconstructs each such selector as `0` exactly when
/// its member equals `0.0`, and as `1` otherwise. Consequently
/// `project_state(lift_state(target)) == target`; the reverse round trip is not
/// promised because a feasible source may contain a non-canonical selector
/// value when its member is zero.
///
/// These maps operate on complete finite raw states. They are exact
/// representation bookkeeping, not tolerance-based feasibility classifiers.
#[must_use = "retain the state map when states cross the promotion boundary"]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sos1BigMPromotion {
    sos1_constraint_id: Sos1ConstraintID,
    members: VariableIDSet,
    fresh_selectors: BTreeMap<VariableID, VariableID>,
    consumed_constraint_ids: BTreeSet<ConstraintID>,
    before_variable_ids: VariableIDSet,
    after_variable_ids: VariableIDSet,
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

    /// Regular formulation rows permanently consumed by the promotion.
    pub fn consumed_constraint_ids(&self) -> &BTreeSet<ConstraintID> {
        &self.consumed_constraint_ids
    }

    /// Complete variable-ID set immediately before promotion.
    pub fn before_variable_ids(&self) -> &VariableIDSet {
        &self.before_variable_ids
    }

    /// Complete variable-ID set immediately after promotion.
    pub fn after_variable_ids(&self) -> &VariableIDSet {
        &self.after_variable_ids
    }

    /// Project a complete finite pre-promotion state to the promoted instance.
    pub fn project_state(&self, before: &v1::State) -> crate::Result<v1::State> {
        validate_state(before, &self.before_variable_ids, "pre-promotion")?;
        let mut entries = before.entries.clone();
        for selector in self.fresh_selectors.values() {
            entries.remove(&selector.into_inner()).ok_or_else(|| {
                crate::error!(
                    { ?selector },
                    "Pre-promotion state is missing fresh selector {selector:?}"
                )
            })?;
        }
        let after = v1::State { entries };
        validate_state(&after, &self.after_variable_ids, "post-promotion")?;
        Ok(after)
    }

    /// Canonically lift a complete finite post-promotion state.
    ///
    /// Mathematical exact zero is used: both `0.0` and `-0.0` produce selector
    /// value `0.0`; every other finite member value produces `1.0`.
    pub fn lift_state(&self, after: &v1::State) -> crate::Result<v1::State> {
        validate_state(after, &self.after_variable_ids, "post-promotion")?;
        let mut entries = after.entries.clone();
        for (&member, &selector) in &self.fresh_selectors {
            let member_value = entries.get(&member.into_inner()).copied().ok_or_else(|| {
                crate::error!(
                    { ?member },
                    "Post-promotion state is missing SOS1 member {member:?}"
                )
            })?;
            let selector_value = if member_value == 0.0 { 0.0 } else { 1.0 };
            if entries
                .insert(selector.into_inner(), selector_value)
                .is_some()
            {
                crate::bail!(
                    { ?selector },
                    "Post-promotion state unexpectedly contains fresh selector {selector:?}"
                );
            }
        }
        let before = v1::State { entries };
        validate_state(&before, &self.before_variable_ids, "pre-promotion")?;
        Ok(before)
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
    /// - absence of every fresh selector from the objective, retained active
    ///   rows, removed rows, every special-constraint family, named functions,
    ///   dependency keys/RHS expressions, and fixed-value state.
    ///
    /// Rust-side concepts not modeled by the initial Lean semantics are
    /// handled conservatively. Selected semi variables, fixed or dependent
    /// members, fixed binary member bounds, non-default consumed-row context,
    /// and non-default fresh-selector labels are rejected. Unrelated nonlinear
    /// expressions and unrelated special constraints are preserved unchanged.
    /// Calling this family-specific method is the explicit request to add the
    /// SOS1 capability to the instance; witness rejection means only that the
    /// claimed formulation is outside this conservative checker.
    ///
    /// The correctness contract uses exact algebraic feasibility. It does not
    /// claim equality of [`Evaluate`] results for a positive [`crate::ATol`],
    /// because regular rows and SOS1 constraints apply different tolerance
    /// classifiers near zero.
    ///
    /// On success the verified formulation rows and fresh selectors are
    /// permanently consumed and a new active SOS1 constraint is inserted. The
    /// operation is atomic: every change is applied to a staged clone before
    /// replacing `self`.
    pub fn promote_sos1_big_m(
        &mut self,
        witness: &Sos1BigMPromotionWitness,
    ) -> crate::Result<Sos1BigMPromotion> {
        let plan = self.plan_sos1_big_m_promotion(witness)?;
        let mut staged = self.clone();
        staged
            .constraint_collection
            .consume_active_rows(&plan.result.consumed_constraint_ids)?;
        let fresh_selector_ids = plan
            .result
            .fresh_selectors
            .values()
            .copied()
            .collect::<BTreeSet<_>>();
        staged
            .decision_variables
            .remove_unfixed_rows(&fresh_selector_ids)?;
        staged
            .sos1_constraint_collection
            .insert_active_with_context(
                plan.result.sos1_constraint_id,
                Sos1Constraint::new(plan.result.members.clone())?,
                ConstraintContext::default(),
            )?;

        debug_assert_eq!(
            staged
                .decision_variables()
                .keys()
                .copied()
                .collect::<VariableIDSet>(),
            plan.result.after_variable_ids
        );
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
        let mut consumed_constraint_ids = BTreeSet::new();

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
                    if self.variable_labels().collect_for(selector) != Default::default() {
                        crate::bail!(
                            { ?member, ?selector },
                            "Fresh SOS1 selector {selector:?} has a modeling label that promotion would discard"
                        );
                    }

                    self.validate_optional_sos1_link(
                        member,
                        selector,
                        upper_link,
                        bound.upper() > 0.0,
                        "upper",
                        || canonical_sos1_big_m_upper_link(member, selector, bound.upper()),
                        &mut consumed_constraint_ids,
                    )?;
                    self.validate_optional_sos1_link(
                        member,
                        selector,
                        lower_link,
                        bound.lower() < 0.0,
                        "lower",
                        || canonical_sos1_big_m_lower_link(member, selector, bound.lower()),
                        &mut consumed_constraint_ids,
                    )?;
                    fresh_selectors.insert(member, selector);
                }
            }
        }

        if !consumed_constraint_ids.insert(witness.cardinality_constraint) {
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

        for id in &consumed_constraint_ids {
            if self.constraint_context().collect_for(*id) != ConstraintContext::default() {
                crate::bail!(
                    { ?id },
                    "SOS1 formulation constraint {id:?} has context that promotion would discard"
                );
            }
        }
        self.ensure_variables_isolated_for_sos1_promotion(
            &fresh_selector_ids,
            &consumed_constraint_ids,
        )?;

        let before_variable_ids = self
            .decision_variables()
            .keys()
            .copied()
            .collect::<VariableIDSet>();
        let after_variable_ids = before_variable_ids
            .difference(&fresh_selector_ids)
            .copied()
            .collect::<VariableIDSet>();
        let sos1_constraint_id = self.next_sos1_constraint_id()?;

        Ok(Sos1BigMPromotionPlan {
            result: Sos1BigMPromotion {
                sos1_constraint_id,
                members,
                fresh_selectors,
                consumed_constraint_ids,
                before_variable_ids,
                after_variable_ids,
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
        consumed: &mut BTreeSet<ConstraintID>,
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
                if !consumed.insert(id) {
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

    /// Exhaustively prove that fresh coordinates occur only in consumed rows.
    ///
    /// `DecisionVariableUsage` intentionally indexes only active solver input,
    /// so it is insufficient for a representation-compression operation that
    /// deletes variables from the root object.
    fn ensure_variables_isolated_for_sos1_promotion(
        &self,
        private_ids: &VariableIDSet,
        consumed_regular_rows: &BTreeSet<ConstraintID>,
    ) -> crate::Result<()> {
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
        for (id, function) in self.decision_variable_dependency.iter() {
            if private_ids.contains(id) {
                crate::bail!(
                    { ?id },
                    "Fresh SOS1 selector {id:?} is a dependency target"
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
            crate::bail!({ ?id }, "Fresh SOS1 selector {id:?} is fixed");
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

fn reject_required_ids(
    private_ids: &VariableIDSet,
    required_ids: &VariableIDSet,
    location: &str,
) -> crate::Result<()> {
    if let Some(id) = private_ids.intersection(required_ids).next() {
        crate::bail!(
            { ?id, location },
            "Fresh SOS1 selector {id:?} is used by {location}"
        );
    }
    Ok(())
}

fn validate_state(
    state: &v1::State,
    expected_ids: &VariableIDSet,
    side: &'static str,
) -> crate::Result<()> {
    let actual_ids = state
        .entries
        .keys()
        .copied()
        .map(VariableID::from)
        .collect::<VariableIDSet>();
    if &actual_ids != expected_ids {
        crate::bail!(
            { side },
            "{side} state has a different variable-ID set from its SOS1 promotion"
        );
    }
    for (&id, &value) in &state.entries {
        if !value.is_finite() {
            crate::bail!(
                { id, value, side },
                "{side} state value for variable {} is not finite",
                VariableID::from(id)
            );
        }
    }
    Ok(())
}

#[cfg(test)]
fn state(entries: impl IntoIterator<Item = (u64, f64)>) -> v1::State {
    v1::State {
        entries: entries.into_iter().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, linear, quadratic, ATol, AcyclicAssignments, DecisionVariable, Function,
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
    fn promotes_mixed_canonical_formulation_and_maps_states() {
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
            promotion.consumed_constraint_ids(),
            &BTreeSet::from([upper_row_id(), lower_row_id(), cardinality_row_id()])
        );
        assert!(!instance.decision_variables().contains_key(&selector_id()));
        assert!(instance.constraints().is_empty());
        assert!(instance.removed_constraints().is_empty());
        assert_eq!(
            &instance.sos1_constraints()[&Sos1ConstraintID::from(0)].variables,
            promotion.members()
        );
        assert!(instance.decision_variables().contains_key(&unrelated_id()));

        // Projection accepts a feasible but non-canonical source selector.
        let before = state([(0, 0.0), (1, 0.0), (10, 1.0), (20, 7.0)]);
        let after = promotion.project_state(&before).unwrap();
        assert_eq!(after, state([(0, 0.0), (1, 0.0), (20, 7.0)]));
        let canonical = promotion.lift_state(&after).unwrap();
        assert_eq!(canonical.entries[&10], 0.0);
        assert_ne!(canonical, before);

        // The direction promised by the promotion is a section law.
        let target = state([(0, 0.0), (1, -2.0), (20, 4.0)]);
        assert_eq!(
            promotion
                .project_state(&promotion.lift_state(&target).unwrap())
                .unwrap(),
            target
        );
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
    fn all_reused_members_produce_an_identity_state_map() {
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
        assert_eq!(
            promotion.before_variable_ids(),
            promotion.after_variable_ids()
        );
        let original = state([(3, -0.0), (4, 1.0)]);
        let projected = promotion.project_state(&original).unwrap();
        let lifted = promotion.lift_state(&projected).unwrap();
        assert_eq!(projected.entries[&3].to_bits(), (-0.0f64).to_bits());
        assert_eq!(lifted.entries[&3].to_bits(), (-0.0f64).to_bits());
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
    fn rejects_metadata_that_would_be_discarded() {
        let (mut labeled_selector, witness) = mixed_instance();
        labeled_selector
            .set_variable_label(
                selector_id(),
                ModelingLabel {
                    name: Some("user_selector".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_atomic_rejection(labeled_selector, &witness, "modeling label");

        let (mut contextual_row, witness) = mixed_instance();
        contextual_row
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
        assert_atomic_rejection(contextual_row, &witness, "context");
    }

    #[test]
    fn fresh_selector_isolation_covers_every_instance_owner() {
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
        assert_atomic_rejection(instance, &witness, "active regular");

        let mut instance = base.clone();
        let id = instance
            .add_constraint(
                Constraint::less_than_or_equal_to_zero(Function::from(linear!(10))),
                Default::default(),
            )
            .unwrap();
        instance
            .relax_constraint(id, "test".to_string(), [])
            .unwrap();
        assert_atomic_rejection(instance, &witness, "removed regular");

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
        let id = instance
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
            .relax(
                id,
                RemovedReason {
                    reason: "test".to_string(),
                    parameters: Default::default(),
                },
            )
            .unwrap();
        assert_atomic_rejection(instance, &witness, "removed Indicator");

        let mut instance = base.clone();
        instance
            .add_one_hot_constraint(
                OneHotConstraint::new(BTreeSet::from([selector_id()])).unwrap(),
                Default::default(),
            )
            .unwrap();
        assert_atomic_rejection(instance, &witness, "active OneHot");

        let mut instance = base.clone();
        let id = instance
            .add_one_hot_constraint(
                OneHotConstraint::new(BTreeSet::from([selector_id()])).unwrap(),
                Default::default(),
            )
            .unwrap();
        instance
            .one_hot_constraint_collection
            .relax(
                id,
                RemovedReason {
                    reason: "test".to_string(),
                    parameters: Default::default(),
                },
            )
            .unwrap();
        assert_atomic_rejection(instance, &witness, "removed OneHot");

        let mut instance = base.clone();
        instance
            .add_sos1_constraint(
                Sos1Constraint::new(BTreeSet::from([selector_id()])).unwrap(),
                Default::default(),
            )
            .unwrap();
        assert_atomic_rejection(instance, &witness, "active SOS1");

        let mut instance = base.clone();
        let id = instance
            .add_sos1_constraint(
                Sos1Constraint::new(BTreeSet::from([selector_id()])).unwrap(),
                Default::default(),
            )
            .unwrap();
        instance
            .sos1_constraint_collection
            .relax(
                id,
                RemovedReason {
                    reason: "test".to_string(),
                    parameters: Default::default(),
                },
            )
            .unwrap();
        assert_atomic_rejection(instance, &witness, "removed SOS1");

        let mut instance = base.clone();
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
        assert_atomic_rejection(instance, &witness, "named function");

        let mut instance = base.clone();
        instance.decision_variable_dependency =
            AcyclicAssignments::new([(unrelated_id(), Function::from(linear!(10)))]).unwrap();
        assert_atomic_rejection(instance, &witness, "decision-variable dependency");

        let mut instance = base.clone();
        instance.decision_variable_dependency =
            AcyclicAssignments::new([(selector_id(), Function::Zero)]).unwrap();
        assert_atomic_rejection(instance, &witness, "dependency target");

        let mut instance = base;
        instance
            .decision_variables
            .set_fixed_value(selector_id(), 0.0, ATol::default())
            .unwrap();
        assert_atomic_rejection(instance, &witness, "is fixed");
    }

    #[test]
    fn state_map_rejects_stale_and_nonfinite_states() {
        let (mut instance, witness) = mixed_instance();
        let promotion = instance.promote_sos1_big_m(&witness).unwrap();

        assert!(promotion
            .project_state(&state([(0, 0.0), (1, 0.0), (10, 0.0)]))
            .unwrap_err()
            .to_string()
            .contains("variable-ID set"));
        assert!(promotion
            .project_state(&state([(0, 0.0), (1, f64::NAN), (10, 0.0), (20, 0.0),]))
            .unwrap_err()
            .to_string()
            .contains("not finite"));
        assert!(promotion
            .lift_state(&state([(0, 0.0), (1, 0.0), (10, 0.0), (20, 0.0)]))
            .unwrap_err()
            .to_string()
            .contains("variable-ID set"));
    }
}
