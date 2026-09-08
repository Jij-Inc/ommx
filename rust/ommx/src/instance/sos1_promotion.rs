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
//! Each request is first checked without allocating a target SOS1 ID. The public
//! batch plan then rejects incompatible candidates, reserves distinct target
//! IDs, and records the invariants that make every combined commit effect valid
//! for its source instance. In particular, consumed regular rows and
//! fresh-selector assignment targets are disjoint, no fresh selector becomes a
//! member of another promoted SOS1, and the combined assignments cannot
//! introduce a dependency cycle. The plan retains an exclusive borrow of the
//! source instance: callers may inspect all rejections and either apply the
//! successful effects or drop the plan without mutation. Apply starts with one
//! batch lifecycle move that validates every row ID before changing state.
//! Failure while applying the remaining effects is therefore an internal
//! plan-invariant violation, not request rejection. No path clones the instance
//! or mutates first and rolls back later.

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
/// [`Instance::plan_promote_sos1_big_m`] checks the role against the current
/// member domain and the meaning of the claimed regular rows before changing
/// the instance.
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

/// Untrusted stable-ID request for a batch of SOS1 Big-M promotions.
///
/// Each outer key identifies the regular cardinality constraint of one
/// formulation. Its value maps intended SOS1 member IDs to their claimed
/// selector roles. Duplicate cardinality claims are unrepresentable; overlap
/// between other claimed rows is checked by the instance-bound batch plan.
///
/// Bounds and coefficients are intentionally absent: validation always reads
/// them from the current [`Instance`]. The request is runtime-only and has no
/// serialization contract. All entries are untrusted claims; no
/// instance-dependent validation occurs until
/// [`Instance::plan_promote_sos1_big_m`] is called.
pub type Sos1BigMPromotionRequest =
    BTreeMap<ConstraintID, BTreeMap<VariableID, Sos1BigMSelectorClaim>>;

/// Result of a batch of checked SOS1 Big-M promotions.
///
/// The map has exactly the same cardinality-constraint keys as the request.
/// Each value is either the allocated SOS1 ID or that formulation's rejection
/// reason. Strict application returns this same report with only successful
/// entries, or returns a batch-rejection signal without mutation.
/// Membership, retained formulation history, and selector reconstruction are
/// owned by the mutated [`Instance`] and can be queried there.
pub type Sos1BigMPromotion = BTreeMap<ConstraintID, crate::Result<Sos1ConstraintID>>;

/// Signal that a fully-valid SOS1 Big-M promotion batch could not be formed.
///
/// This signal is produced by
/// [`Sos1BigMPromotionPlan::apply_if_fully_valid`] before the bound
/// [`Instance`] is mutated. It owns every planning rejection together with the
/// cardinality constraint ID of the corresponding formulation. Callers can inspect
/// those IDs, repair or remove the rejected claims, and retry against
/// the unchanged instance.
///
/// # Invariants
///
/// - `request_count` is the number of formulations in the batch request;
/// - `rejections` is non-empty and its keys are a subset of the request keys;
/// - `rejections.len() <= request_count`; and
/// - no plan effect was applied to the bound instance.
#[non_exhaustive]
#[derive(Debug)]
pub struct Sos1BigMPromotionBatchRejected {
    request_count: usize,
    rejections: BTreeMap<ConstraintID, crate::Error>,
}

impl Sos1BigMPromotionBatchRejected {
    /// Number of requests in the rejected batch.
    pub fn request_count(&self) -> usize {
        self.request_count
    }

    /// Iterates over all planning rejections in cardinality-constraint ID order.
    ///
    /// Each item contains the cardinality constraint ID and the complete Rust
    /// error chain for that formulation.
    pub fn rejections(&self) -> impl Iterator<Item = (ConstraintID, &crate::Error)> + '_ {
        self.rejections.iter().map(|(&id, error)| (id, error))
    }
}

impl std::fmt::Display for Sos1BigMPromotionBatchRejected {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "SOS1 Big-M promotion rejected {} of {} requests",
            self.rejections.len(),
            self.request_count
        )?;
        for (id, error) in &self.rejections {
            write!(formatter, "\ncardinality constraint {id}: {error:#}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Sos1BigMPromotionBatchRejected {}

/// Instance-validated SOS1 Big-M promotion candidate before batch reconciliation.
///
/// This value is deliberately incomplete. Multiple candidates may consume the
/// same regular row, and target SOS1 IDs have not yet been allocated. Only
/// [`Sos1BigMPromotionPlan::new`] reconciles candidates and finalizes the
/// survivors into [`PlannedSos1BigMPromotion`] values.
///
/// # Invariants
///
/// Only [`Instance::build_sos1_big_m_promotion_candidate`] constructs this
/// value. Its SOS1 has a non-empty set of registered, finite-domain members
/// that are neither fixed nor existing assignment targets. Each fresh selector
/// is a registered, unfixed, full-domain binary variable, is not an existing
/// assignment target, and is distinct from every member and other fresh
/// selector in this candidate. The consumed rows are active, have the verified
/// link/cardinality shapes, and contain every active solver use of its fresh
/// selectors. These facts hold for the source instance at construction time;
/// the enclosing plan supplies the exclusive borrow and cross-candidate checks.
#[derive(Debug)]
struct Sos1BigMPromotionCandidate {
    fresh_selectors: BTreeMap<VariableID, VariableID>,
    relaxed_constraint_ids: BTreeSet<ConstraintID>,
    sos1_constraint: Sos1Constraint,
}

/// Fully validated effect for one SOS1 Big-M promotion.
///
/// # Invariants
///
/// This value is constructed only by [`Sos1BigMPromotionPlan::new`] and
/// can be applied only through that instance-bound aggregate plan.
/// For every `(member, selector)` in `candidate.fresh_selectors`:
///
/// - `selector` is distinct from every promoted member and every other fresh
///   selector;
/// - neither `member` nor `selector` is a target in the existing
///   `decision_variable_dependency` table;
/// - the only assignment added during commit is
///   `selector <- abs(signum(member))`.
///
/// Consequently, every new dependency edge starts at a fresh assignment target
/// and ends at a variable with no outgoing assignment edge. Existing assignment
/// right-hand sides may reference members or selectors, but those incoming edges
/// cannot close a cycle. Rebuilding `decision_variable_dependency` from the
/// existing and prepared assignments during commit therefore cannot fail; an
/// error there means this aggregate invariant was broken and is handled as a
/// panic rather than an invalid-request result.
#[derive(Debug)]
struct PlannedSos1BigMPromotion {
    sos1_constraint_id: Sos1ConstraintID,
    candidate: Sos1BigMPromotionCandidate,
}

/// Checked SOS1 Big-M batch promotion bound to its source [`Instance`].
///
/// Construct this value with [`Instance::plan_promote_sos1_big_m`]. Planning
/// does not mutate the instance. Because the plan retains an exclusive borrow,
/// its successful entries cannot become stale and it cannot be applied to a
/// different instance.
///
/// Use [`Self::is_fully_valid`] and [`Self::rejections`] to inspect the batch.
/// Dropping the plan applies nothing; [`Self::apply_if_fully_valid`] provides
/// all-or-nothing application, while [`Self::apply`] applies every successful
/// entry and preserves every rejection in the result map. Neither
/// policy requires a rollback operation.
///
/// The source cannot be changed while a plan remains available for application:
///
/// ```compile_fail,E0499
/// use ommx::{ATol, Function, Instance, Sos1BigMPromotionRequest};
/// let mut instance = Instance::default();
/// let request = Sos1BigMPromotionRequest::new();
/// let plan = instance.plan_promote_sos1_big_m(&request, ATol::default());
/// instance.set_objective(Function::Zero).unwrap();
/// let _ = plan.apply();
/// ```
///
/// # Invariants
///
/// - `instance` is the exact [`Instance`] against which `entries` was prepared;
/// - the exclusive borrow prevents that instance from changing before Apply;
/// - `entries` has exactly the same cardinality-constraint keys as the request;
/// - every successful effect consumes active regular rows disjoint from those
///   of every other successful effect;
/// - successful target SOS1 IDs are pairwise distinct and absent from both
///   the active and removed SOS1 collections;
/// - every successful SOS1 member and fresh selector is registered;
/// - no successful SOS1 member is an existing assignment target;
/// - the existing assignments are acyclic, and every new assignment has the
///   exact form `selector <- abs(signum(member))`;
/// - successful fresh-selector assignment targets are pairwise distinct,
///   absent from the existing assignment targets, and never a member of any
///   successful promotion; therefore the combined assignments preserve an
///   acyclic dependency graph; and
/// - rejected entries have no storage effect.
///
/// Apply consumes the plan and cannot introduce a new recoverable rejection
/// under these invariants. An `expect` reached while applying a successful
/// entry therefore identifies an internal invariant violation.
#[must_use = "inspect the plan and either apply it or deliberately drop it"]
#[derive(Debug)]
pub struct Sos1BigMPromotionPlan<'a> {
    instance: &'a mut Instance,
    entries: BTreeMap<ConstraintID, crate::Result<PlannedSos1BigMPromotion>>,
}

fn find_sos1_big_m_promotion_conflicts(
    candidates: &BTreeMap<ConstraintID, crate::Result<Sos1BigMPromotionCandidate>>,
) -> BTreeMap<ConstraintID, BTreeSet<ConstraintID>> {
    // Each candidate's selector-isolation check rejects a fresh selector used
    // by any active solver row outside that candidate's claimed formulation.
    // Consequently, two individually valid candidates can share a fresh
    // selector, or use another candidate's fresh selector as a member, only if
    // they also share at least one claimed regular row. Rejecting every
    // claimant of an overlapping row therefore establishes the complete
    // cross-candidate selector and dependency invariant.
    let mut regular_claimants = BTreeMap::<ConstraintID, Vec<ConstraintID>>::new();
    for (&cardinality, candidate) in candidates {
        let Ok(candidate) = candidate else {
            continue;
        };
        for &id in &candidate.relaxed_constraint_ids {
            regular_claimants.entry(id).or_default().push(cardinality);
        }
    }

    let mut conflicts = BTreeMap::<ConstraintID, BTreeSet<ConstraintID>>::new();
    for (id, claimants) in regular_claimants {
        if claimants.len() > 1 {
            for cardinality in claimants {
                conflicts.entry(cardinality).or_default().insert(id);
            }
        }
    }
    conflicts
}

fn sos1_big_m_promotion_conflict_error(
    cardinality: ConstraintID,
    regular_constraint_ids: BTreeSet<ConstraintID>,
) -> crate::Error {
    crate::error!(
        {
            ?cardinality,
            ?regular_constraint_ids
        },
        "SOS1 Big-M formulation with cardinality constraint {cardinality} conflicts with another individually valid request: consumed regular rows {regular_constraint_ids:?}"
    )
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
        // Let epsilon be the supplied ATol. In the real-number semantics, a
        // Continuous upper residual admits x - U <= epsilon, while SOS1
        // classifies values through epsilon as zero. Thus a potentially active
        // upper value exists exactly when U > 0; the lower side is symmetric
        // because L - x <= epsilon can reach below -epsilon exactly when L <
        // 0. The ATol term therefore cancels from this exact-sign rule. The
        // actual representable extrema are derived separately from
        // Bound::contains, so no expanded endpoint is needed here. Integer
        // bounds already have integer endpoints and promotion requires epsilon
        // < 1, so the same exact-sign rule detects an available nonzero integer.
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
        let (lower, upper, discrete) = match kind {
            Kind::Continuous => {
                // Derive the actual representable domain from the same
                // residual membership predicate used by DecisionVariable
                // validation. In particular, do not materialize `L - atol` or
                // `U + atol`: rounding those endpoints and subtracting the
                // stored bound again can disagree with Bound::contains.
                let (lower, upper) = bound.finite_feasible_extrema(atol);
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

impl<'a> Sos1BigMPromotionPlan<'a> {
    fn new(instance: &'a mut Instance, request: &Sos1BigMPromotionRequest, atol: ATol) -> Self {
        let mut candidates = request
            .iter()
            .map(|(&cardinality, claims)| {
                (
                    cardinality,
                    instance.build_sos1_big_m_promotion_candidate(cardinality, claims, atol),
                )
            })
            .collect::<BTreeMap<_, _>>();

        // Only individually valid formulations participate in compatibility
        // checks. Invalid requests must not make a valid request look
        // conflicting.
        for (cardinality, regular_constraint_ids) in
            find_sos1_big_m_promotion_conflicts(&candidates)
        {
            candidates.insert(
                cardinality,
                Err(sos1_big_m_promotion_conflict_error(
                    cardinality,
                    regular_constraint_ids,
                )),
            );
        }

        let survivor_count = candidates.values().filter(|entry| entry.is_ok()).count();
        if let Err(error) = instance
            .sos1_constraint_collection
            .ensure_unused_id_capacity(survivor_count)
        {
            let message = error.to_string();
            let entries = candidates
                .into_iter()
                .map(|(cardinality, candidate)| (cardinality, match candidate {
                    Ok(_) => Err(crate::error!(
                        { ?cardinality, survivor_count },
                        "Cannot allocate SOS1 constraint IDs for the compatible promotion batch: {message}"
                    )),
                    Err(error) => Err(error),
                }))
                .collect();
            return Self { instance, entries };
        }

        let first_id = (survivor_count > 0)
            .then(|| instance.sos1_constraint_collection.unused_id().into_inner());
        let mut offset = 0_u64;
        let entries = candidates
            .into_iter()
            .map(|(cardinality, candidate)| {
                (
                    cardinality,
                    candidate.map(|candidate| {
                        let sos1_constraint_id = Sos1ConstraintID::from(
                            first_id
                                .expect("a non-empty compatible batch has a first SOS1 ID")
                                .checked_add(offset)
                                .expect("batch ID capacity was validated before allocation"),
                        );
                        offset += 1;
                        PlannedSos1BigMPromotion {
                            sos1_constraint_id,
                            candidate,
                        }
                    }),
                )
            })
            .collect();

        Self { instance, entries }
    }

    /// Returns `true` when every request can be promoted.
    ///
    /// An empty batch is fully valid. This method only inspects the plan and
    /// does not mutate its bound instance.
    pub fn is_fully_valid(&self) -> bool {
        self.entries.values().all(Result::is_ok)
    }

    /// Iterates over rejected formulations in cardinality-constraint ID order.
    ///
    /// Each item contains the formulation's cardinality constraint ID and the
    /// caller-facing error that will be retained under that key by
    /// [`Self::apply`]. Inspecting rejections does not mutate the bound
    /// instance.
    pub fn rejections(&self) -> impl Iterator<Item = (ConstraintID, &crate::Error)> + '_ {
        self.entries
            .iter()
            .filter_map(|(&id, entry)| entry.as_ref().err().map(|error| (id, error)))
    }

    /// Applies the batch only when every request is valid.
    ///
    /// If any request was rejected during planning, this method returns a
    /// [`Sos1BigMPromotionBatchRejected`] containing every rejected cardinality ID
    /// and error chain, and leaves the bound instance unchanged. Otherwise it
    /// applies every planned effect and returns a report with the request's keys.
    /// An empty plan succeeds with an empty map.
    ///
    /// # Panics
    ///
    /// An `expect` failure denotes an SDK bug that violates the plan's stated
    /// invariants, not a failure permitted for a constructed plan. Rejected
    /// caller input takes the pre-application error path below.
    #[must_use = "a rejected batch leaves the instance unchanged"]
    pub fn apply_if_fully_valid(self) -> crate::Result<Sos1BigMPromotion> {
        if !self.is_fully_valid() {
            let Self {
                instance: _,
                entries,
            } = self;
            let request_count = entries.len();
            let rejections = entries
                .into_iter()
                .filter_map(|(id, entry)| entry.err().map(|error| (id, error)))
                .collect::<BTreeMap<_, _>>();
            debug_assert!(!rejections.is_empty());
            return Err(Sos1BigMPromotionBatchRejected {
                request_count,
                rejections,
            }
            .into());
        }

        Ok(self.apply())
    }

    /// Applies every successful entry to the unchanged bound instance.
    ///
    /// The returned map has exactly the same keys as the batch request.
    /// Rejected formulations retain their planning errors and have no
    /// storage effect. Successful entries are already jointly validated, so
    /// Apply introduces no new recoverable failure. A panic during their
    /// storage effects indicates an internal violation of this type's
    /// invariants.
    ///
    /// # Panics
    ///
    /// An `expect` failure denotes an SDK bug that violates the plan's stated
    /// invariants, not a failure permitted for a constructed plan. Every
    /// caller-controlled failure has already been recorded during planning.
    #[must_use = "each request has an aligned success or rejection result"]
    pub fn apply(self) -> Sos1BigMPromotion {
        let Self { instance, entries } = self;
        if entries.values().any(Result::is_ok) {
            let removal_reasons = entries
                .values()
                .filter_map(|entry| entry.as_ref().ok())
                .flat_map(|planned| {
                    planned.candidate.relaxed_constraint_ids.iter().map(|&id| {
                        (
                            id,
                            RemovedReason {
                                reason: "promoted validated SOS1 Big-M formulation".to_string(),
                                parameters: [(
                                    "sos1_constraint_id".to_string(),
                                    planned.sos1_constraint_id.to_string(),
                                )]
                                .into_iter()
                                .collect(),
                            },
                        )
                    })
                })
                .collect();

            // The prepared effects prove that every row is active and claimed
            // once, so this storage-level validation cannot reject a valid plan.
            instance
                .constraint_collection
                .move_active_rows_to_removed_with_reasons(removal_reasons)
                .expect("regular row availability is a Sos1BigMPromotionPlan invariant");

            let dependencies =
                std::mem::take(&mut instance.decision_variable_dependency)
                    .into_iter()
                    .chain(
                        entries
                            .values()
                            .filter_map(|entry| entry.as_ref().ok())
                            .flat_map(|planned| {
                                planned.candidate.fresh_selectors.iter().map(
                                    |(&member, &selector)| {
                                        (
                                            selector,
                                            Function::from(crate::linear!(member.into_inner()))
                                                .signum()
                                                .abs(),
                                        )
                                    },
                                )
                            }),
                    );
            instance.decision_variable_dependency = crate::AcyclicAssignments::new(dependencies)
                .expect("acyclic dependencies are a Sos1BigMPromotionPlan invariant");
        }

        entries
            .into_iter()
            .map(|(cardinality, entry)| {
                (
                    cardinality,
                    entry.map(|planned| {
                        let PlannedSos1BigMPromotion {
                            sos1_constraint_id,
                            candidate,
                        } = planned;
                        let Sos1BigMPromotionCandidate {
                            sos1_constraint, ..
                        } = candidate;
                        instance
                            .sos1_constraint_collection
                            .insert_active_with_context(
                                sos1_constraint_id,
                                sos1_constraint,
                                ConstraintContext::default(),
                            )
                            .expect("fresh SOS1 IDs are a Sos1BigMPromotionPlan invariant");
                        sos1_constraint_id
                    }),
                )
            })
            .collect()
    }
}

/// Mutable role-inference state for one legacy hinted member.
/// A successful conversion has at most one link per side and one selector;
/// conflicting insertions are rejected before a request is returned.
#[derive(Debug)]
struct LegacyV1FreshSelectorClaim {
    selector: VariableID,
    upper_link: Option<ConstraintID>,
    lower_link: Option<ConstraintID>,
}

impl Instance {
    /// Reconstruct an untrusted promotion request from one legacy v1 SOS1 hint.
    ///
    /// The legacy hint stores member IDs and an unordered flat list of Big-M
    /// row IDs, but not the member/selector/side role of each row. This
    /// conversion therefore inspects the supplied [`Instance`] and succeeds
    /// only when every listed row has one hinted member and one non-member
    /// selector and the complete mapping is unique. Duplicate IDs, stale
    /// active-row references, unused rows, and ambiguous or incomplete
    /// mappings are rejected.
    ///
    /// The returned request remains an untrusted claim. In particular, this
    /// conversion does not certify the Big-M bounds, link coefficients,
    /// cardinality semantics, or selector isolation. Pass it to
    /// [`Instance::promote_sos1_big_m`] for the complete checked promotion.
    /// Keeping conversion separate lets callers that obtained a raw
    /// [`crate::v1::Sos1`] independently decide when and how to apply it.
    /// The returned batch map contains exactly one entry, keyed by
    /// `hint.binary_constraint_id`. This method does not mutate the instance.
    pub fn sos1_big_m_promotion_request_from_v1_hint(
        &self,
        hint: &crate::v1::Sos1,
    ) -> crate::Result<Sos1BigMPromotionRequest> {
        let mut members = VariableIDSet::new();
        for &raw_id in &hint.decision_variables {
            let id = VariableID::from(raw_id);
            if !members.insert(id) {
                crate::bail!(
                    { ?id },
                    "Legacy v1 SOS1 hint lists decision variable {id:?} more than once"
                );
            }
        }
        if members.is_empty() {
            crate::bail!("Legacy v1 SOS1 hint must list at least one decision variable");
        }
        for &member in &members {
            if !self.decision_variables().contains_key(&member) {
                crate::bail!(
                    { ?member },
                    "Legacy v1 SOS1 hint member {member:?} is not registered in the current instance"
                );
            }
        }

        let cardinality_constraint = ConstraintID::from(hint.binary_constraint_id);
        let cardinality_row = self.constraints().get(&cardinality_constraint).ok_or_else(|| {
            crate::error!(
                { ?cardinality_constraint },
                "Legacy v1 SOS1 hint cardinality constraint {cardinality_constraint:?} is not active"
            )
        })?;

        let mut link_ids = BTreeSet::new();
        for &raw_id in &hint.big_m_constraint_ids {
            let id = ConstraintID::from(raw_id);
            if id == cardinality_constraint {
                crate::bail!(
                    { ?id },
                    "Legacy v1 SOS1 hint uses constraint {id:?} as both cardinality and Big-M link"
                );
            }
            if !link_ids.insert(id) {
                crate::bail!(
                    { ?id },
                    "Legacy v1 SOS1 hint lists Big-M constraint {id:?} more than once"
                );
            }
        }

        let mut fresh_claims = BTreeMap::<VariableID, LegacyV1FreshSelectorClaim>::new();
        let mut selector_owners = BTreeMap::<VariableID, VariableID>::new();
        for &id in &link_ids {
            let row = self.constraints().get(&id).ok_or_else(|| {
                crate::error!(
                    { ?id },
                    "Legacy v1 SOS1 hint Big-M constraint {id:?} is not active"
                )
            })?;
            let linear = row.function().as_linear().ok_or_else(|| {
                crate::error!(
                    { ?id },
                    "Legacy v1 SOS1 hint Big-M constraint {id:?} is not linear, so its selector role cannot be inferred"
                )
            })?;
            let support = linear
                .linear_terms()
                .map(|(variable, _)| variable)
                .collect::<VariableIDSet>();
            let linked_members = support.intersection(&members).copied().collect::<Vec<_>>();
            let selectors = support.difference(&members).copied().collect::<Vec<_>>();
            if linked_members.len() != 1 || selectors.len() != 1 {
                crate::bail!(
                    {
                        ?id,
                        hinted_members = linked_members.len(),
                        non_member_variables = selectors.len()
                    },
                    "Legacy v1 SOS1 hint Big-M constraint {id:?} must identify exactly one hinted member and one non-member selector"
                );
            }
            let member = linked_members[0];
            let selector = selectors[0];

            if let Some(previous_member) = selector_owners.insert(selector, member) {
                if previous_member != member {
                    crate::bail!(
                        { ?selector, ?member, ?previous_member },
                        "Legacy v1 SOS1 hint assigns selector {selector:?} to more than one member"
                    );
                }
            }

            let claim = fresh_claims
                .entry(member)
                .or_insert(LegacyV1FreshSelectorClaim {
                    selector,
                    upper_link: None,
                    lower_link: None,
                });
            if claim.selector != selector {
                crate::bail!(
                    { ?member, first_selector = ?claim.selector, ?selector },
                    "Legacy v1 SOS1 hint assigns different selectors to member {member:?}"
                );
            }

            let member_coefficient = linear
                .get(&LinearMonomial::Variable(member))
                .expect("member was collected from the linear row support")
                .into_inner();
            let (side, slot) = if member_coefficient > 0.0 {
                ("upper", &mut claim.upper_link)
            } else {
                ("lower", &mut claim.lower_link)
            };
            if let Some(previous_id) = slot.replace(id) {
                crate::bail!(
                    { ?member, ?selector, ?id, ?previous_id, side },
                    "Legacy v1 SOS1 hint assigns more than one {side} link to member {member:?}"
                );
            }
        }

        let cardinality_selectors = cardinality_row.function().required_ids();
        let mut selector_claims = BTreeMap::new();
        let mut assigned_selectors = VariableIDSet::new();
        let mut unresolved_members = Vec::new();
        for &member in &members {
            let variable = &self.decision_variables()[&member];
            let is_full_binary =
                variable.kind() == Kind::Binary && variable.bound() == Bound::of_binary();
            if is_full_binary {
                if fresh_claims.remove(&member).is_some() {
                    crate::bail!(
                        { ?member },
                        "Legacy v1 SOS1 hint assigns a Big-M link to full-domain binary member {member:?}"
                    );
                }
                assigned_selectors.insert(member);
                selector_claims.insert(member, Sos1BigMSelectorClaim::Reused);
            } else if let Some(claim) = fresh_claims.remove(&member) {
                assigned_selectors.insert(claim.selector);
                selector_claims.insert(
                    member,
                    Sos1BigMSelectorClaim::Fresh {
                        selector: claim.selector,
                        upper_link: claim.upper_link,
                        lower_link: claim.lower_link,
                    },
                );
            } else {
                unresolved_members.push(member);
            }
        }
        debug_assert!(fresh_claims.is_empty());

        let unresolved_selectors = cardinality_selectors
            .difference(&assigned_selectors)
            .copied()
            .collect::<Vec<_>>();
        match (unresolved_members.len(), unresolved_selectors.len()) {
            (0, 0) => {}
            (1, 1) => {
                selector_claims.insert(
                    unresolved_members[0],
                    Sos1BigMSelectorClaim::Fresh {
                        selector: unresolved_selectors[0],
                        upper_link: None,
                        lower_link: None,
                    },
                );
            }
            (members, selectors) if members > 1 && members == selectors => {
                crate::bail!(
                    {
                        unresolved_members = members,
                        unresolved_selectors = selectors
                    },
                    "Legacy v1 SOS1 hint is ambiguous: it does not map unlinked members to cardinality selectors"
                );
            }
            (members, selectors) => {
                crate::bail!(
                    {
                        unresolved_members = members,
                        unresolved_selectors = selectors
                    },
                    "Legacy v1 SOS1 hint is incomplete: it does not uniquely identify every member selector"
                );
            }
        }

        Ok(BTreeMap::from([(cardinality_constraint, selector_claims)]))
    }

    /// Check and reconcile SOS1 Big-M promotion requests as one batch.
    ///
    /// Every request is checked against the same unchanged instance. The
    /// resulting plan retains exactly one success or rejection for every
    /// cardinality constraint ID. Individually invalid formulations are rejected
    /// independently. If otherwise valid requests consume any of the same
    /// regular rows, every participant in that conflict is rejected; unrelated
    /// requests remain applicable. SOS1 members may be shared by independent
    /// requests.
    ///
    /// An empty request map produces an empty, fully valid plan without
    /// inspecting `atol`. The tolerance is validated only for requests that
    /// are actually checked.
    ///
    /// Each request is untrusted. This method validates all of the following
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
    ///
    /// Rust-side concepts not modeled by the initial Lean semantics are
    /// handled conservatively. Selected semi variables, fixed or dependent
    /// members, fixed binary member bounds, and already fixed/dependent fresh
    /// selectors are rejected. The output objective, removed constraints,
    /// named functions, and dependency RHS expressions are outside active
    /// solver input. They may reference fresh selectors and observe the
    /// canonical dependent-selector value during evaluation. Row context,
    /// selector labels, unrelated nonlinear expressions, and unrelated special
    /// constraints are also preserved unchanged.
    /// Calling this family-specific method is the explicit request to add the
    /// SOS1 capability to the instance; request rejection means only that the
    /// claimed formulation is outside this conservative checker.
    ///
    /// For a raw link `a * (t - M * z) <= 0`, normalization changes the
    /// comparison tolerance to `atol / a`. The checker accepts the scale and
    /// Big-M only when that threshold agrees with SOS1's `abs(member) <= atol`
    /// zero classification: reconstructed `z = 0` must satisfy the raw row,
    /// every active member must force `z = 1`, and `z = 1` must cover the full
    /// member domain feasible under the same `atol`. Continuous domains are
    /// derived directly from [`Bound::contains`] without constructing expanded
    /// endpoints, while Integer domains are checked at their discrete values.
    /// The checker imposes no domain-derived upper cap on a finite,
    /// representable loose Big-M; non-unit scales and tolerance-level shortfalls
    /// are accepted only where these feasibility conditions hold. The raw
    /// residual is checked as well as its normalized form to avoid accepting an
    /// f64 normalization-rounding artifact.
    ///
    /// For a non-empty request map, the supplied `atol` must be finite and
    /// smaller than one so the exact binary selector-cardinality row still
    /// means "at most one". Relation, variable support, zero constant, and
    /// cardinality remain exact structural requirements. Link presence remains
    /// determined by the exact sign of the stored domain endpoint. The
    /// equivalence guarantee is local to the claimed formulation and
    /// parameterized by this `atol`; callers must use the same tolerance for
    /// subsequent state reconstruction and evaluation. The transformed
    /// [`Instance`] does not store a global evaluation tolerance, and unrelated
    /// removed history is preserved rather than reinterpreted.
    ///
    /// Planning does not mutate the instance. The returned plan holds its
    /// exclusive borrow, so callers may inspect [`Sos1BigMPromotionPlan::rejections`]
    /// and drop it to apply nothing. If the caller invokes
    /// [`Sos1BigMPromotionPlan::apply`], every successful request relaxes
    /// its verified formulation rows, retains fresh selectors as dependent
    /// variables, and inserts a new active SOS1 constraint. Rejected requests
    /// have no storage effect, while independent successful entries are still
    /// applied.
    ///
    /// Target IDs are allocated together only after validation and conflict
    /// detection. If the target SOS1 ID namespace cannot fit every otherwise
    /// successful request, those requests are rejected before mutation. The
    /// returned batch plan holds an exclusive borrow of this instance until
    /// it is dropped or consumed by Apply, so successful effects cannot become
    /// stale. Neither planning nor application clones the [`Instance`], and no
    /// rollback path is needed because all recoverable failures are retained
    /// by the plan before mutation.
    pub fn plan_promote_sos1_big_m(
        &mut self,
        request: &Sos1BigMPromotionRequest,
        atol: ATol,
    ) -> Sos1BigMPromotionPlan<'_> {
        Sos1BigMPromotionPlan::new(self, request, atol)
    }

    /// Plans and immediately applies SOS1 Big-M promotions as one batch.
    ///
    /// This is the best-effort convenience form of
    /// [`Self::plan_promote_sos1_big_m`]. The returned map has exactly the same
    /// keys as the batch request. Independently valid formulations are
    /// applied even when other entries are rejected. Use
    /// [`Self::promote_sos1_big_m_if_fully_valid`] when all requests must be
    /// valid, or [`Self::plan_promote_sos1_big_m`] to inspect the plan before
    /// choosing an application policy.
    #[must_use = "each request has an aligned success or rejection result"]
    pub fn promote_sos1_big_m(
        &mut self,
        request: &Sos1BigMPromotionRequest,
        atol: ATol,
    ) -> Sos1BigMPromotion {
        self.plan_promote_sos1_big_m(request, atol).apply()
    }

    /// Plans and applies SOS1 Big-M promotions only if the full batch is valid.
    ///
    /// This all-or-nothing convenience form leaves the instance unchanged and
    /// returns [`Sos1BigMPromotionBatchRejected`] in the error chain if any
    /// formulation is rejected. On success it returns the same batch report
    /// as the best-effort API, with every entry successful. An empty request succeeds without inspecting
    /// `atol`.
    ///
    /// Use [`Self::promote_sos1_big_m`] when independently valid requests should
    /// still be applied from a partially rejected batch, or
    /// [`Self::plan_promote_sos1_big_m`] when the plan must be inspected before
    /// choosing an application policy.
    ///
    /// # Panics
    ///
    /// Panics only if an internal plan invariant is violated while applying a
    /// fully valid plan. See
    /// [`Sos1BigMPromotionPlan::apply_if_fully_valid`] for details.
    pub fn promote_sos1_big_m_if_fully_valid(
        &mut self,
        request: &Sos1BigMPromotionRequest,
        atol: ATol,
    ) -> crate::Result<Sos1BigMPromotion> {
        self.plan_promote_sos1_big_m(request, atol)
            .apply_if_fully_valid()
    }

    fn build_sos1_big_m_promotion_candidate(
        &self,
        cardinality_constraint: ConstraintID,
        selector_claims: &BTreeMap<VariableID, Sos1BigMSelectorClaim>,
        atol: ATol,
    ) -> crate::Result<Sos1BigMPromotionCandidate> {
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
        if selector_claims.is_empty() {
            crate::bail!("SOS1 Big-M promotion request must contain at least one member");
        }

        let members = selector_claims.keys().copied().collect::<VariableIDSet>();
        let mut fresh_selectors = BTreeMap::new();
        let mut fresh_selector_ids = VariableIDSet::new();
        let mut relaxed_constraint_ids = BTreeSet::new();

        for (&member, &claim) in selector_claims {
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

        if !relaxed_constraint_ids.insert(cardinality_constraint) {
            crate::bail!(
                { ?cardinality_constraint },
                "SOS1 cardinality constraint is also claimed as a member link"
            );
        }
        let selector_ids = selector_claims.iter().map(|(&member, claim)| match claim {
            Sos1BigMSelectorClaim::Reused => member,
            Sos1BigMSelectorClaim::Fresh { selector, .. } => *selector,
        });
        let expected_cardinality = canonical_sos1_big_m_cardinality(selector_ids)?;
        self.ensure_exact_sos1_formulation_row(
            cardinality_constraint,
            &expected_cardinality,
            "cardinality",
        )?;

        self.ensure_variables_isolated_for_sos1_promotion(
            &fresh_selector_ids,
            &relaxed_constraint_ids,
        )?;

        let sos1_constraint = Sos1Constraint::new(members)?;

        Ok(Sos1BigMPromotionCandidate {
            fresh_selectors,
            relaxed_constraint_ids,
            sos1_constraint,
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

    /// Prove that fresh selectors occur in active solver input only in the
    /// regular formulation rows claimed by the request.
    ///
    /// The active-formulation traversal and its family-typed `Except` visitor
    /// are the source of truth. [`Instance::decision_variable_usage`] uses the
    /// same visitor with an empty exception set. The output objective, removed
    /// rows, named functions, and dependency RHS expressions are intentionally
    /// outside the traversal; they may retain references and observe the
    /// canonical dependent-selector value during evaluation.
    fn ensure_variables_isolated_for_sos1_promotion(
        &self,
        private_ids: &VariableIDSet,
        excluded_regular_constraints: &BTreeSet<ConstraintID>,
    ) -> crate::Result<()> {
        let except = super::analysis::SolverUseExcept {
            regular_constraints: excluded_regular_constraints.clone(),
            ..Default::default()
        };
        let used = super::analysis::used_decision_variable_ids_except(self, &except);
        for &id in private_ids {
            if used.contains(&id) {
                crate::bail!(
                    { ?id },
                    "Fresh SOS1 selector {id:?} is used by active solver input outside the claimed formulation rows"
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
        coeff, linear, quadratic, ATol, AcyclicAssignments, DecisionVariable, DecisionVariableRole,
        Evaluate, Function, IndicatorConstraint, ModelingLabel, NamedFunction, OneHotConstraint,
        RemovedReason, Sense,
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

    fn two_term_link_for(
        member: VariableID,
        selector: VariableID,
        member_coefficient: f64,
        selector_coefficient: f64,
    ) -> Constraint {
        let function = (crate::Linear::single_term(
            LinearMonomial::Variable(member),
            crate::Coefficient::try_from(member_coefficient).unwrap(),
        ) + crate::Linear::single_term(
            LinearMonomial::Variable(selector),
            crate::Coefficient::try_from(selector_coefficient).unwrap(),
        ))
        .unwrap();
        Constraint::less_than_or_equal_to_zero(Function::from(function))
    }

    fn two_term_link(member_coefficient: f64, selector_coefficient: f64) -> Constraint {
        two_term_link_for(
            member_integer_id(),
            selector_id(),
            member_coefficient,
            selector_coefficient,
        )
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
        let request = BTreeMap::from([(
            cardinality_row_id(),
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
        )]);
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
        let request = BTreeMap::from([(
            cardinality_row_id(),
            BTreeMap::from([(
                member_integer_id(),
                Sos1BigMSelectorClaim::Fresh {
                    selector: selector_id(),
                    upper_link: upper_link_id,
                    lower_link: lower_link_id,
                },
            )]),
        )]);
        (instance, request)
    }

    fn shared_member_batch_instance() -> (Instance, Sos1BigMPromotionRequest) {
        let member = member_integer_id();
        let selectors = [selector_id(), VariableID::from(11)];
        let row_ids = [
            [upper_row_id(), lower_row_id(), cardinality_row_id()],
            [
                ConstraintID::from(200),
                ConstraintID::from(201),
                ConstraintID::from(202),
            ],
        ];
        let mut constraints = BTreeMap::new();
        let mut requests = BTreeMap::new();
        for (selector, [upper, lower, cardinality]) in selectors.into_iter().zip(row_ids) {
            constraints.insert(upper, two_term_link_for(member, selector, 1.0, -3.0));
            constraints.insert(lower, two_term_link_for(member, selector, -1.0, -2.0));
            constraints.insert(
                cardinality,
                canonical_sos1_big_m_cardinality([selector]).unwrap(),
            );
            requests.insert(
                cardinality,
                BTreeMap::from([(
                    member,
                    Sos1BigMSelectorClaim::Fresh {
                        selector,
                        upper_link: Some(upper),
                        lower_link: Some(lower),
                    },
                )]),
            );
        }
        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([
                (member, integer(-2.0, 3.0)),
                (selectors[0], DecisionVariable::binary()),
                (selectors[1], DecisionVariable::binary()),
            ]))
            .constraints(constraints)
            .build()
            .unwrap();
        (instance, requests)
    }

    fn overlapping_batch_instance() -> (Instance, Sos1BigMPromotionRequest) {
        let (mut instance, mut request) = shared_member_batch_instance();
        let duplicate_cardinality = ConstraintID::from(103);
        let row = instance.constraints()[&cardinality_row_id()].clone();
        instance
            .constraint_collection
            .insert_active_with_context(duplicate_cardinality, row, ConstraintContext::default())
            .unwrap();
        request.insert(
            duplicate_cardinality,
            request[&cardinality_row_id()].clone(),
        );
        (instance, request)
    }

    fn promote_one(
        instance: &mut Instance,
        request: &Sos1BigMPromotionRequest,
        atol: ATol,
    ) -> crate::Result<Sos1ConstraintID> {
        let mut outcomes = instance.promote_sos1_big_m(request, atol);
        assert_eq!(
            outcomes.len(),
            1,
            "one request must produce exactly one aligned SOS1 promotion outcome"
        );
        outcomes
            .pop_first()
            .expect("one formulation must retain its keyed SOS1 promotion outcome")
            .1
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
        let error = promote_one(&mut instance, request, atol).unwrap_err();
        assert!(
            error.to_string().contains(expected),
            "expected {expected:?} in error, got: {error:#}"
        );
        assert_eq!(instance, before);
    }

    #[test]
    fn promotes_mixed_formulation_and_retains_history() {
        let (mut instance, request) = mixed_instance();
        assert_eq!(
            instance.decision_variable_role(selector_id()),
            Some(DecisionVariableRole::Used)
        );
        let promotion = promote_one(&mut instance, &request, ATol::default()).unwrap();

        assert_eq!(promotion, Sos1ConstraintID::from(0));
        assert_eq!(
            &instance.sos1_constraints()[&promotion].variables,
            &VariableIDSet::from([member_binary_id(), member_integer_id()])
        );
        assert_eq!(
            instance.decision_variable_dependency().get(&selector_id()),
            Some(
                &Function::from(crate::linear!(member_integer_id().into_inner()))
                    .signum()
                    .abs()
            )
        );
        assert_eq!(
            instance
                .removed_constraints()
                .keys()
                .copied()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([upper_row_id(), lower_row_id(), cardinality_row_id()])
        );
        assert!(instance.decision_variables().contains_key(&selector_id()));
        assert_eq!(
            instance.decision_variable_role(selector_id()),
            Some(DecisionVariableRole::Dependent)
        );
        assert!(instance.constraints().is_empty());
        assert_eq!(instance.removed_constraints().len(), 3);
        assert!(instance
            .removed_constraints()
            .values()
            .all(|(_, reason)| reason.reason == "promoted validated SOS1 Big-M formulation"));
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
    fn batch_promotes_disjoint_formulations_with_shared_member() {
        let (mut instance, requests) = shared_member_batch_instance();

        let plan = instance.plan_promote_sos1_big_m(&requests, ATol::default());
        assert!(plan.is_fully_valid());
        assert_eq!(plan.rejections().count(), 0);
        let outcomes = plan.apply();

        assert_eq!(outcomes.len(), 2);
        assert_eq!(
            outcomes.keys().collect::<Vec<_>>(),
            requests.keys().collect::<Vec<_>>()
        );
        let promotions = outcomes
            .values()
            .map(|outcome| *outcome.as_ref().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            promotions,
            vec![Sos1ConstraintID::from(0), Sos1ConstraintID::from(1)]
        );
        assert!(promotions
            .iter()
            .all(|promotion| instance.sos1_constraints()[promotion].variables
                == VariableIDSet::from([member_integer_id()])));
        assert!(instance.constraints().is_empty());
        assert_eq!(instance.removed_constraints().len(), 6);
        assert_eq!(instance.sos1_constraints().len(), 2);
        assert_eq!(instance.decision_variable_dependency().len(), 2);
        assert!(instance
            .decision_variable_dependency()
            .get(&selector_id())
            .is_some());
        assert!(instance
            .decision_variable_dependency()
            .get(&VariableID::from(11))
            .is_some());
    }

    #[test]
    fn empty_batch_is_an_atol_independent_no_op() {
        let (mut instance, _) = mixed_instance();
        let before = instance.clone();

        let plan =
            instance.plan_promote_sos1_big_m(&BTreeMap::new(), ATol::new(f64::INFINITY).unwrap());
        assert!(plan.is_fully_valid());
        assert_eq!(plan.rejections().count(), 0);
        let outcomes = plan.apply();

        assert!(outcomes.is_empty());
        assert_eq!(instance, before);
    }

    #[test]
    fn batch_reconstruction_preserves_existing_dependencies_on_both_selectors() {
        let (mut instance, request) = shared_member_batch_instance();
        instance
            .add_decision_variable(unrelated_id(), integer(0.0, 2.0), Default::default())
            .unwrap();
        instance.decision_variable_dependency = AcyclicAssignments::new([(
            unrelated_id(),
            Function::from((linear!(10) + linear!(11)).unwrap()).abs(),
        )])
        .unwrap();
        let original_dependency = instance
            .decision_variable_dependency
            .get(&unrelated_id())
            .unwrap()
            .clone();

        let report = instance
            .promote_sos1_big_m_if_fully_valid(&request, ATol::default())
            .unwrap();

        assert!(report.values().all(Result::is_ok));
        assert_eq!(instance.decision_variable_dependency.len(), 3);
        assert_eq!(
            instance.decision_variable_dependency.get(&unrelated_id()),
            Some(&original_dependency)
        );
        for (member_value, selector_value) in [(0.0, 0.0), (-2.0, 1.0), (3.0, 1.0)] {
            let state = instance
                .populate_state(
                    crate::v1::State::from_iter([(1, member_value)]),
                    ATol::default(),
                )
                .unwrap();
            assert_eq!(state.entries[&10], selector_value);
            assert_eq!(state.entries[&11], selector_value);
            assert_eq!(state.entries[&20], 2.0 * selector_value);
        }
        assert_eq!(
            Instance::from_v2_bytes(&instance.to_v2_bytes()).unwrap(),
            instance
        );
    }

    #[test]
    fn rejected_plan_can_be_inspected_and_dropped_without_mutation() {
        let (mut instance, mut requests) = shared_member_batch_instance();
        let claims = requests.remove(&cardinality_row_id()).unwrap();
        requests.insert(ConstraintID::from(999), claims);
        let before = instance.clone();

        {
            let plan = instance.plan_promote_sos1_big_m(&requests, ATol::default());
            assert!(!plan.is_fully_valid());
            let rejections = plan.rejections().collect::<Vec<_>>();
            assert_eq!(rejections.len(), 1);
            assert_eq!(rejections[0].0, ConstraintID::from(999));
            assert!(rejections[0]
                .1
                .to_string()
                .contains("cardinality constraint ConstraintID(999) is not active"));
        }

        assert_eq!(instance, before);
    }

    #[test]
    fn fully_valid_application_reports_every_rejection_without_mutation() {
        let (mut instance, mut requests) = overlapping_batch_instance();
        requests.insert(ConstraintID::from(999), BTreeMap::new());
        let before = instance.clone();

        let error = instance
            .promote_sos1_big_m_if_fully_valid(&requests, ATol::default())
            .unwrap_err();
        let rejected = error
            .downcast_ref::<Sos1BigMPromotionBatchRejected>()
            .expect("the strict batch error retains its domain signal");

        assert_eq!(rejected.request_count(), 4);
        let rejections = rejected.rejections().collect::<Vec<_>>();
        assert_eq!(
            rejections
                .iter()
                .map(|(index, _)| *index)
                .collect::<Vec<_>>(),
            vec![
                cardinality_row_id(),
                ConstraintID::from(103),
                ConstraintID::from(999)
            ]
        );
        assert!(rejections[2]
            .1
            .to_string()
            .contains("must contain at least one member"));
        assert!(rejections[..2].iter().all(|(_, error)| error
            .to_string()
            .contains("outside the claimed formulation rows")));
        assert!(error
            .to_string()
            .contains("SOS1 Big-M promotion rejected 3 of 4 requests"));
        assert_eq!(instance, before);
    }

    #[test]
    fn fully_valid_application_preserves_all_request_keys() {
        let (mut instance, requests) = shared_member_batch_instance();

        let promotions = instance
            .promote_sos1_big_m_if_fully_valid(&requests, ATol::default())
            .unwrap();

        assert_eq!(promotions.len(), 2);
        assert_eq!(
            promotions
                .values()
                .map(|result| *result.as_ref().unwrap())
                .collect::<Vec<_>>(),
            vec![Sos1ConstraintID::from(0), Sos1ConstraintID::from(1)]
        );
        assert_eq!(
            promotions.keys().collect::<Vec<_>>(),
            requests.keys().collect::<Vec<_>>()
        );
        assert!(instance.constraints().is_empty());
        assert_eq!(instance.removed_constraints().len(), 6);
        assert_eq!(instance.sos1_constraints().len(), 2);
    }

    #[test]
    fn empty_fully_valid_application_is_an_atol_independent_no_op() {
        let (mut instance, _) = mixed_instance();
        let before = instance.clone();

        let promotions = instance
            .promote_sos1_big_m_if_fully_valid(&BTreeMap::new(), ATol::new(f64::INFINITY).unwrap())
            .unwrap();

        assert!(promotions.is_empty());
        assert_eq!(instance, before);
    }

    #[test]
    fn batch_rejects_overlapping_formulations_and_promotes_unrelated_formulation() {
        let (mut instance, requests) = overlapping_batch_instance();

        let outcomes = instance.promote_sos1_big_m(&requests, ATol::default());

        assert_eq!(
            outcomes.keys().collect::<Vec<_>>(),
            requests.keys().collect::<Vec<_>>()
        );
        assert!(outcomes[&cardinality_row_id()]
            .as_ref()
            .unwrap_err()
            .to_string()
            .contains("outside the claimed formulation rows"));
        assert!(outcomes[&ConstraintID::from(103)].is_err());
        let promotion = outcomes[&ConstraintID::from(202)].as_ref().unwrap();
        assert_eq!(*promotion, Sos1ConstraintID::from(0));
        assert_eq!(instance.sos1_constraints().len(), 1);
        for id in [
            upper_row_id(),
            lower_row_id(),
            cardinality_row_id(),
            ConstraintID::from(103),
        ] {
            assert!(instance.constraints().contains_key(&id));
        }
        for id in [
            ConstraintID::from(200),
            ConstraintID::from(201),
            ConstraintID::from(202),
        ] {
            assert!(instance.removed_constraints().contains_key(&id));
        }
    }

    #[test]
    fn batch_rejects_an_invalid_request_and_promotes_an_independent_request() {
        let (mut instance, mut requests) = shared_member_batch_instance();
        let claims = requests.remove(&cardinality_row_id()).unwrap();
        requests.insert(ConstraintID::from(999), claims);

        let plan = instance.plan_promote_sos1_big_m(&requests, ATol::default());
        assert!(!plan.is_fully_valid());
        assert_eq!(
            plan.rejections()
                .map(|(index, _)| index)
                .collect::<Vec<_>>(),
            vec![ConstraintID::from(999)]
        );
        let outcomes = plan.apply();

        assert_eq!(outcomes.len(), 2);
        assert_eq!(
            outcomes.keys().collect::<Vec<_>>(),
            requests.keys().collect::<Vec<_>>()
        );
        assert!(outcomes[&ConstraintID::from(999)].is_err());
        assert_eq!(
            *outcomes[&ConstraintID::from(202)].as_ref().unwrap(),
            Sos1ConstraintID::from(0)
        );
        for id in [upper_row_id(), lower_row_id(), cardinality_row_id()] {
            assert!(instance.constraints().contains_key(&id));
        }
        for id in [
            ConstraintID::from(200),
            ConstraintID::from(201),
            ConstraintID::from(202),
        ] {
            assert!(instance.removed_constraints().contains_key(&id));
        }
    }

    #[test]
    fn invalid_request_does_not_conflict_with_a_valid_claim_on_the_same_rows() {
        let (mut instance, mut request) = mixed_instance();
        let mut invalid = request[&cardinality_row_id()].clone();
        invalid.insert(VariableID::from(999), Sos1BigMSelectorClaim::Reused);
        request.insert(ConstraintID::from(999), invalid);

        let outcomes = instance.promote_sos1_big_m(&request, ATol::default());

        assert_eq!(outcomes.len(), 2);
        assert!(outcomes[&ConstraintID::from(999)].is_err());
        assert_eq!(
            *outcomes[&cardinality_row_id()].as_ref().unwrap(),
            Sos1ConstraintID::from(0)
        );
        assert!(instance.constraints().is_empty());
        assert_eq!(instance.removed_constraints().len(), 3);
        assert_eq!(instance.sos1_constraints().len(), 1);
    }

    #[test]
    fn batch_rejects_id_exhaustion_before_any_mutation() {
        let (mut instance, requests) = shared_member_batch_instance();
        instance
            .sos1_constraint_collection
            .insert_active_with_context(
                Sos1ConstraintID::from(u64::MAX),
                Sos1Constraint::new(VariableIDSet::from([member_integer_id()])).unwrap(),
                ConstraintContext::default(),
            )
            .unwrap();
        let before = instance.clone();

        let outcomes = instance.promote_sos1_big_m(&requests, ATol::default());

        assert!(outcomes.values().all(Result::is_err));
        assert!(outcomes.values().all(|outcome| outcome
            .as_ref()
            .unwrap_err()
            .to_string()
            .contains("Cannot allocate SOS1 constraint IDs")));
        assert_eq!(instance, before);
    }

    #[test]
    fn batch_id_reservation_accounts_for_removed_ids_and_the_whole_batch() {
        for (maximum, fits) in [(u64::MAX - 2, true), (u64::MAX - 1, false)] {
            for strict in [false, true] {
                let (mut instance, request) = shared_member_batch_instance();
                let maximum = Sos1ConstraintID::from(maximum);
                instance
                    .sos1_constraint_collection
                    .insert_active_with_context(
                        maximum,
                        Sos1Constraint::new(VariableIDSet::from([member_integer_id()])).unwrap(),
                        ConstraintContext::default(),
                    )
                    .unwrap();
                instance
                    .sos1_constraint_collection
                    .relax(maximum, test_removed_reason())
                    .unwrap();
                let before = instance.clone();

                let report = if strict {
                    match instance.promote_sos1_big_m_if_fully_valid(&request, ATol::default()) {
                        Ok(report) => report,
                        Err(error) => {
                            assert!(!fits);
                            let rejected = error
                                .downcast_ref::<Sos1BigMPromotionBatchRejected>()
                                .unwrap();
                            assert_eq!(rejected.request_count(), 2);
                            assert_eq!(rejected.rejections().count(), 2);
                            assert_eq!(instance, before);
                            continue;
                        }
                    }
                } else {
                    instance.promote_sos1_big_m(&request, ATol::default())
                };
                assert_eq!(
                    report.keys().collect::<Vec<_>>(),
                    request.keys().collect::<Vec<_>>()
                );
                if fits {
                    assert_eq!(
                        report
                            .values()
                            .map(|outcome| outcome.as_ref().unwrap().into_inner())
                            .collect::<Vec<_>>(),
                        vec![u64::MAX - 1, u64::MAX],
                    );
                    assert!(instance.removed_sos1_constraints().contains_key(&maximum));
                    assert_eq!(instance.removed_constraints().len(), 6);
                } else {
                    assert!(report.values().all(Result::is_err));
                    assert_eq!(instance, before);
                }
            }
        }
    }

    #[test]
    fn a_fresh_selector_cannot_become_a_member_of_another_successful_promotion() {
        let (mut instance, mut request) = shared_member_batch_instance();
        let reused_cardinality = instance
            .add_constraint(
                canonical_sos1_big_m_cardinality([selector_id()]).unwrap(),
                ConstraintContext::default(),
            )
            .unwrap();
        request.insert(
            reused_cardinality,
            BTreeMap::from([(selector_id(), Sos1BigMSelectorClaim::Reused)]),
        );

        let report = instance.promote_sos1_big_m(&request, ATol::default());

        assert!(report[&cardinality_row_id()]
            .as_ref()
            .unwrap_err()
            .to_string()
            .contains("outside the claimed formulation rows"));
        assert!(report[&ConstraintID::from(202)].is_ok());
        let reused_sos1 = report[&reused_cardinality].as_ref().unwrap();
        assert_eq!(
            instance.sos1_constraints()[reused_sos1].variables,
            VariableIDSet::from([selector_id()])
        );
        assert!(instance
            .decision_variable_dependency()
            .get(&selector_id())
            .is_none());
        assert!(instance
            .decision_variable_dependency()
            .get(&VariableID::from(11))
            .is_some());
        assert!(instance.constraints().contains_key(&cardinality_row_id()));
    }

    #[test]
    fn reconciliation_rejects_every_row_claimant_but_ignores_invalid_entries() {
        let (instance, request) = mixed_instance();
        let build = || {
            instance
                .build_sos1_big_m_promotion_candidate(
                    cardinality_row_id(),
                    &request[&cardinality_row_id()],
                    ATol::default(),
                )
                .unwrap()
        };
        // Isolate reconciliation from candidate validation: current selector
        // isolation already rejects duplicate formulations with distinct
        // cardinality rows. This directly guards the aggregate row invariant
        // even if candidate validation is extended in the future.
        let mut first = build();
        first.relaxed_constraint_ids = BTreeSet::from([ConstraintID::from(1)]);
        let mut middle = build();
        middle.relaxed_constraint_ids =
            BTreeSet::from([ConstraintID::from(1), ConstraintID::from(2)]);
        let mut last = build();
        last.relaxed_constraint_ids = BTreeSet::from([ConstraintID::from(2)]);
        let candidates = BTreeMap::from([
            (ConstraintID::from(10), Ok(first)),
            (ConstraintID::from(20), Ok(middle)),
            (ConstraintID::from(30), Ok(last)),
            (ConstraintID::from(40), Ok(build())),
            (ConstraintID::from(50), Err(crate::error!("invalid claim"))),
        ]);

        assert_eq!(
            find_sos1_big_m_promotion_conflicts(&candidates),
            BTreeMap::from([
                (
                    ConstraintID::from(10),
                    BTreeSet::from([ConstraintID::from(1)])
                ),
                (
                    ConstraintID::from(20),
                    BTreeSet::from([ConstraintID::from(1), ConstraintID::from(2)])
                ),
                (
                    ConstraintID::from(30),
                    BTreeSet::from([ConstraintID::from(2)])
                ),
            ])
        );
    }

    #[test]
    fn promotion_preserves_the_claimed_formulations_projected_feasibility() {
        let atol = ATol::default();
        let (original, request) = mixed_instance();
        let mut promoted = original.clone();
        let _ = promote_one(&mut promoted, &request, atol).unwrap();

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
        let promotion = promote_one(&mut instance, &request, atol).unwrap();

        let near_zero = instance
            .evaluate(&crate::v1::State::from_iter([(1, 5.0e-7)]), atol)
            .unwrap();
        assert_eq!(near_zero.state().entries[&selector_id().into_inner()], 0.0);
        let evaluated = near_zero
            .evaluated_sos1_constraints()
            .get(&promotion)
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
            .get(&promotion)
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
        let sampled = &sample_set.sos1_constraints()[&promotion];
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
                .get(&promotion)
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
    fn accepts_tight_unit_scale_links_at_continuous_bound_endpoints() {
        let atol = ATol::new(1.0e-6).unwrap();
        let bound = Bound::new(-2.0, 2.0).unwrap();
        let member = DecisionVariable::new(Kind::Continuous, bound, ATol::default()).unwrap();
        let (original, request) =
            fresh_instance(member.clone(), Some(upper_row_id()), Some(lower_row_id()));
        let mut promoted = original.clone();

        let promotion = promote_one(&mut promoted, &request, atol).unwrap();
        let (lower, upper) = bound.finite_feasible_extrema(atol);
        for member_value in [lower, upper] {
            let original_solution = original
                .evaluate(
                    &crate::v1::State::from_iter([
                        (member_integer_id().into_inner(), member_value),
                        (selector_id().into_inner(), 1.0),
                    ]),
                    atol,
                )
                .unwrap();
            let promoted_solution = promoted
                .evaluate(
                    &crate::v1::State::from_iter([(
                        member_integer_id().into_inner(),
                        member_value,
                    )]),
                    atol,
                )
                .unwrap();

            assert!(original_solution.feasible());
            assert!(promoted_solution.feasible());
            assert!(promoted_solution.feasible_relaxed());
            assert_eq!(
                promoted_solution.state().entries[&selector_id().into_inner()],
                1.0
            );
            assert!(promoted_solution
                .evaluated_sos1_constraints()
                .contains_key(&promotion));
        }

        let (mut too_small, request) =
            fresh_instance(member, Some(upper_row_id()), Some(lower_row_id()));
        too_small
            .constraint_collection
            .replace_active_row(upper_row_id(), upper_link(1.0, 2.0 - atol.into_inner()))
            .unwrap();
        assert_atomic_rejection_with_atol(
            too_small,
            &request,
            atol,
            "does not cover the ATol-feasible member domain",
        );
    }

    #[test]
    fn accepts_finite_member_bounds_that_exclude_zero() {
        let (mut positive, request) = fresh_instance(integer(1.0, 3.0), Some(upper_row_id()), None);
        let _promotion = promote_one(&mut positive, &request, ATol::default()).unwrap();
        assert_eq!(positive.sos1_constraints().len(), 1);

        let (mut negative, request) =
            fresh_instance(integer(-3.0, -1.0), None, Some(lower_row_id()));
        let _promotion = promote_one(&mut negative, &request, ATol::default()).unwrap();
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
        let request = BTreeMap::from([(
            cardinality,
            BTreeMap::from([
                (VariableID::from(3), Sos1BigMSelectorClaim::Reused),
                (VariableID::from(4), Sos1BigMSelectorClaim::Reused),
            ]),
        )]);
        let promotion = promote_one(&mut instance, &request, ATol::default()).unwrap();
        assert_eq!(
            instance.sos1_constraints()[&promotion].variables,
            VariableIDSet::from([VariableID::from(3), VariableID::from(4)])
        );
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

        let _promotion = promote_one(&mut instance, &request, ATol::new(1.0e-6).unwrap()).unwrap();

        assert_eq!(instance.removed_constraints().len(), 3);
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

        let _ = promote_one(&mut instance, &request, atol).unwrap();

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
        let _ = promote_one(&mut upper_on_boundary, &request, atol).unwrap();
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
        let _ = promote_one(&mut lower_on_boundary, &request, atol).unwrap();
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
        let _ = promote_one(&mut positive, &request, ATol::default()).unwrap();

        let (mut negative, request) = fresh_instance(
            integer(-3.0, -1.0),
            Some(upper_row_id()),
            Some(lower_row_id()),
        );
        negative
            .constraint_collection
            .replace_active_row(upper_row_id(), upper_link(2.0, 1.0))
            .unwrap();
        let _ = promote_one(&mut negative, &request, ATol::default()).unwrap();
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
    fn rejects_exhausted_sos1_constraint_ids_without_mutation() {
        let (mut instance, request) = mixed_instance();
        instance
            .sos1_constraint_collection
            .insert_active_with_context(
                Sos1ConstraintID::from(u64::MAX),
                Sos1Constraint::new(BTreeSet::from([unrelated_id()])).unwrap(),
                ConstraintContext::default(),
            )
            .unwrap();

        assert_atomic_rejection(instance, &request, "Cannot allocate");
    }

    #[test]
    fn link_ids_must_name_active_regular_constraints() {
        let (instance, mut request) = mixed_instance();
        request.get_mut(&cardinality_row_id()).unwrap().insert(
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
        let empty_request = BTreeMap::from([(ConstraintID::from(0), BTreeMap::new())]);
        let before = empty.clone();
        assert!(promote_one(&mut empty, &empty_request, ATol::default())
            .unwrap_err()
            .to_string()
            .contains("at least one member"));
        assert_eq!(empty, before);

        let (instance, mut collision_request) = mixed_instance();
        collision_request
            .get_mut(&cardinality_row_id())
            .unwrap()
            .insert(
                member_integer_id(),
                Sos1BigMSelectorClaim::Fresh {
                    selector: member_binary_id(),
                    upper_link: Some(upper_row_id()),
                    lower_link: Some(lower_row_id()),
                },
            );
        assert_atomic_rejection(instance, &collision_request, "collides");

        let duplicate_member = VariableID::from(2);
        let (mut instance, mut duplicate_selector_request) = mixed_instance();
        instance
            .decision_variables
            .insert(
                duplicate_member,
                integer(-1.0, 1.0),
                Default::default(),
                None,
                ATol::default(),
            )
            .unwrap();
        duplicate_selector_request
            .get_mut(&cardinality_row_id())
            .unwrap()
            .insert(
                duplicate_member,
                Sos1BigMSelectorClaim::Fresh {
                    selector: selector_id(),
                    upper_link: None,
                    lower_link: None,
                },
            );
        assert_atomic_rejection(
            instance,
            &duplicate_selector_request,
            "assigned to more than one member",
        );

        let (instance, mut duplicate_row_request) = mixed_instance();
        let claims = duplicate_row_request.remove(&cardinality_row_id()).unwrap();
        duplicate_row_request.insert(upper_row_id(), claims);
        assert_atomic_rejection(instance, &duplicate_row_request, "also claimed");
    }

    #[test]
    fn preserves_selector_labels_and_relaxed_row_context_without_cloning_instance() {
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
        // A whole-Instance staging clone would duplicate these owned strings.
        // Their allocation identity guards the plan-first commit without
        // cloning the Instance.
        let selector_label_ptr = instance
            .variable_labels()
            .name(selector_id())
            .unwrap()
            .as_ptr();
        let link_label_ptr = instance
            .constraint_context()
            .name(upper_row_id())
            .unwrap()
            .as_ptr();
        let _ = promote_one(&mut instance, &request, ATol::default()).unwrap();

        assert_eq!(
            instance.variable_labels().name(selector_id()),
            Some("user_selector")
        );
        assert_eq!(
            instance.constraint_context().name(upper_row_id()),
            Some("user_link")
        );
        assert_eq!(
            instance
                .variable_labels()
                .name(selector_id())
                .unwrap()
                .as_ptr(),
            selector_label_ptr
        );
        assert_eq!(
            instance
                .constraint_context()
                .name(upper_row_id())
                .unwrap()
                .as_ptr(),
            link_label_ptr
        );
        assert!(instance.removed_constraints().contains_key(&upper_row_id()));
    }

    #[test]
    fn fresh_selector_isolation_rejects_retained_active_solver_usage() {
        let (base, request) = mixed_instance();

        let mut instance = base.clone();
        instance.set_objective(Function::from(linear!(10))).unwrap();
        assert_atomic_rejection(instance, &request, "active solver input");

        let mut instance = base.clone();
        instance
            .add_constraint(
                Constraint::less_than_or_equal_to_zero(Function::from(linear!(10))),
                Default::default(),
            )
            .unwrap();
        assert_atomic_rejection(instance, &request, "active solver input");

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
        assert_atomic_rejection(instance, &request, "active solver input");

        let mut instance = base.clone();
        instance
            .add_one_hot_constraint(
                OneHotConstraint::new(BTreeSet::from([selector_id()])).unwrap(),
                Default::default(),
            )
            .unwrap();
        assert_atomic_rejection(instance, &request, "active solver input");

        let mut instance = base.clone();
        instance
            .add_sos1_constraint(
                Sos1Constraint::new(BTreeSet::from([selector_id()])).unwrap(),
                Default::default(),
            )
            .unwrap();
        assert_atomic_rejection(instance, &request, "active solver input");

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
    fn non_solver_references_are_preserved_and_use_the_canonical_selector() {
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
            AcyclicAssignments::new([(unrelated_id(), Function::from(linear!(10)).abs())]).unwrap();
        let dependency_instructions = match instance
            .decision_variable_dependency
            .get(&unrelated_id())
            .unwrap()
        {
            Function::Expression(expression) => {
                crate::function::operation::instructions(expression).as_ptr()
            }
            _ => unreachable!("abs creates an Expression"),
        };
        let output_objective = super::super::OutputObjective::new(
            Sense::Minimize,
            (Function::from(linear!(10)) + Function::from(linear!(20))).unwrap(),
            false,
        );
        instance.output_objective = Some(output_objective.clone());

        let _ = promote_one(&mut instance, &request, ATol::default()).unwrap();

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
        assert_eq!(instance.output_objective(), Some(&output_objective));
        assert_eq!(instance.decision_variable_dependency().len(), 2);
        let retained_dependency_instructions = match instance
            .decision_variable_dependency
            .get(&unrelated_id())
            .unwrap()
        {
            Function::Expression(expression) => {
                crate::function::operation::instructions(expression).as_ptr()
            }
            _ => unreachable!("the existing Expression is preserved"),
        };
        assert_eq!(retained_dependency_instructions, dependency_instructions);
        assert_eq!(
            instance.decision_variable_role(selector_id()),
            Some(DecisionVariableRole::Dependent)
        );

        let solution = instance
            .evaluate(
                &crate::v1::State::from_iter([(0, 0.0), (1, -2.0)]),
                ATol::default(),
            )
            .unwrap();
        assert_eq!(solution.state().entries[&selector_id().into_inner()], 1.0);
        assert_eq!(solution.state().entries[&unrelated_id().into_inner()], 1.0);
        assert_eq!(*solution.objective(), 2.0);
    }

    #[test]
    fn dependent_selector_reconstruction_validates_input_state() {
        let (mut instance, request) = mixed_instance();
        let _ = promote_one(&mut instance, &request, ATol::default()).unwrap();

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

    fn mixed_v1_hint() -> crate::v1::Sos1 {
        crate::v1::Sos1 {
            binary_constraint_id: cardinality_row_id().into_inner(),
            // Legacy repeated fields do not encode request roles or ordering.
            big_m_constraint_ids: vec![lower_row_id().into_inner(), upper_row_id().into_inner()],
            decision_variables: vec![
                member_integer_id().into_inner(),
                member_binary_id().into_inner(),
            ],
        }
    }

    fn assert_v1_hint_atomic_rejection(
        mut instance: Instance,
        hint: &crate::v1::Sos1,
        expected: &str,
    ) {
        let before = instance.clone();
        let error = instance
            .sos1_big_m_promotion_request_from_v1_hint(hint)
            .and_then(|request| promote_one(&mut instance, &request, ATol::default()))
            .unwrap_err();
        assert!(
            error.to_string().contains(expected),
            "expected {expected:?} in error, got: {error:#}"
        );
        assert_eq!(instance, before);
    }

    #[test]
    fn legacy_v1_hint_uses_the_same_checked_history_preserving_promotion() {
        let (mut direct, request) = mixed_instance();
        let mut from_hint = direct.clone();

        let expected = promote_one(&mut direct, &request, ATol::default()).unwrap();
        let hint_request = from_hint
            .sos1_big_m_promotion_request_from_v1_hint(&mixed_v1_hint())
            .unwrap();
        let actual = promote_one(&mut from_hint, &hint_request, ATol::default()).unwrap();

        assert_eq!(actual, expected);
        assert_eq!(from_hint, direct);
        assert_eq!(
            &from_hint
                .removed_constraints()
                .keys()
                .copied()
                .collect::<BTreeSet<_>>(),
            &BTreeSet::from([upper_row_id(), lower_row_id(), cardinality_row_id()])
        );
    }

    #[test]
    fn legacy_v1_hint_accepts_all_reused_binary_members_without_big_m_rows() {
        let first = VariableID::from(0);
        let second = VariableID::from(2);
        let cardinality = ConstraintID::from(7);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([
                (first, DecisionVariable::binary()),
                (second, DecisionVariable::binary()),
            ]))
            .constraints(BTreeMap::from([(
                cardinality,
                canonical_sos1_big_m_cardinality([first, second]).unwrap(),
            )]))
            .build()
            .unwrap();
        let hint = crate::v1::Sos1 {
            binary_constraint_id: cardinality.into_inner(),
            big_m_constraint_ids: vec![],
            decision_variables: vec![second.into_inner(), first.into_inner()],
        };

        let request = instance
            .sos1_big_m_promotion_request_from_v1_hint(&hint)
            .unwrap();
        let promotion = promote_one(&mut instance, &request, ATol::default()).unwrap();

        assert_eq!(
            &instance.sos1_constraints()[&promotion].variables,
            &VariableIDSet::from([first, second])
        );
        assert!(instance.decision_variable_dependency().is_empty());
        assert_eq!(
            &instance
                .removed_constraints()
                .keys()
                .copied()
                .collect::<BTreeSet<_>>(),
            &BTreeSet::from([cardinality])
        );
    }

    #[test]
    fn legacy_v1_hint_infers_one_unlinked_fresh_selector_and_allows_id_zero() {
        let member = VariableID::from(1);
        let selector = VariableID::from(10);
        let cardinality = ConstraintID::from(0);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([
                (member, integer(0.0, 0.0)),
                (selector, DecisionVariable::binary()),
            ]))
            .constraints(BTreeMap::from([(
                cardinality,
                canonical_sos1_big_m_cardinality([selector]).unwrap(),
            )]))
            .build()
            .unwrap();
        let hint = crate::v1::Sos1 {
            binary_constraint_id: 0,
            big_m_constraint_ids: vec![],
            decision_variables: vec![member.into_inner()],
        };

        let request = instance
            .sos1_big_m_promotion_request_from_v1_hint(&hint)
            .unwrap();
        let promotion = promote_one(&mut instance, &request, ATol::default()).unwrap();

        assert!(instance.sos1_constraints().contains_key(&promotion));
        assert!(instance
            .decision_variable_dependency()
            .get(&selector)
            .is_some());
        assert_eq!(
            request[&cardinality][&member],
            Sos1BigMSelectorClaim::Fresh {
                selector,
                upper_link: None,
                lower_link: None
            }
        );
        assert_eq!(
            &instance
                .removed_constraints()
                .keys()
                .copied()
                .collect::<BTreeSet<_>>(),
            &BTreeSet::from([cardinality])
        );
    }

    #[test]
    fn legacy_v1_hint_rejects_missing_and_duplicate_ids_without_mutation() {
        let (instance, _) = mixed_instance();
        let mut hint = mixed_v1_hint();
        hint.decision_variables.clear();
        assert_v1_hint_atomic_rejection(instance, &hint, "at least one decision variable");

        let (instance, _) = mixed_instance();
        let mut hint = mixed_v1_hint();
        hint.decision_variables
            .push(member_integer_id().into_inner());
        assert_v1_hint_atomic_rejection(instance, &hint, "more than once");

        let (instance, _) = mixed_instance();
        let mut hint = mixed_v1_hint();
        hint.big_m_constraint_ids.push(upper_row_id().into_inner());
        assert_v1_hint_atomic_rejection(instance, &hint, "more than once");

        let (instance, _) = mixed_instance();
        let mut hint = mixed_v1_hint();
        hint.big_m_constraint_ids
            .push(cardinality_row_id().into_inner());
        assert_v1_hint_atomic_rejection(instance, &hint, "both cardinality and Big-M link");
    }

    #[test]
    fn legacy_v1_hint_rejects_stale_ids_without_mutation() {
        let (instance, _) = mixed_instance();
        let mut hint = mixed_v1_hint();
        hint.decision_variables[0] = 999;
        assert_v1_hint_atomic_rejection(instance, &hint, "is not registered");

        let (instance, _) = mixed_instance();
        let mut hint = mixed_v1_hint();
        hint.binary_constraint_id = 999;
        assert_v1_hint_atomic_rejection(instance, &hint, "is not active");

        let (instance, _) = mixed_instance();
        let mut hint = mixed_v1_hint();
        hint.big_m_constraint_ids[0] = 999;
        assert_v1_hint_atomic_rejection(instance, &hint, "is not active");

        let (mut instance, _) = mixed_instance();
        instance
            .relax_constraint(upper_row_id(), "test".to_string(), [])
            .unwrap();
        assert_v1_hint_atomic_rejection(instance, &mixed_v1_hint(), "is not active");
    }

    #[test]
    fn legacy_v1_hint_rejects_ambiguous_selector_roles_without_mutation() {
        let (mut instance, _) = mixed_instance();
        let binary_member_selector = VariableID::from(11);
        instance
            .decision_variables
            .insert(
                binary_member_selector,
                DecisionVariable::binary(),
                Default::default(),
                None,
                ATol::default(),
            )
            .unwrap();
        let binary_member_link = Constraint::less_than_or_equal_to_zero(Function::from(
            (Linear::single_term(LinearMonomial::Variable(member_binary_id()), coeff!(1.0))
                + Linear::single_term(
                    LinearMonomial::Variable(binary_member_selector),
                    coeff!(-1.0),
                ))
            .unwrap(),
        ));
        let binary_member_link_id = instance
            .add_constraint(binary_member_link, Default::default())
            .unwrap();
        let mut hint = mixed_v1_hint();
        hint.big_m_constraint_ids
            .push(binary_member_link_id.into_inner());
        assert_v1_hint_atomic_rejection(instance, &hint, "full-domain binary member");

        let (mut instance, _) = mixed_instance();
        let second_selector = VariableID::from(11);
        instance
            .decision_variables
            .insert(
                second_selector,
                DecisionVariable::binary(),
                Default::default(),
                None,
                ATol::default(),
            )
            .unwrap();
        let second_link = Constraint::less_than_or_equal_to_zero(Function::from(
            (Linear::single_term(LinearMonomial::Variable(member_integer_id()), coeff!(1.0))
                + Linear::single_term(LinearMonomial::Variable(second_selector), coeff!(-3.0)))
            .unwrap(),
        ));
        let second_link_id = instance
            .add_constraint(second_link, Default::default())
            .unwrap();
        let mut hint = mixed_v1_hint();
        hint.big_m_constraint_ids.push(second_link_id.into_inner());
        assert_v1_hint_atomic_rejection(instance, &hint, "different selectors");

        let (mut instance, _) = mixed_instance();
        let duplicate_upper = instance
            .add_constraint(upper_link(1.0, 3.0), Default::default())
            .unwrap();
        let mut hint = mixed_v1_hint();
        hint.big_m_constraint_ids.push(duplicate_upper.into_inner());
        assert_v1_hint_atomic_rejection(instance, &hint, "more than one upper link");

        let (mut instance, _) = mixed_instance();
        let malformed_support = Constraint::less_than_or_equal_to_zero(Function::from(
            ((Linear::single_term(LinearMonomial::Variable(member_integer_id()), coeff!(1.0))
                + Linear::single_term(LinearMonomial::Variable(selector_id()), coeff!(-3.0)))
            .unwrap()
                + Linear::single_term(LinearMonomial::Variable(unrelated_id()), coeff!(1.0)))
            .unwrap(),
        ));
        instance
            .constraint_collection
            .replace_active_row(upper_row_id(), malformed_support)
            .unwrap();
        assert_v1_hint_atomic_rejection(
            instance,
            &mixed_v1_hint(),
            "exactly one hinted member and one non-member selector",
        );

        let first_member = VariableID::from(1);
        let second_member = VariableID::from(2);
        let first_selector = VariableID::from(10);
        let second_selector = VariableID::from(11);
        let cardinality = ConstraintID::from(100);
        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([
                (first_member, integer(0.0, 0.0)),
                (second_member, integer(0.0, 0.0)),
                (first_selector, DecisionVariable::binary()),
                (second_selector, DecisionVariable::binary()),
            ]))
            .constraints(BTreeMap::from([(
                cardinality,
                canonical_sos1_big_m_cardinality([first_selector, second_selector]).unwrap(),
            )]))
            .build()
            .unwrap();
        let hint = crate::v1::Sos1 {
            binary_constraint_id: cardinality.into_inner(),
            big_m_constraint_ids: vec![],
            decision_variables: vec![first_member.into_inner(), second_member.into_inner()],
        };
        assert_v1_hint_atomic_rejection(instance, &hint, "is ambiguous");
    }

    #[test]
    fn legacy_v1_hint_delegates_incomplete_and_invalid_semantics_to_checker() {
        let (mut instance, _) = mixed_instance();
        let extra_selector = VariableID::from(11);
        instance
            .decision_variables
            .insert(
                extra_selector,
                DecisionVariable::binary(),
                Default::default(),
                None,
                ATol::default(),
            )
            .unwrap();
        instance
            .constraint_collection
            .replace_active_row(
                cardinality_row_id(),
                canonical_sos1_big_m_cardinality([
                    member_binary_id(),
                    selector_id(),
                    extra_selector,
                ])
                .unwrap(),
            )
            .unwrap();
        assert_v1_hint_atomic_rejection(instance, &mixed_v1_hint(), "is incomplete");

        let (instance, _) = mixed_instance();
        let mut hint = mixed_v1_hint();
        hint.big_m_constraint_ids
            .retain(|&id| id != lower_row_id().into_inner());
        assert_v1_hint_atomic_rejection(instance, &hint, "missing its required lower link");

        let (mut instance, _) = mixed_instance();
        instance
            .constraint_collection
            .replace_active_row(
                cardinality_row_id(),
                Constraint::less_than_or_equal_to_zero(Function::Zero),
            )
            .unwrap();
        assert_v1_hint_atomic_rejection(
            instance,
            &mixed_v1_hint(),
            "does not match the canonical row exactly",
        );

        let (mut instance, _) = mixed_instance();
        instance
            .constraint_collection
            .replace_active_row(upper_row_id(), upper_link(1.0, 1.0))
            .unwrap();
        assert_v1_hint_atomic_rejection(
            instance,
            &mixed_v1_hint(),
            "does not cover the ATol-feasible member domain",
        );
    }

    fn test_removed_reason() -> RemovedReason {
        RemovedReason {
            reason: "test".to_string(),
            parameters: Default::default(),
        }
    }
}
