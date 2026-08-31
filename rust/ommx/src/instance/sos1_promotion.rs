//! Checked promotion of SOS1 Big-M formulations.
//!
//! The promotion request accepted here is deliberately untrusted. It identifies
//! stable OMMX IDs and their claimed roles; the current [`Instance`] remains the
//! sole source of truth for domains and row contents. A successful promotion
//! rewrites the active formulation from
//!
//! `Base(x) AND ValidatedSos1BigM(x, z)`
//!
//! into
//!
//! `Base(x) AND SOS1(x)`
//!
//! while retaining the complete formulation history in the same [`Instance`].
//! Verified Big-M rows move to the removed collection, and fresh selectors
//! become dependent variables reconstructed by a composed [`Function`].
//!
//! The reusable proof plan remains private and is applied only to a staged
//! clone. This keeps request rejection atomic and prevents a checked plan from
//! being reused after the instance changes.

use super::Instance;
use crate::{
    ATol, Bound, Constraint, ConstraintContext, ConstraintID, Equality, Evaluate, Function, Kind,
    Linear, LinearMonomial, RemovedReason, Sos1Constraint, Sos1ConstraintID, VariableID,
    VariableIDSet,
};
use std::collections::{BTreeMap, BTreeSet};

/// Claimed selector role for one member of an SOS1 Big-M formulation.
///
/// This is an unchecked claim, not a verified fact.
/// [`Instance::promote_sos1_big_m`] checks the role against the current member
/// domain and the meaning of the claimed regular rows before changing the
/// instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sos1BigMSelectorClaim {
    /// The member itself is claimed to be a full-domain binary selector.
    Reused,
    /// A separate private binary selector is claimed for the member.
    Fresh {
        /// Private binary selector associated with the member.
        selector: VariableID,
        /// Upper link whose positive normalization is
        /// `member - M * selector <= 0`, when required.
        upper_link: Option<ConstraintID>,
        /// Lower link whose positive normalization is
        /// `-member - M * selector <= 0`, when required.
        lower_link: Option<ConstraintID>,
    },
}

/// Untrusted stable-ID request for one SOS1 Big-M promotion.
///
/// Map keys are the intended SOS1 members. Each value claims whether the
/// member is reused as its selector or linked to a fresh selector. The final
/// regular row is identified separately because it must be the exact
/// cardinality constraint over all claimed selectors.
///
/// Bounds and coefficients are intentionally absent: validation always reads
/// them from the current [`Instance`]. The request is runtime-only and has no
/// serialization contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sos1BigMPromotionRequest {
    selector_claims: BTreeMap<VariableID, Sos1BigMSelectorClaim>,
    cardinality_constraint: ConstraintID,
}

impl Sos1BigMPromotionRequest {
    /// Create an untrusted promotion request. No instance-dependent validation
    /// occurs until [`Instance::promote_sos1_big_m`] is called.
    pub fn new(
        selector_claims: BTreeMap<VariableID, Sos1BigMSelectorClaim>,
        cardinality_constraint: ConstraintID,
    ) -> Self {
        Self {
            selector_claims,
            cardinality_constraint,
        }
    }

    /// Claimed member-to-selector roles, keyed by intended SOS1 member ID.
    pub fn selector_claims(&self) -> &BTreeMap<VariableID, Sos1BigMSelectorClaim> {
        &self.selector_claims
    }

    /// Claimed canonical selector-cardinality row.
    pub fn cardinality_constraint(&self) -> ConstraintID {
        self.cardinality_constraint
    }
}

/// Result of one checked SOS1 Big-M promotion.
///
/// State reconstruction is owned by the mutated [`Instance`]: each fresh
/// selector remains registered and is assigned a composed [`Function`] evaluated
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

    /// Verified formulation rows moved from active to removed.
    pub fn relaxed_constraint_ids(&self) -> &BTreeSet<ConstraintID> {
        &self.relaxed_constraint_ids
    }
}

#[derive(Debug)]
struct Sos1BigMPromotionPlan {
    result: Sos1BigMPromotion,
}

#[derive(Debug, Clone, Copy)]
enum Sos1LinkSide {
    Upper,
    Lower,
}

impl Sos1LinkSide {
    fn name(self) -> &'static str {
        match self {
            Self::Upper => "upper",
            Self::Lower => "lower",
        }
    }

    fn is_required(self, bound: Bound) -> bool {
        match self {
            Self::Upper => bound.upper() > 0.0,
            Self::Lower => bound.lower() < 0.0,
        }
    }

    fn member_coefficient_has_expected_sign(self, coefficient: f64) -> bool {
        match self {
            Self::Upper => coefficient > 0.0,
            Self::Lower => coefficient < 0.0,
        }
    }

    fn member_value(self, signed_value: f64) -> f64 {
        match self {
            Self::Upper => signed_value,
            Self::Lower => -signed_value,
        }
    }

    fn feasible_signed_domain(
        self,
        kind: Kind,
        bound: Bound,
        atol: ATol,
    ) -> crate::Result<SignedFeasibleDomain> {
        let tolerance = atol.into_inner();
        let (lower, upper, discrete) = match kind {
            Kind::Continuous => {
                let lower = bound.lower() - tolerance;
                let upper = bound.upper() + tolerance;
                if !lower.is_finite() || !upper.is_finite() {
                    crate::bail!(
                        { ?kind, ?bound, tolerance, side = self.name() },
                        "SOS1 Big-M promotion cannot certify a non-finite ATol-feasible member domain"
                    );
                }
                (lower, upper, false)
            }
            Kind::Integer => (bound.lower(), bound.upper(), true),
            _ => crate::bail!(
                { ?kind, side = self.name() },
                "SOS1 Big-M promotion cannot certify links for member kind {kind:?}"
            ),
        };
        let (lower, upper) = match self {
            Self::Upper => (lower, upper),
            Self::Lower => (-upper, -lower),
        };
        Ok(SignedFeasibleDomain {
            lower,
            upper,
            discrete,
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct SignedFeasibleDomain {
    lower: f64,
    upper: f64,
    discrete: bool,
}

#[derive(Debug, Clone, Copy)]
enum ActiveMinimum {
    /// A continuous domain contains values arbitrarily close to, but strictly
    /// above, the SOS1 zero threshold.
    OpenAtTolerance,
    /// The smallest active value is attained by the feasible domain.
    Attained(f64),
}

impl SignedFeasibleDomain {
    fn max_zero_classified(self, atol: ATol) -> Option<f64> {
        if self.discrete {
            // Promotion requires ATol < 1, so zero is the only integer value
            // classified as zero by SOS1 evaluation.
            return (self.lower <= 0.0 && 0.0 <= self.upper).then_some(0.0);
        }

        let tolerance = atol.into_inner();
        let lower = self.lower.max(-tolerance);
        let upper = self.upper.min(tolerance);
        (lower <= upper).then_some(upper)
    }

    fn min_active(self, atol: ATol) -> Option<ActiveMinimum> {
        let tolerance = atol.into_inner();
        if self.upper <= tolerance {
            return None;
        }
        if self.discrete {
            return Some(ActiveMinimum::Attained(self.lower.max(1.0)));
        }
        if self.lower > tolerance {
            Some(ActiveMinimum::Attained(self.lower))
        } else {
            Some(ActiveMinimum::OpenAtTolerance)
        }
    }
}

fn canonical_sos1_big_m_cardinality(
    selectors: impl IntoIterator<Item = VariableID>,
) -> crate::Result<Constraint> {
    let function =
        selectors
            .into_iter()
            .try_fold(Linear::from(crate::coeff!(-1.0)), |sum, selector| {
                sum + Linear::single_term(LinearMonomial::Variable(selector), crate::coeff!(1.0))
            })?;
    Ok(Constraint::less_than_or_equal_to_zero(Function::from(
        function,
    )))
}

fn populated_required_ids(
    function: &Function,
    dependencies: &crate::AcyclicAssignments,
) -> VariableIDSet {
    let mut closure = function.required_ids();
    let mut pending = closure.iter().copied().collect::<Vec<_>>();
    while let Some(id) = pending.pop() {
        let Some(dependency) = dependencies.get(&id) else {
            continue;
        };
        for required_id in dependency.required_ids() {
            if closure.insert(required_id) {
                pending.push(required_id);
            }
        }
    }
    closure
}

impl Instance {
    /// Promote one validated Big-M selector formulation to SOS1.
    ///
    /// The request is untrusted. This method validates all of the following
    /// against the current instance before mutation:
    ///
    /// - a non-empty member set with finite supported domains;
    /// - exact agreement between full binary members and reused-selector roles;
    /// - distinct full binary fresh selectors outside the member set;
    /// - upper and lower links that normalize to the expected two-variable
    ///   Big-M shape and preserve the claimed formulation's projected feasible
    ///   set under the supplied `atol`;
    /// - the exact canonical selector-cardinality row;
    /// - absence of every fresh selector from current active solver input,
    ///   except for the claimed formulation rows;
    /// - absence of every fresh selector from the output objective.
    ///
    /// Rust-side concepts not modeled by the initial Lean semantics are
    /// handled conservatively. Selected semi variables, fixed or dependent
    /// members, fixed binary member bounds, and already fixed/dependent fresh
    /// selectors are rejected. Removed constraints, named functions,
    /// dependency RHS expressions, row context, and selector labels remain
    /// valid history and are preserved unchanged. Direct or
    /// dependency-transitive output-objective references are rejected because
    /// dependent reconstruction would change the returned objective value.
    /// Unrelated nonlinear expressions and unrelated special constraints are
    /// preserved unchanged.
    /// Calling this family-specific method is the explicit request to add the
    /// SOS1 capability to the instance; request rejection means only that the
    /// claimed formulation is outside this conservative checker.
    ///
    /// For a raw link `a * (t - M * z) <= 0`, normalization changes the
    /// comparison tolerance to `atol / a`. The checker accepts the scale and
    /// Big-M only when that threshold agrees with SOS1's `abs(member) <= atol`
    /// zero classification: reconstructed `z = 0` must satisfy the raw row,
    /// every active member must force `z = 1`, and `z = 1` must cover the full
    /// member domain feasible under the same `atol`. Continuous domains include
    /// the tolerance admitted by [`Bound::contains`], while Integer domains are
    /// checked at their discrete values. The checker imposes no domain-derived
    /// upper cap on a finite, representable loose Big-M; non-unit scales and
    /// tolerance-level shortfalls are accepted only where these feasibility
    /// conditions hold. The raw residual is checked as well as its normalized
    /// form to avoid accepting an f64 normalization-rounding artifact.
    ///
    /// The supplied `atol` must be finite and smaller than one so the exact
    /// binary selector-cardinality row still means "at most one". Relation,
    /// variable support, zero constant, and cardinality remain exact structural
    /// requirements. Link presence remains determined by the exact sign of the
    /// stored domain endpoint. The equivalence guarantee is local to the claimed
    /// formulation and parameterized by this `atol`; callers must use the same
    /// tolerance for subsequent state reconstruction and evaluation. The
    /// transformed [`Instance`] does not store a global evaluation tolerance,
    /// and unrelated removed history is preserved rather than reinterpreted.
    ///
    /// On success the verified formulation rows are relaxed, fresh selectors
    /// remain registered as dependent variables, and a new active SOS1
    /// constraint is inserted. The operation is atomic: every change is
    /// applied to a staged clone before replacing `self`.
    pub fn promote_sos1_big_m(
        &mut self,
        request: &Sos1BigMPromotionRequest,
        atol: ATol,
    ) -> crate::Result<Sos1BigMPromotion> {
        let plan = self.plan_sos1_big_m_promotion(request, atol)?;
        let mut staged = self.clone();
        for &id in &plan.result.relaxed_constraint_ids {
            staged.constraint_collection.relax(
                id,
                RemovedReason {
                    reason: "promoted validated SOS1 Big-M formulation".to_string(),
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
                        Function::from(crate::linear!(member.into_inner()))
                            .signum()
                            .abs(),
                    )
                }),
        );
        staged.decision_variable_dependency = crate::AcyclicAssignments::new(dependencies)?;
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
        request: &Sos1BigMPromotionRequest,
        atol: ATol,
    ) -> crate::Result<Sos1BigMPromotionPlan> {
        if !atol.into_inner().is_finite() {
            crate::bail!(
                { atol = atol.into_inner() },
                "SOS1 Big-M promotion requires a finite ATol"
            );
        }
        if atol.into_inner() >= 1.0 {
            crate::bail!(
                { atol = atol.into_inner() },
                "SOS1 Big-M promotion requires ATol < 1 so binary selector cardinality and SOS1 zero classification agree"
            );
        }
        if request.selector_claims.is_empty() {
            crate::bail!("SOS1 Big-M promotion request must contain at least one member");
        }

        let members = request
            .selector_claims
            .keys()
            .copied()
            .collect::<VariableIDSet>();
        let mut fresh_selectors = BTreeMap::new();
        let mut fresh_selector_ids = VariableIDSet::new();
        let mut relaxed_constraint_ids = BTreeSet::new();

        for (&member, &claim) in &request.selector_claims {
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
            match claim {
                Sos1BigMSelectorClaim::Reused => {
                    if !is_full_binary {
                        crate::bail!(
                            { ?member, kind = ?variable.kind(), ?bound },
                            "Reused SOS1 selector {member:?} does not have the full binary domain [0, 1]"
                        );
                    }
                }
                Sos1BigMSelectorClaim::Fresh {
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
                        variable,
                        Sos1LinkSide::Upper,
                        atol,
                        &mut relaxed_constraint_ids,
                    )?;
                    self.validate_optional_sos1_link(
                        member,
                        selector,
                        lower_link,
                        variable,
                        Sos1LinkSide::Lower,
                        atol,
                        &mut relaxed_constraint_ids,
                    )?;
                    fresh_selectors.insert(member, selector);
                }
            }
        }

        if !relaxed_constraint_ids.insert(request.cardinality_constraint) {
            crate::bail!(
                { cardinality = ?request.cardinality_constraint },
                "SOS1 cardinality constraint is also claimed as a member link"
            );
        }
        let selector_ids = request
            .selector_claims
            .iter()
            .map(|(&member, claim)| match claim {
                Sos1BigMSelectorClaim::Reused => member,
                Sos1BigMSelectorClaim::Fresh { selector, .. } => *selector,
            });
        let expected_cardinality = canonical_sos1_big_m_cardinality(selector_ids)?;
        self.ensure_exact_sos1_formulation_row(
            request.cardinality_constraint,
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
        variable: &crate::DecisionVariable,
        side: Sos1LinkSide,
        atol: ATol,
        relaxed: &mut BTreeSet<ConstraintID>,
    ) -> crate::Result<()> {
        let side_name = side.name();
        let bound = variable.bound();
        match actual_id {
            None if !side.is_required(bound) => Ok(()),
            None => crate::bail!(
                { ?member, ?selector, side = side_name },
                "SOS1 member {member:?} is missing its required {side_name} link"
            ),
            Some(id) => {
                if !relaxed.insert(id) {
                    crate::bail!(
                        { ?member, ?selector, ?id, side = side_name },
                        "Regular constraint {id:?} is claimed for more than one SOS1 formulation role"
                    );
                }
                self.ensure_sufficient_sos1_link(id, member, selector, variable, side, atol)
            }
        }
    }

    fn ensure_sufficient_sos1_link(
        &self,
        id: ConstraintID,
        member: VariableID,
        selector: VariableID,
        variable: &crate::DecisionVariable,
        side: Sos1LinkSide,
        atol: ATol,
    ) -> crate::Result<()> {
        let side_name = side.name();
        let actual = self.constraints().get(&id).ok_or_else(|| {
            crate::error!(
                { ?id, side = side_name },
                "Claimed SOS1 {side_name} link constraint {id:?} is not active"
            )
        })?;
        if actual.equality != Equality::LessThanOrEqualToZero {
            crate::bail!(
                { ?id, side = side_name, equality = ?actual.equality },
                "Claimed SOS1 {side_name} link constraint {id:?} is not a less-than-or-equal-to-zero row"
            );
        }
        let linear = actual.function().as_linear().ok_or_else(|| {
            crate::error!(
                { ?id, side = side_name },
                "Claimed SOS1 {side_name} link constraint {id:?} is not linear"
            )
        })?;
        let member_monomial = LinearMonomial::Variable(member);
        let selector_monomial = LinearMonomial::Variable(selector);
        let (Some(member_coefficient), Some(selector_coefficient)) =
            (linear.get(&member_monomial), linear.get(&selector_monomial))
        else {
            crate::bail!(
                { ?id, ?member, ?selector, side = side_name },
                "Claimed SOS1 {side_name} link constraint {id:?} must contain exactly the member and selector terms with no constant"
            );
        };
        if linear.num_terms() != 2 {
            crate::bail!(
                { ?id, ?member, ?selector, side = side_name },
                "Claimed SOS1 {side_name} link constraint {id:?} must contain exactly the member and selector terms with no constant"
            );
        }

        let member_coefficient = member_coefficient.into_inner();
        if !side.member_coefficient_has_expected_sign(member_coefficient) {
            crate::bail!(
                { ?id, ?member, member_coefficient, side = side_name },
                "Claimed SOS1 {side_name} link constraint {id:?} has the wrong member-coefficient sign"
            );
        }
        let selector_coefficient = selector_coefficient.into_inner();
        if selector_coefficient >= 0.0 {
            crate::bail!(
                { ?id, ?selector, selector_coefficient, side = side_name },
                "Claimed SOS1 {side_name} link constraint {id:?} must have a negative selector coefficient"
            );
        }

        let scale = member_coefficient.abs();
        let big_m = -selector_coefficient / scale;
        if !big_m.is_finite() || big_m <= 0.0 {
            crate::bail!(
                { ?id, member_coefficient, selector_coefficient, big_m, side = side_name },
                "Claimed SOS1 {side_name} link constraint {id:?} does not have a finite positive Big-M after normalization"
            );
        }

        let normalized_tolerance = atol.into_inner() / scale;
        if !normalized_tolerance.is_finite() || normalized_tolerance <= 0.0 {
            crate::bail!(
                {
                    ?id,
                    ?member,
                    scale,
                    atol = atol.into_inner(),
                    normalized_tolerance,
                    side = side_name
                },
                "Claimed SOS1 {side_name} link constraint {id:?} does not have a finite positive ATol after normalization"
            );
        }
        let normalized_atol = ATol::new(normalized_tolerance)
            .expect("finite positive normalized tolerance was checked above");
        let domain = side.feasible_signed_domain(variable.kind(), variable.bound(), atol)?;

        let normalized_is_satisfied = |signed_value: f64, selector_value: f64| {
            let residual = signed_value - big_m * selector_value;
            residual.is_finite()
                && Equality::LessThanOrEqualToZero.is_satisfied(residual, normalized_atol)
        };
        let raw_is_satisfied = |signed_value: f64, selector_value: f64| -> crate::Result<bool> {
            let member_value = side.member_value(signed_value);
            let residual =
                member_coefficient * member_value + selector_coefficient * selector_value;
            if !residual.is_finite() {
                crate::bail!(
                    {
                        ?id,
                        ?member,
                        ?selector,
                        member_value,
                        selector_value,
                        residual,
                        side = side_name
                    },
                    "Claimed SOS1 {side_name} link constraint {id:?} has a non-finite residual on the ATol-feasible member domain"
                );
            }
            Ok(Equality::LessThanOrEqualToZero.is_satisfied(residual, atol))
        };

        // The reconstructed selector is zero throughout |member| <= ATol.
        // Every such point must remain feasible in the original raw row.
        if let Some(max_zero) = domain.max_zero_classified(atol) {
            let normalized_feasible = normalized_is_satisfied(max_zero, 0.0);
            let raw_feasible = raw_is_satisfied(max_zero, 0.0)?;
            if !normalized_feasible || !raw_feasible {
                crate::bail!(
                    {
                        ?id,
                        ?member,
                        scale,
                        max_zero,
                        atol = atol.into_inner(),
                        normalized_tolerance,
                        side = side_name
                    },
                    "Claimed SOS1 {side_name} link constraint {id:?} does not preserve the SOS1 zero classification under the supplied ATol"
                );
            }
        }

        // Conversely, a member active on this side must make z = 0
        // infeasible, so every feasible original selector assignment uses z =
        // 1. For a continuous interval meeting the open boundary x > ATol,
        // this requires scale >= 1; an attained minimum can be checked
        // directly in the raw row.
        match domain.min_active(atol) {
            None => {}
            Some(ActiveMinimum::OpenAtTolerance) if scale < 1.0 => {
                crate::bail!(
                    {
                        ?id,
                        ?member,
                        scale,
                        atol = atol.into_inner(),
                        normalized_tolerance,
                        side = side_name
                    },
                    "Claimed SOS1 {side_name} link constraint {id:?} does not force an active member to use selector value one under the supplied ATol"
                );
            }
            Some(ActiveMinimum::OpenAtTolerance) => {}
            Some(ActiveMinimum::Attained(min_active)) => {
                let normalized_feasible = normalized_is_satisfied(min_active, 0.0);
                let raw_feasible = raw_is_satisfied(min_active, 0.0)?;
                if normalized_feasible || raw_feasible {
                    crate::bail!(
                        {
                            ?id,
                            ?member,
                            scale,
                            min_active,
                            atol = atol.into_inner(),
                            normalized_tolerance,
                            side = side_name
                        },
                        "Claimed SOS1 {side_name} link constraint {id:?} does not force an active member to use selector value one under the supplied ATol"
                    );
                }
            }
        }

        // An active value on the opposite side satisfies this row
        // mathematically, but the original f64 residual must also remain
        // finite. Otherwise removing the row would turn an evaluation error
        // into an active-model feasible point.
        if domain.lower < -atol.into_inner()
            && (!normalized_is_satisfied(domain.lower, 1.0)
                || !raw_is_satisfied(domain.lower, 1.0)?)
        {
            crate::bail!(
                {
                    ?id,
                    ?member,
                    scale,
                    big_m,
                    domain_lower = domain.lower,
                    atol = atol.into_inner(),
                    normalized_tolerance,
                    side = side_name
                },
                "Claimed SOS1 {side_name} link constraint {id:?} is not feasible over the complete ATol-feasible member domain at selector value one"
            );
        }

        // At z = 1 the row is monotone in the signed member value, so the
        // largest same-side active value proves Big-M coverage. This includes
        // the bound tolerance for Continuous members.
        if domain.upper > atol.into_inner()
            && (!normalized_is_satisfied(domain.upper, 1.0)
                || !raw_is_satisfied(domain.upper, 1.0)?)
        {
            crate::bail!(
                {
                    ?id,
                    ?member,
                    scale,
                    big_m,
                    domain_upper = domain.upper,
                    atol = atol.into_inner(),
                    normalized_tolerance,
                    side = side_name
                },
                "Claimed SOS1 {side_name} link constraint {id:?} has Big-M {big_m}, which does not cover the ATol-feasible member domain"
            );
        }
        Ok(())
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
    /// the formulation rows claimed by the request and do not occur in the
    /// output objective.
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
        let output_objective_ids = self
            .output_objective()
            .map(|output| {
                populated_required_ids(output.function(), &self.decision_variable_dependency)
            })
            .unwrap_or_default();
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

            if output_objective_ids.contains(&id) {
                crate::bail!(
                    { ?id },
                    "Fresh SOS1 selector {id:?} is used by the output objective"
                );
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

    fn two_term_link(member_coefficient: f64, selector_coefficient: f64) -> Constraint {
        let function = (crate::Linear::single_term(
            LinearMonomial::Variable(member_integer_id()),
            crate::Coefficient::try_from(member_coefficient).unwrap(),
        ) + crate::Linear::single_term(
            LinearMonomial::Variable(selector_id()),
            crate::Coefficient::try_from(selector_coefficient).unwrap(),
        ))
        .unwrap();
        Constraint::less_than_or_equal_to_zero(Function::from(function))
    }

    fn upper_link(scale: f64, big_m: f64) -> Constraint {
        two_term_link(scale, -scale * big_m)
    }

    fn lower_link(scale: f64, big_m: f64) -> Constraint {
        two_term_link(-scale, -scale * big_m)
    }

    fn mixed_instance() -> (Instance, Sos1BigMPromotionRequest) {
        let constraints = BTreeMap::from([
            (upper_row_id(), upper_link(1.0, 3.0)),
            (lower_row_id(), lower_link(1.0, 2.0)),
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
        let request = Sos1BigMPromotionRequest::new(
            BTreeMap::from([
                (member_binary_id(), Sos1BigMSelectorClaim::Reused),
                (
                    member_integer_id(),
                    Sos1BigMSelectorClaim::Fresh {
                        selector: selector_id(),
                        upper_link: Some(upper_row_id()),
                        lower_link: Some(lower_row_id()),
                    },
                ),
            ]),
            cardinality_row_id(),
        );
        (instance, request)
    }

    fn fresh_instance(
        member: DecisionVariable,
        upper_link_id: Option<ConstraintID>,
        lower_link_id: Option<ConstraintID>,
    ) -> (Instance, Sos1BigMPromotionRequest) {
        let bound = member.bound();
        let mut constraints = BTreeMap::new();
        if let Some(id) = upper_link_id {
            constraints.insert(id, upper_link(1.0, bound.upper()));
        }
        if let Some(id) = lower_link_id {
            constraints.insert(id, lower_link(1.0, -bound.lower()));
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
        let request = Sos1BigMPromotionRequest::new(
            BTreeMap::from([(
                member_integer_id(),
                Sos1BigMSelectorClaim::Fresh {
                    selector: selector_id(),
                    upper_link: upper_link_id,
                    lower_link: lower_link_id,
                },
            )]),
            cardinality_row_id(),
        );
        (instance, request)
    }

    fn assert_atomic_rejection(
        instance: Instance,
        request: &Sos1BigMPromotionRequest,
        expected: &str,
    ) {
        assert_atomic_rejection_with_atol(instance, request, ATol::default(), expected);
    }

    fn assert_atomic_rejection_with_atol(
        mut instance: Instance,
        request: &Sos1BigMPromotionRequest,
        atol: ATol,
        expected: &str,
    ) {
        let before = instance.clone();
        let error = instance.promote_sos1_big_m(request, atol).unwrap_err();
        assert!(
            error.to_string().contains(expected),
            "expected {expected:?} in error, got: {error:#}"
        );
        assert_eq!(instance, before);
    }

    #[test]
    fn promotes_mixed_formulation_and_retains_history() {
        let (mut instance, request) = mixed_instance();
        let promotion = instance
            .promote_sos1_big_m(&request, ATol::default())
            .unwrap();

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
            .all(|(_, reason)| reason.reason == "promoted validated SOS1 Big-M formulation"));
        assert_eq!(
            &instance.sos1_constraints()[&Sos1ConstraintID::from(0)].variables,
            promotion.members()
        );
        assert!(instance.decision_variables().contains_key(&unrelated_id()));

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
    fn promotion_preserves_the_claimed_formulations_projected_feasibility() {
        let atol = ATol::default();
        let (original, request) = mixed_instance();
        let mut promoted = original.clone();
        let _ = promoted.promote_sos1_big_m(&request, atol).unwrap();

        for reused_member in [0.0, 1.0] {
            for fresh_member in -2..=3 {
                let fresh_member = f64::from(fresh_member);
                let original_projected_feasible = [0.0, 1.0].into_iter().any(|selector| {
                    original
                        .evaluate(
                            &crate::v1::State::from_iter([
                                (0, reused_member),
                                (1, fresh_member),
                                (10, selector),
                            ]),
                            atol,
                        )
                        .unwrap()
                        .feasible()
                });
                let promoted_solution = promoted
                    .evaluate(
                        &crate::v1::State::from_iter([(0, reused_member), (1, fresh_member)]),
                        atol,
                    )
                    .unwrap();

                assert_eq!(
                    promoted_solution.feasible_relaxed(),
                    original_projected_feasible,
                    "projected feasibility differs at reused={reused_member}, fresh={fresh_member}"
                );
                assert_eq!(
                    promoted_solution.feasible(),
                    promoted_solution.feasible_relaxed(),
                    "canonical selector does not satisfy the claimed removed rows at reused={reused_member}, fresh={fresh_member}"
                );
            }
        }
    }

    #[test]
    fn reconstructed_selector_uses_shared_atol_semantics() {
        let member = DecisionVariable::new(
            Kind::Continuous,
            Bound::new(-2.0, 3.0).unwrap(),
            ATol::default(),
        )
        .unwrap();
        let (mut instance, request) =
            fresh_instance(member, Some(upper_row_id()), Some(lower_row_id()));
        let atol = ATol::new(1.0e-6).unwrap();
        instance
            .constraint_collection
            .replace_active_row(upper_row_id(), upper_link(1.0, 3.0 + atol.into_inner()))
            .unwrap();
        instance
            .constraint_collection
            .replace_active_row(lower_row_id(), lower_link(1.0, 2.0 + atol.into_inner()))
            .unwrap();
        let promotion = instance.promote_sos1_big_m(&request, atol).unwrap();

        let near_zero = instance
            .evaluate(&crate::v1::State::from_iter([(1, 5.0e-7)]), atol)
            .unwrap();
        assert_eq!(near_zero.state().entries[&selector_id().into_inner()], 0.0);
        let evaluated = near_zero
            .evaluated_sos1_constraints()
            .get(&promotion.sos1_constraint_id())
            .unwrap();
        assert!(evaluated.stage.feasible);
        assert_eq!(evaluated.stage.active_variable, None);

        let on_boundary = instance
            .evaluate(&crate::v1::State::from_iter([(1, 1.0e-6)]), atol)
            .unwrap();
        assert_eq!(
            on_boundary.state().entries[&selector_id().into_inner()],
            0.0
        );
        let evaluated = on_boundary
            .evaluated_sos1_constraints()
            .get(&promotion.sos1_constraint_id())
            .unwrap();
        assert!(evaluated.stage.feasible);
        assert_eq!(evaluated.stage.active_variable, None);
        assert!(
            on_boundary
                .evaluated_constraints()
                .get(&upper_row_id())
                .unwrap()
                .stage
                .feasible
        );
        assert!(on_boundary.feasible());
        assert!(on_boundary.feasible_relaxed());

        let sample_id = crate::SampleID::from(0);
        let sample_set = instance
            .evaluate_samples(
                &crate::Sampled::from(crate::v1::State::from_iter([(1, 1.0e-6)])),
                atol,
            )
            .unwrap();
        assert_eq!(
            sample_set.decision_variables()[&selector_id()]
                .samples()
                .get(sample_id),
            Some(&0.0)
        );
        let sampled = &sample_set.sos1_constraints()[&promotion.sos1_constraint_id()];
        assert!(sampled.stage.feasible[&sample_id]);
        assert_eq!(sampled.stage.active_variable[&sample_id], None);
        assert_eq!(sample_set.is_sample_feasible(sample_id), Some(true));
        assert_eq!(sample_set.is_sample_feasible_relaxed(sample_id), Some(true));

        let negative_boundary = instance
            .evaluate(&crate::v1::State::from_iter([(1, -1.0e-6)]), atol)
            .unwrap();
        assert_eq!(
            negative_boundary.state().entries[&selector_id().into_inner()],
            0.0
        );
        assert_eq!(
            negative_boundary
                .evaluated_sos1_constraints()
                .get(&promotion.sos1_constraint_id())
                .unwrap()
                .stage
                .active_variable,
            None
        );
        assert!(
            negative_boundary
                .evaluated_constraints()
                .get(&lower_row_id())
                .unwrap()
                .stage
                .feasible
        );
        assert!(negative_boundary.feasible());
        assert!(negative_boundary.feasible_relaxed());
    }

    #[test]
    fn accepts_finite_member_bounds_that_exclude_zero() {
        let (mut positive, request) = fresh_instance(integer(1.0, 3.0), Some(upper_row_id()), None);
        let _promotion = positive
            .promote_sos1_big_m(&request, ATol::default())
            .unwrap();
        assert_eq!(positive.sos1_constraints().len(), 1);

        let (mut negative, request) =
            fresh_instance(integer(-3.0, -1.0), None, Some(lower_row_id()));
        let _promotion = negative
            .promote_sos1_big_m(&request, ATol::default())
            .unwrap();
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
        let request = Sos1BigMPromotionRequest::new(
            BTreeMap::from([
                (VariableID::from(3), Sos1BigMSelectorClaim::Reused),
                (VariableID::from(4), Sos1BigMSelectorClaim::Reused),
            ]),
            cardinality,
        );
        let promotion = instance
            .promote_sos1_big_m(&request, ATol::default())
            .unwrap();
        assert!(promotion.fresh_selectors().is_empty());
        assert!(instance.decision_variable_dependency().is_empty());
        assert_eq!(instance.decision_variables().len(), 2);
        assert!(instance.constraints().is_empty());
        assert!(instance.removed_constraints().contains_key(&cardinality));
    }

    #[test]
    fn accepts_loose_links_and_preserves_original_rows() {
        let (mut instance, request) = mixed_instance();
        let upper = upper_link(1.0, 10.0);
        let lower = lower_link(1.0, 8.0);
        instance
            .constraint_collection
            .replace_active_row(upper_row_id(), upper.clone())
            .unwrap();
        instance
            .constraint_collection
            .replace_active_row(lower_row_id(), lower.clone())
            .unwrap();

        let promotion = instance
            .promote_sos1_big_m(&request, ATol::new(1.0e-6).unwrap())
            .unwrap();

        assert_eq!(promotion.relaxed_constraint_ids().len(), 3);
        assert_eq!(instance.removed_constraints()[&upper_row_id()].0, upper);
        assert_eq!(instance.removed_constraints()[&lower_row_id()].0, lower);
    }

    #[test]
    fn accepts_nonunit_scales_when_integer_domain_preserves_atol_semantics() {
        let atol = ATol::new(0.125).unwrap();
        let (mut instance, request) = mixed_instance();
        instance
            .constraint_collection
            .replace_active_row(upper_row_id(), upper_link(0.5, 2.75))
            .unwrap();
        instance
            .constraint_collection
            .replace_active_row(lower_row_id(), lower_link(4.0, 1.96875))
            .unwrap();

        let _ = instance.promote_sos1_big_m(&request, atol).unwrap();

        for member_value in [3.0, -2.0] {
            let solution = instance
                .evaluate(
                    &crate::v1::State::from_iter([(0, 0.0), (1, member_value)]),
                    atol,
                )
                .unwrap();
            assert!(solution.feasible());
            assert!(solution.feasible_relaxed());
        }

        let (mut upper_outside, request) = mixed_instance();
        upper_outside
            .constraint_collection
            .replace_active_row(upper_row_id(), upper_link(2.0, 2.875))
            .unwrap();
        assert_atomic_rejection_with_atol(
            upper_outside,
            &request,
            atol,
            "does not cover the ATol-feasible member domain",
        );

        let (mut lower_outside, request) = mixed_instance();
        lower_outside
            .constraint_collection
            .replace_active_row(lower_row_id(), lower_link(4.0, 1.875))
            .unwrap();
        assert_atomic_rejection_with_atol(
            lower_outside,
            &request,
            atol,
            "does not cover the ATol-feasible member domain",
        );

        let (mut threshold_too_weak, request) = mixed_instance();
        threshold_too_weak
            .constraint_collection
            .replace_active_row(upper_row_id(), upper_link(0.125, 10.0))
            .unwrap();
        assert_atomic_rejection_with_atol(
            threshold_too_weak,
            &request,
            atol,
            "does not force an active member",
        );
    }

    #[test]
    fn rejects_nonunit_scales_across_a_continuous_zero_boundary() {
        let atol = ATol::new(0.125).unwrap();
        let member = DecisionVariable::new(
            Kind::Continuous,
            Bound::new(-2.0, 3.0).unwrap(),
            ATol::default(),
        )
        .unwrap();

        let (mut upper_too_large, request) =
            fresh_instance(member.clone(), Some(upper_row_id()), Some(lower_row_id()));
        upper_too_large
            .constraint_collection
            .replace_active_row(upper_row_id(), upper_link(2.0, 10.0))
            .unwrap();
        assert_atomic_rejection_with_atol(
            upper_too_large,
            &request,
            atol,
            "does not preserve the SOS1 zero classification",
        );

        let (mut upper_too_small, request) =
            fresh_instance(member.clone(), Some(upper_row_id()), Some(lower_row_id()));
        upper_too_small
            .constraint_collection
            .replace_active_row(upper_row_id(), upper_link(0.5, 10.0))
            .unwrap();
        assert_atomic_rejection_with_atol(
            upper_too_small,
            &request,
            atol,
            "does not force an active member",
        );

        let (mut lower_too_large, request) =
            fresh_instance(member.clone(), Some(upper_row_id()), Some(lower_row_id()));
        lower_too_large
            .constraint_collection
            .replace_active_row(lower_row_id(), lower_link(2.0, 10.0))
            .unwrap();
        assert_atomic_rejection_with_atol(
            lower_too_large,
            &request,
            atol,
            "does not preserve the SOS1 zero classification",
        );

        let (mut lower_too_small, request) =
            fresh_instance(member, Some(upper_row_id()), Some(lower_row_id()));
        lower_too_small
            .constraint_collection
            .replace_active_row(lower_row_id(), lower_link(0.5, 10.0))
            .unwrap();
        assert_atomic_rejection_with_atol(
            lower_too_small,
            &request,
            atol,
            "does not force an active member",
        );
    }

    #[test]
    fn continuous_link_coverage_uses_the_atol_feasible_domain() {
        let atol = ATol::new(0.125).unwrap();

        let positive = DecisionVariable::new(
            Kind::Continuous,
            Bound::new(1.0, 3.0).unwrap(),
            ATol::default(),
        )
        .unwrap();
        let (mut upper_on_boundary, request) =
            fresh_instance(positive.clone(), Some(upper_row_id()), None);
        upper_on_boundary
            .constraint_collection
            .replace_active_row(upper_row_id(), upper_link(2.0, 3.0625))
            .unwrap();
        let original_upper = upper_on_boundary.clone();
        let _ = upper_on_boundary
            .promote_sos1_big_m(&request, atol)
            .unwrap();
        let before = original_upper
            .evaluate(&crate::v1::State::from_iter([(1, 3.125), (10, 1.0)]), atol)
            .unwrap();
        let after = upper_on_boundary
            .evaluate(&crate::v1::State::from_iter([(1, 3.125)]), atol)
            .unwrap();
        assert!(before.feasible());
        assert!(after.feasible());
        assert!(after.feasible_relaxed());

        let (mut upper_outside, request) = fresh_instance(positive, Some(upper_row_id()), None);
        upper_outside
            .constraint_collection
            .replace_active_row(upper_row_id(), upper_link(2.0, 3.0))
            .unwrap();
        assert_atomic_rejection_with_atol(
            upper_outside,
            &request,
            atol,
            "does not cover the ATol-feasible member domain",
        );

        let negative = DecisionVariable::new(
            Kind::Continuous,
            Bound::new(-3.0, -1.0).unwrap(),
            ATol::default(),
        )
        .unwrap();
        let (mut lower_on_boundary, request) =
            fresh_instance(negative.clone(), None, Some(lower_row_id()));
        lower_on_boundary
            .constraint_collection
            .replace_active_row(lower_row_id(), lower_link(0.5, 2.875))
            .unwrap();
        let original_lower = lower_on_boundary.clone();
        let _ = lower_on_boundary
            .promote_sos1_big_m(&request, atol)
            .unwrap();
        let before = original_lower
            .evaluate(&crate::v1::State::from_iter([(1, -3.125), (10, 1.0)]), atol)
            .unwrap();
        let after = lower_on_boundary
            .evaluate(&crate::v1::State::from_iter([(1, -3.125)]), atol)
            .unwrap();
        assert!(before.feasible());
        assert!(after.feasible());
        assert!(after.feasible_relaxed());

        let (mut lower_outside, request) = fresh_instance(negative, None, Some(lower_row_id()));
        lower_outside
            .constraint_collection
            .replace_active_row(lower_row_id(), lower_link(0.5, 2.75))
            .unwrap();
        assert_atomic_rejection_with_atol(
            lower_outside,
            &request,
            atol,
            "does not cover the ATol-feasible member domain",
        );
    }

    #[test]
    fn accepts_valid_redundant_links_on_domain_implied_sides() {
        let (mut positive, request) = fresh_instance(
            integer(1.0, 3.0),
            Some(upper_row_id()),
            Some(lower_row_id()),
        );
        positive
            .constraint_collection
            .replace_active_row(lower_row_id(), lower_link(2.0, 1.0))
            .unwrap();
        let _ = positive
            .promote_sos1_big_m(&request, ATol::default())
            .unwrap();

        let (mut negative, request) = fresh_instance(
            integer(-3.0, -1.0),
            Some(upper_row_id()),
            Some(lower_row_id()),
        );
        negative
            .constraint_collection
            .replace_active_row(upper_row_id(), upper_link(2.0, 1.0))
            .unwrap();
        let _ = negative
            .promote_sos1_big_m(&request, ATol::default())
            .unwrap();
    }

    #[test]
    fn rejects_invalid_redundant_link_without_mutation() {
        let (mut instance, request) = fresh_instance(
            integer(1.0, 3.0),
            Some(upper_row_id()),
            Some(lower_row_id()),
        );
        instance
            .constraint_collection
            .replace_active_row(lower_row_id(), upper_link(1.0, 1.0))
            .unwrap();

        assert_atomic_rejection(instance, &request, "member-coefficient sign");
    }

    #[test]
    fn rejects_missing_required_link_without_mutation() {
        let (instance, request) = fresh_instance(integer(-2.0, 3.0), None, Some(lower_row_id()));
        assert_atomic_rejection(instance, &request, "missing its required upper link");

        let (instance, request) = fresh_instance(integer(-2.0, 3.0), Some(upper_row_id()), None);
        assert_atomic_rejection(instance, &request, "missing its required lower link");

        let small_positive = DecisionVariable::new(
            Kind::Continuous,
            Bound::new(0.0, 0.0625).unwrap(),
            ATol::default(),
        )
        .unwrap();
        let (instance, request) = fresh_instance(small_positive, None, None);
        assert_atomic_rejection_with_atol(
            instance,
            &request,
            ATol::new(0.125).unwrap(),
            "missing its required upper link",
        );
    }

    #[test]
    fn rejects_nonlinear_payload_with_canonical_looking_linear_terms() {
        let (mut instance, request) = mixed_instance();
        let linear_terms = (quadratic!(1) + (coeff!(-3.0) * quadratic!(10)).unwrap()).unwrap();
        let nonlinear = Function::from((linear_terms + quadratic!(1, 10)).unwrap());
        instance
            .constraint_collection
            .replace_active_row(
                upper_row_id(),
                Constraint::less_than_or_equal_to_zero(nonlinear),
            )
            .unwrap();

        assert_atomic_rejection(instance, &request, "is not linear");
    }

    #[test]
    fn rejects_nonsemantic_link_shapes_without_mutation() {
        let (mut wrong_member_sign, request) = mixed_instance();
        wrong_member_sign
            .constraint_collection
            .replace_active_row(upper_row_id(), lower_link(1.0, 3.0))
            .unwrap();
        assert_atomic_rejection(wrong_member_sign, &request, "member-coefficient sign");

        let (mut wrong_selector_sign, request) = mixed_instance();
        wrong_selector_sign
            .constraint_collection
            .replace_active_row(upper_row_id(), two_term_link(1.0, 3.0))
            .unwrap();
        assert_atomic_rejection(
            wrong_selector_sign,
            &request,
            "negative selector coefficient",
        );

        let (mut shifted, request) = mixed_instance();
        let shifted_function = (upper_link(1.0, 3.0)
            .function()
            .as_linear()
            .unwrap()
            .into_owned()
            + crate::Linear::from(coeff!(0.25)))
        .unwrap();
        shifted
            .constraint_collection
            .replace_active_row(
                upper_row_id(),
                Constraint::less_than_or_equal_to_zero(Function::from(shifted_function)),
            )
            .unwrap();
        assert_atomic_rejection(shifted, &request, "with no constant");

        let (mut extra_variable, request) = mixed_instance();
        let extra_function = (upper_link(1.0, 3.0)
            .function()
            .as_linear()
            .unwrap()
            .into_owned()
            + crate::Linear::single_term(LinearMonomial::Variable(unrelated_id()), coeff!(0.25)))
        .unwrap();
        extra_variable
            .constraint_collection
            .replace_active_row(
                upper_row_id(),
                Constraint::less_than_or_equal_to_zero(Function::from(extra_function)),
            )
            .unwrap();
        assert_atomic_rejection(extra_variable, &request, "with no constant");

        let (mut equality, request) = mixed_instance();
        equality
            .constraint_collection
            .replace_active_row(
                upper_row_id(),
                Constraint::equal_to_zero(upper_link(1.0, 3.0).function().clone()),
            )
            .unwrap();
        assert_atomic_rejection(equality, &request, "less-than-or-equal-to-zero");
    }

    #[test]
    fn rejects_nonfinite_validation_inputs_without_mutation() {
        let (instance, request) = mixed_instance();
        assert_atomic_rejection_with_atol(
            instance,
            &request,
            ATol::new(f64::INFINITY).unwrap(),
            "requires a finite ATol",
        );

        let (mut overflowed_ratio, request) = mixed_instance();
        overflowed_ratio
            .constraint_collection
            .replace_active_row(upper_row_id(), two_term_link(f64::MIN_POSITIVE, -f64::MAX))
            .unwrap();
        assert_atomic_rejection(
            overflowed_ratio,
            &request,
            "finite positive Big-M after normalization",
        );

        let extreme_negative = DecisionVariable::new(
            Kind::Continuous,
            Bound::new(-1.0e308, -1.0).unwrap(),
            ATol::default(),
        )
        .unwrap();
        let (mut raw_residual_overflow, request) =
            fresh_instance(extreme_negative, Some(upper_row_id()), Some(lower_row_id()));
        raw_residual_overflow
            .constraint_collection
            .replace_active_row(upper_row_id(), upper_link(2.0, 10.0))
            .unwrap();
        assert_atomic_rejection(
            raw_residual_overflow,
            &request,
            "non-finite residual on the ATol-feasible member domain",
        );
    }

    #[test]
    fn rejects_atol_that_weakens_binary_cardinality() {
        let (instance, request) = mixed_instance();
        assert_atomic_rejection_with_atol(
            instance,
            &request,
            ATol::new(1.0).unwrap(),
            "requires ATol < 1",
        );
    }

    #[test]
    fn link_ids_must_name_active_regular_constraints() {
        let (instance, mut request) = mixed_instance();
        request.selector_claims.insert(
            member_integer_id(),
            Sos1BigMSelectorClaim::Fresh {
                selector: selector_id(),
                upper_link: Some(ConstraintID::from(999)),
                lower_link: Some(lower_row_id()),
            },
        );
        assert_atomic_rejection(instance, &request, "is not active");

        let (mut removed, request) = mixed_instance();
        removed
            .relax_constraint(upper_row_id(), "test".to_string(), [])
            .unwrap();
        assert_atomic_rejection(removed, &request, "is not active");
    }

    #[test]
    fn cardinality_row_remains_exact() {
        let (mut instance, request) = mixed_instance();
        let changed_one = f64::from_bits(1.0f64.to_bits() + 1);
        let cardinality = ((crate::Linear::single_term(
            LinearMonomial::Variable(member_binary_id()),
            coeff!(1.0),
        ) + crate::Linear::single_term(
            LinearMonomial::Variable(selector_id()),
            crate::Coefficient::try_from(changed_one).unwrap(),
        ))
        .unwrap()
            + crate::Linear::from(coeff!(-1.0)))
        .unwrap();
        instance
            .constraint_collection
            .replace_active_row(
                cardinality_row_id(),
                Constraint::less_than_or_equal_to_zero(Function::from(cardinality)),
            )
            .unwrap();

        assert_atomic_rejection(
            instance,
            &request,
            "does not match the canonical row exactly",
        );
    }

    #[test]
    fn rejects_unmodeled_members_without_mutation() {
        let (instance, request) = fresh_instance(DecisionVariable::continuous(), None, None);
        assert_atomic_rejection(instance, &request, "does not have finite bounds");

        let fixed_binary =
            DecisionVariable::new(Kind::Binary, Bound::new(1.0, 1.0).unwrap(), ATol::default())
                .unwrap();
        let (instance, request) = fresh_instance(fixed_binary, Some(upper_row_id()), None);
        assert_atomic_rejection(instance, &request, "Binary SOS1 member");

        let semi = DecisionVariable::new(
            Kind::SemiContinuous,
            Bound::new(-2.0, 3.0).unwrap(),
            ATol::default(),
        )
        .unwrap();
        let (instance, request) = fresh_instance(semi, Some(upper_row_id()), Some(lower_row_id()));
        assert_atomic_rejection(instance, &request, "semi-variable");

        let (mut instance, request) = mixed_instance();
        instance
            .decision_variables
            .set_fixed_value(member_integer_id(), 0.0, ATol::default())
            .unwrap();
        assert_atomic_rejection(instance, &request, "is fixed");

        let (mut instance, request) = mixed_instance();
        instance.decision_variable_dependency =
            AcyclicAssignments::new([(member_integer_id(), Function::Zero)]).unwrap();
        assert_atomic_rejection(instance, &request, "dependency target");
    }

    #[test]
    fn rejects_structurally_invalid_requests_without_mutation() {
        let mut empty = Instance::default();
        let empty_request = Sos1BigMPromotionRequest::new(BTreeMap::new(), ConstraintID::from(0));
        let before = empty.clone();
        assert!(empty
            .promote_sos1_big_m(&empty_request, ATol::default())
            .unwrap_err()
            .to_string()
            .contains("at least one member"));
        assert_eq!(empty, before);

        let (instance, mut collision_request) = mixed_instance();
        collision_request.selector_claims.insert(
            member_integer_id(),
            Sos1BigMSelectorClaim::Fresh {
                selector: member_binary_id(),
                upper_link: Some(upper_row_id()),
                lower_link: Some(lower_row_id()),
            },
        );
        assert_atomic_rejection(instance, &collision_request, "collides");

        let (instance, mut duplicate_row_request) = mixed_instance();
        duplicate_row_request.cardinality_constraint = upper_row_id();
        assert_atomic_rejection(instance, &duplicate_row_request, "also claimed");
    }

    #[test]
    fn preserves_selector_labels_and_relaxed_row_context() {
        let (mut instance, request) = mixed_instance();
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
        let _ = instance
            .promote_sos1_big_m(&request, ATol::default())
            .unwrap();

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
        let (base, request) = mixed_instance();

        let mut instance = base.clone();
        instance.set_objective(Function::from(linear!(10))).unwrap();
        assert_atomic_rejection(instance, &request, "the objective");

        let mut instance = base.clone();
        instance.output_objective = Some(super::super::OutputObjective::new(
            Sense::Minimize,
            Function::from(linear!(10)),
            false,
        ));
        assert_atomic_rejection(instance, &request, "the output objective");

        let mut instance = base.clone();
        instance.decision_variable_dependency =
            AcyclicAssignments::new([(unrelated_id(), Function::from(linear!(10)))]).unwrap();
        instance.output_objective = Some(super::super::OutputObjective::new(
            Sense::Minimize,
            Function::from(linear!(20)),
            false,
        ));
        assert_atomic_rejection(instance, &request, "the output objective");

        let mut instance = base.clone();
        instance
            .add_constraint(
                Constraint::less_than_or_equal_to_zero(Function::from(linear!(10))),
                Default::default(),
            )
            .unwrap();
        assert_atomic_rejection(instance, &request, "retained active regular");

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
        assert_atomic_rejection(instance, &request, "active Indicator");

        let mut instance = base.clone();
        instance
            .add_one_hot_constraint(
                OneHotConstraint::new(BTreeSet::from([selector_id()])).unwrap(),
                Default::default(),
            )
            .unwrap();
        assert_atomic_rejection(instance, &request, "active OneHot");

        let mut instance = base.clone();
        instance
            .add_sos1_constraint(
                Sos1Constraint::new(BTreeSet::from([selector_id()])).unwrap(),
                Default::default(),
            )
            .unwrap();
        assert_atomic_rejection(instance, &request, "active SOS1");

        let mut instance = base.clone();
        instance.decision_variable_dependency =
            AcyclicAssignments::new([(selector_id(), Function::Zero)]).unwrap();
        assert_atomic_rejection(instance, &request, "dependency target");

        let mut instance = base;
        instance
            .decision_variables
            .set_fixed_value(selector_id(), 0.0, ATol::default())
            .unwrap();
        assert_atomic_rejection(instance, &request, "is fixed");
    }

    #[test]
    fn removed_named_and_dependency_rhs_references_are_preserved() {
        let (mut instance, request) = mixed_instance();

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
            AcyclicAssignments::new([(unrelated_id(), Function::from(linear!(10)))]).unwrap();

        let _ = instance
            .promote_sos1_big_m(&request, ATol::default())
            .unwrap();

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
                &crate::v1::State::from_iter([(0, 0.0), (1, -2.0)]),
                ATol::default(),
            )
            .unwrap();
        assert_eq!(solution.state().entries[&selector_id().into_inner()], 1.0);
        assert_eq!(solution.state().entries[&unrelated_id().into_inner()], 1.0);
    }

    #[test]
    fn dependent_selector_reconstruction_validates_input_state() {
        let (mut instance, request) = mixed_instance();
        let _ = instance
            .promote_sos1_big_m(&request, ATol::default())
            .unwrap();

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
