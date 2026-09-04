//! Best-effort promotion of legacy v1 constraint hints during deserialization.
//!
//! Legacy hints are advisory, untrusted wire-format metadata. This module keeps
//! the ordinary v1 parser's ignore behavior unchanged and provides an explicit
//! alternate entry point that attempts every supported hint. A rejected hint
//! never prevents an otherwise valid [`Instance`] from being returned; callers
//! receive the original hint and its error in a structured report.

use super::{
    one_hot_promotion::validate_one_hot_promotion_source, Instance, OneHotPromotionRequest,
    Sos1BigMPromotion, Sos1BigMPromotionRequest,
};
use crate::{message_io, v1, ATol, ConstraintID, OneHotConstraintID, Parse};

/// Outcome of attempting one legacy v1 one-hot hint.
///
/// The `index` in each variant is the hint's original position in
/// [`v1::ConstraintHints::one_hot_constraints`].
#[non_exhaustive]
#[derive(Debug)]
pub enum V1OneHotHintPromotionOutcome {
    /// The hint was verified against the decoded instance and promoted.
    Promoted {
        /// Original index within the one-hot hint family.
        index: usize,
        /// Original untrusted wire-format hint.
        hint: v1::OneHot,
        /// ID allocated to the promoted OneHot constraint.
        one_hot_constraint_id: OneHotConstraintID,
    },
    /// The hint could not be verified against its source row.
    Rejected {
        /// Original index within the one-hot hint family.
        index: usize,
        /// Original untrusted wire-format hint.
        hint: v1::OneHot,
        /// Validation error.
        error: crate::Error,
    },
}

impl V1OneHotHintPromotionOutcome {
    /// Original index within [`v1::ConstraintHints::one_hot_constraints`].
    pub fn index(&self) -> usize {
        match self {
            Self::Promoted { index, .. } | Self::Rejected { index, .. } => *index,
        }
    }

    /// Original untrusted wire-format hint.
    pub fn hint(&self) -> &v1::OneHot {
        match self {
            Self::Promoted { hint, .. } | Self::Rejected { hint, .. } => hint,
        }
    }

    /// Allocated OneHot constraint ID, or `None` when the hint was rejected.
    pub fn one_hot_constraint_id(&self) -> Option<OneHotConstraintID> {
        match self {
            Self::Promoted {
                one_hot_constraint_id,
                ..
            } => Some(*one_hot_constraint_id),
            Self::Rejected { .. } => None,
        }
    }

    /// Rejection error, or `None` when the hint was promoted.
    pub fn error(&self) -> Option<&crate::Error> {
        match self {
            Self::Promoted { .. } => None,
            Self::Rejected { error, .. } => Some(error),
        }
    }

    /// Return whether the hint was promoted.
    pub fn is_promoted(&self) -> bool {
        matches!(self, Self::Promoted { .. })
    }
}

/// Outcome of attempting one legacy v1 SOS1 hint.
///
/// The `index` in each variant is the hint's original position in
/// [`v1::ConstraintHints::sos1_constraints`].
#[non_exhaustive]
#[derive(Debug)]
pub enum V1Sos1HintPromotionOutcome {
    /// The hint was verified against the decoded instance and promoted.
    Promoted {
        /// Original index within the SOS1 hint family.
        index: usize,
        /// Original untrusted wire-format hint.
        hint: v1::Sos1,
        /// Verified promotion result.
        promotion: Sos1BigMPromotion,
    },
    /// The hint could not be verified or conflicted with another valid hint.
    Rejected {
        /// Original index within the SOS1 hint family.
        index: usize,
        /// Original untrusted wire-format hint.
        hint: v1::Sos1,
        /// Validation or conflict error.
        error: crate::Error,
    },
}

impl V1Sos1HintPromotionOutcome {
    /// Original index within [`v1::ConstraintHints::sos1_constraints`].
    pub fn index(&self) -> usize {
        match self {
            Self::Promoted { index, .. } | Self::Rejected { index, .. } => *index,
        }
    }

    /// Original untrusted wire-format hint.
    pub fn hint(&self) -> &v1::Sos1 {
        match self {
            Self::Promoted { hint, .. } | Self::Rejected { hint, .. } => hint,
        }
    }

    /// Verified promotion result, or `None` when the hint was rejected.
    pub fn promotion(&self) -> Option<&Sos1BigMPromotion> {
        match self {
            Self::Promoted { promotion, .. } => Some(promotion),
            Self::Rejected { .. } => None,
        }
    }

    /// Rejection error, or `None` when the hint was promoted.
    pub fn error(&self) -> Option<&crate::Error> {
        match self {
            Self::Promoted { .. } => None,
            Self::Rejected { error, .. } => Some(error),
        }
    }

    /// Return whether the hint was promoted.
    pub fn is_promoted(&self) -> bool {
        matches!(self, Self::Promoted { .. })
    }
}

/// Structured outcomes from best-effort promotion of legacy v1 hints.
///
/// Each family preserves the order of its corresponding repeated protobuf
/// field. A report can contain both promoted and rejected hints. Rejections are
/// data, not failures of the surrounding byte-deserialization operation.
#[must_use = "inspect the report for rejected legacy constraint hints"]
#[derive(Debug)]
pub struct V1ConstraintHintPromotionReport {
    one_hot_outcomes: Vec<V1OneHotHintPromotionOutcome>,
    sos1_outcomes: Vec<V1Sos1HintPromotionOutcome>,
}

impl V1ConstraintHintPromotionReport {
    /// Outcomes in original one-hot hint order.
    pub fn one_hot_outcomes(&self) -> &[V1OneHotHintPromotionOutcome] {
        &self.one_hot_outcomes
    }

    /// Outcomes in original SOS1 hint order.
    pub fn sos1_outcomes(&self) -> &[V1Sos1HintPromotionOutcome] {
        &self.sos1_outcomes
    }

    /// Return whether at least one hint was rejected.
    pub fn has_rejections(&self) -> bool {
        self.one_hot_outcomes
            .iter()
            .any(|outcome| !outcome.is_promoted())
            || self
                .sos1_outcomes
                .iter()
                .any(|outcome| !outcome.is_promoted())
    }

    /// Consume the report and return both outcome families in wire order.
    ///
    /// Unlike the borrowed accessors, this transfers ownership of rejection
    /// errors to the caller, preserving their complete error chains for
    /// downcasting or propagation.
    pub fn into_parts(
        self,
    ) -> (
        Vec<V1OneHotHintPromotionOutcome>,
        Vec<V1Sos1HintPromotionOutcome>,
    ) {
        (self.one_hot_outcomes, self.sos1_outcomes)
    }
}

#[derive(Debug)]
enum PreparedOneHotHint {
    Promotable {
        index: usize,
        hint: v1::OneHot,
        source_constraint_id: ConstraintID,
    },
    Rejected {
        index: usize,
        hint: v1::OneHot,
        error: crate::Error,
    },
}

#[derive(Debug)]
enum PreparedSos1Hint {
    Promotable {
        index: usize,
        hint: v1::Sos1,
    },
    Rejected {
        index: usize,
        hint: v1::Sos1,
        error: crate::Error,
    },
}

/// Instance-bound plan for promoting both legacy hint families.
///
/// # Invariants
///
/// - `instance` is the exact decoded v1 [`Instance`] against which every
///   `PreparedOneHotHint::Promotable` source was validated and every SOS1
///   request was reconstructed; its exclusive borrow prevents intervening
///   mutation before Apply;
/// - `one_hot_request` contains exactly the distinct source IDs of the
///   promotable OneHot hints, and every such hint retains the same ID for report
///   lookup;
/// - v1 parsing initializes both active and removed OneHot collections empty,
///   so all validated sources fit in that independent target-ID namespace and
///   [`Instance::promote_one_hot`] returns one successful entry for every ID in
///   `one_hot_request`;
/// - `sos1_requests` has exactly one entry, in order, for every
///   `PreparedSos1Hint::Promotable`; and
/// - Apply invokes only the family owner APIs. OneHot promotion runs first, and
///   the SOS1 batch is then checked and applied against that resulting bound
///   instance, so no detached family effect can become stale.
///
/// Consequently, all caller-controlled hint failures are report data. Every
/// `expect` in Apply asserts one of the cardinality, key-alignment, or empty
/// OneHot-namespace invariants above; reaching one means this private plan's
/// construction contract was broken.
#[derive(Debug)]
struct V1ConstraintHintPromotionPlan<'a> {
    instance: &'a mut Instance,
    one_hot_request: OneHotPromotionRequest,
    sos1_requests: Vec<Sos1BigMPromotionRequest>,
    one_hot_hints: Vec<PreparedOneHotHint>,
    sos1_hints: Vec<PreparedSos1Hint>,
    atol: ATol,
}

impl<'a> V1ConstraintHintPromotionPlan<'a> {
    fn prepare(instance: &'a mut Instance, mut hints: v1::ConstraintHints, atol: ATol) -> Self {
        let mut one_hot_request = OneHotPromotionRequest::new();
        let one_hot_hints = std::mem::take(&mut hints.one_hot_constraints)
            .into_iter()
            .enumerate()
            .map(|(index, hint)| {
                let source_constraint_id = ConstraintID::from(hint.constraint_id);
                let request = OneHotPromotionRequest::from(&hint);
                match validate_one_hot_promotion_source(instance, source_constraint_id) {
                    Ok(()) => {
                        one_hot_request.extend(request);
                        PreparedOneHotHint::Promotable {
                            index,
                            hint,
                            source_constraint_id,
                        }
                    }
                    Err(error) => PreparedOneHotHint::Rejected { index, hint, error },
                }
            })
            .collect();

        let mut sos1_requests = Vec::new();
        let sos1_hints = std::mem::take(&mut hints.sos1_constraints)
            .into_iter()
            .enumerate()
            .map(
                |(index, hint)| match Sos1BigMPromotionRequest::from_v1_hint(instance, &hint) {
                    Ok(request) => {
                        sos1_requests.push(request);
                        PreparedSos1Hint::Promotable { index, hint }
                    }
                    Err(error) => PreparedSos1Hint::Rejected { index, hint, error },
                },
            )
            .collect();

        Self {
            instance,
            one_hot_request,
            sos1_requests,
            one_hot_hints,
            sos1_hints,
            atol,
        }
    }

    fn apply(self) -> V1ConstraintHintPromotionReport {
        let Self {
            instance,
            one_hot_request,
            sos1_requests,
            one_hot_hints,
            sos1_hints,
            atol,
        } = self;

        let one_hot_promotions = instance
            .promote_one_hot(&one_hot_request)
            .into_iter()
            .map(|(source_constraint_id, result)| {
                let target_id = result.expect(
                    "validated v1 OneHot sources must fit in the parsed instance's empty target namespace",
                );
                (source_constraint_id, target_id)
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let sos1_promotions = instance.promote_sos1_big_m_batch(&sos1_requests, atol);

        let one_hot_outcomes = one_hot_hints
            .into_iter()
            .map(|prepared| match prepared {
                PreparedOneHotHint::Promotable {
                    index,
                    hint,
                    source_constraint_id,
                } => V1OneHotHintPromotionOutcome::Promoted {
                    index,
                    hint,
                    one_hot_constraint_id: *one_hot_promotions.get(&source_constraint_id).expect(
                        "every promotable v1 OneHot hint must retain its requested source ID",
                    ),
                },
                PreparedOneHotHint::Rejected { index, hint, error } => {
                    V1OneHotHintPromotionOutcome::Rejected { index, hint, error }
                }
            })
            .collect();

        let mut sos1_promotions = sos1_promotions.into_iter();
        let sos1_outcomes = sos1_hints
            .into_iter()
            .map(|prepared| match prepared {
                PreparedSos1Hint::Promotable { index, hint } => {
                    match sos1_promotions
                        .next()
                        .expect("every converted v1 SOS1 hint must have one aligned result")
                    {
                        Ok(promotion) => V1Sos1HintPromotionOutcome::Promoted {
                            index,
                            hint,
                            promotion,
                        },
                        Err(error) => V1Sos1HintPromotionOutcome::Rejected { index, hint, error },
                    }
                }
                PreparedSos1Hint::Rejected { index, hint, error } => {
                    V1Sos1HintPromotionOutcome::Rejected { index, hint, error }
                }
            })
            .collect();
        debug_assert!(sos1_promotions.next().is_none());

        V1ConstraintHintPromotionReport {
            one_hot_outcomes,
            sos1_outcomes,
        }
    }
}

impl Instance {
    /// Deserialize a v1 instance and attempt every legacy constraint hint.
    ///
    /// This is an explicit best-effort alternative to [`Instance::from_v1_bytes`].
    /// The ordinary parser continues to ignore all legacy hints. This method
    /// first parses the same base [`Instance`] and reconstructs requests from
    /// its regular rows. Invalid hints are retained as rejected outcomes rather
    /// than causing the byte parse to fail.
    ///
    /// Repeated valid OneHot hints for the same source ID are one promotion
    /// request, and every raw occurrence reports the same allocated target ID.
    /// The legacy member list is advisory and ignored. SOS1 outcomes retain
    /// their raw-hint order, including the family's all-participants rejection
    /// policy for incompatible otherwise-valid requests. OneHot promotion is
    /// completed first. The converted SOS1 requests are
    /// then batch-checked and applied against that resulting instance, so each
    /// family owner validates the exact state that it mutates. One private
    /// instance-bound plan retains mutation authority across that sequence.
    ///
    /// The supplied `atol` is used only to verify SOS1 Big-M formulations.
    /// OneHot recognition is exact and, although it preserves the exact
    /// feasible set over binary assignments, does not promise identical
    /// approximate-feasibility classification at a nonzero tolerance.
    ///
    /// # Errors
    ///
    /// Returns an error only when the bytes cannot be decoded or the underlying
    /// v1 message cannot be parsed as an [`Instance`]. Hint-specific failures
    /// are returned in the [`V1ConstraintHintPromotionReport`].
    pub fn from_v1_bytes_with_promotion(
        bytes: &[u8],
        atol: ATol,
    ) -> crate::Result<(Self, V1ConstraintHintPromotionReport)> {
        let mut raw = message_io::decode::<v1::Instance>(bytes, "ommx.v1.Instance")?;
        let hints = raw.constraint_hints.take().unwrap_or_default();
        let mut instance = Parse::parse(raw, &())?;
        let report = V1ConstraintHintPromotionPlan::prepare(&mut instance, hints, atol).apply();
        Ok((instance, report))
    }
}
