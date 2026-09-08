//! Best-effort promotion of legacy v1 constraint hints during deserialization.
//!
//! Legacy hints are advisory, untrusted wire-format metadata. This module keeps
//! the ordinary v1 parser's ignore behavior unchanged and provides an explicit
//! alternate entry point that attempts every supported hint. A rejected hint
//! never prevents an otherwise valid [`Instance`] from being returned; callers
//! receive the original hint and its error in a structured report.

use super::{Instance, OneHotPromotionRequest, Sos1BigMPromotionRequest};
use crate::{message_io, v1, ATol, ConstraintID, OneHotConstraintID, Parse, Sos1ConstraintID};
use std::{
    collections::{btree_map::Entry, BTreeMap, BTreeSet},
    sync::Arc,
};

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
        /// Shared validation error; duplicate hints retain the same error chain.
        error: Arc<crate::Error>,
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
        /// ID allocated to the promoted SOS1 constraint.
        sos1_constraint_id: Sos1ConstraintID,
    },
    /// The hint could not be verified or conflicted with another converted
    /// claim for the same cardinality constraint.
    Rejected {
        /// Original index within the SOS1 hint family.
        index: usize,
        /// Original untrusted wire-format hint.
        hint: v1::Sos1,
        /// Original conversion, validation, or conflict error chain.
        /// Duplicate normalized claims share validation and conflict errors.
        error: Arc<crate::Error>,
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

    /// Allocated SOS1 constraint ID, or `None` when the hint was rejected.
    pub fn sos1_constraint_id(&self) -> Option<Sos1ConstraintID> {
        match self {
            Self::Promoted {
                sos1_constraint_id, ..
            } => Some(*sos1_constraint_id),
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
/// Hints that normalize to the same request share their result, including
/// rejection errors.
///
/// # Invariants
///
/// Created only by [`Instance::from_v1_bytes_with_promotion`]. Each family has
/// exactly one outcome per raw hint, in wire order, with its original index and
/// message unchanged. Each outcome contains either one target ID or one error.
/// Target IDs identify promotions applied to the returned instance. Duplicate
/// normalized requests share one target ID or one original error chain.
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
    /// This transfers shared ownership of rejection errors to the caller.
    /// Their complete error chains remain available for downcasting through
    /// the `Arc`; duplicate normalized requests share the same error allocation.
    pub fn into_parts(
        self,
    ) -> (
        Vec<V1OneHotHintPromotionOutcome>,
        Vec<V1Sos1HintPromotionOutcome>,
    ) {
        (self.one_hot_outcomes, self.sos1_outcomes)
    }
}

/// Each variant retains the original wire index and unmodified hint.
/// Before outcomes are assembled, every converted hint's cardinality ID has
/// exactly one result from either the SOS1 batch or the conflicting-ID set.
/// Rejected conversions carry their own error and require no result lookup.
#[derive(Debug)]
enum PreparedSos1Hint {
    Converted {
        index: usize,
        hint: v1::Sos1,
    },
    Rejected {
        index: usize,
        hint: v1::Sos1,
        error: Arc<crate::Error>,
    },
}

impl Instance {
    /// Sequence family-owned promotion operations without introducing a second
    /// Plan/Apply abstraction. Each family constructs its own validated plan
    /// against the exact instance it mutates; only planning rejections enter
    /// this report. The map lookups below assert the family APIs' exact-key
    /// result contracts and the converted-hint invariant above.
    fn promote_v1_constraint_hints(
        &mut self,
        hints: v1::ConstraintHints,
        atol: ATol,
    ) -> V1ConstraintHintPromotionReport {
        let one_hot_request = hints
            .one_hot_constraints
            .iter()
            .flat_map(OneHotPromotionRequest::from)
            .collect::<OneHotPromotionRequest>();

        // Reconstruct while every original regular row is still available.
        // Normalization ignores repeated-field order, but never silently
        // overwrites different claims for the same cardinality constraint.
        let mut sos1_request = Sos1BigMPromotionRequest::new();
        let mut conflicting_ids = BTreeSet::new();
        let sos1_hints = hints
            .sos1_constraints
            .into_iter()
            .enumerate()
            .map(
                |(index, hint)| match self.sos1_big_m_promotion_request_from_v1_hint(&hint) {
                    Ok(mut request) => {
                        let (cardinality, claims) = request
                            .pop_first()
                            .expect("v1 SOS1 conversion returns exactly one request");
                        debug_assert!(request.is_empty());
                        debug_assert_eq!(
                            cardinality,
                            ConstraintID::from(hint.binary_constraint_id)
                        );
                        match sos1_request.entry(cardinality) {
                            Entry::Vacant(entry) => {
                                entry.insert(claims);
                            }
                            Entry::Occupied(entry) if entry.get() != &claims => {
                                conflicting_ids.insert(cardinality);
                            }
                            Entry::Occupied(_) => {}
                        }
                        PreparedSos1Hint::Converted { index, hint }
                    }
                    Err(error) => PreparedSos1Hint::Rejected {
                        index,
                        hint,
                        error: Arc::new(error),
                    },
                },
            )
            .collect::<Vec<_>>();
        for id in &conflicting_ids {
            sos1_request.remove(id);
        }

        // Neither family API can add a rejection during Apply. Their Err
        // entries were determined by their own plans before storage effects.
        let one_hot_promotions = self
            .promote_one_hot(&one_hot_request)
            .into_iter()
            .map(|(id, result)| (id, result.map_err(Arc::new)))
            .collect::<BTreeMap<_, _>>();
        let mut sos1_promotions = self
            .promote_sos1_big_m(&sos1_request, atol)
            .into_iter()
            .map(|(id, result)| (id, result.map_err(Arc::new)))
            .collect::<BTreeMap<_, _>>();
        for cardinality in conflicting_ids {
            sos1_promotions.insert(cardinality, Err(Arc::new(crate::error!(
                { ?cardinality },
                "Legacy v1 SOS1 hints contain different selector claims for cardinality constraint {cardinality}"
            ))));
        }

        let one_hot_outcomes = hints
            .one_hot_constraints
            .into_iter()
            .enumerate()
            .map(|(index, hint)| {
                let result = one_hot_promotions
                    .get(&ConstraintID::from(hint.constraint_id))
                    .expect("OneHot promotion reports exactly the requested source IDs");
                match result {
                    Ok(one_hot_constraint_id) => V1OneHotHintPromotionOutcome::Promoted {
                        index,
                        hint,
                        one_hot_constraint_id: *one_hot_constraint_id,
                    },
                    Err(error) => V1OneHotHintPromotionOutcome::Rejected {
                        index,
                        hint,
                        error: Arc::clone(error),
                    },
                }
            })
            .collect();
        let sos1_outcomes = sos1_hints
            .into_iter()
            .map(|prepared| match prepared {
                PreparedSos1Hint::Converted { index, hint } => {
                    let result = sos1_promotions
                        .get(&ConstraintID::from(hint.binary_constraint_id))
                        .expect("every converted SOS1 hint has a promotion or conflict outcome");
                    match result {
                        Ok(sos1_constraint_id) => V1Sos1HintPromotionOutcome::Promoted {
                            index,
                            hint,
                            sos1_constraint_id: *sos1_constraint_id,
                        },
                        Err(error) => V1Sos1HintPromotionOutcome::Rejected {
                            index,
                            hint,
                            error: Arc::clone(error),
                        },
                    }
                }
                PreparedSos1Hint::Rejected { index, hint, error } => {
                    V1Sos1HintPromotionOutcome::Rejected { index, hint, error }
                }
            })
            .collect();

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
    /// their raw-hint order. Equivalent converted SOS1 requests with the same
    /// cardinality ID are promoted once; different claims for that ID are all
    /// rejected without entering the SOS1 batch. Conversion failures do not
    /// block other hints. Duplicate normalized requests share the original
    /// rejection error chain.
    /// OneHot promotion completes first, then the SOS1 owner batch-checks and
    /// applies against that resulting instance. Each family owner validates
    /// the exact state it mutates; this loader does not duplicate its Plan or
    /// weaken its infallible Apply contract.
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
        let report = instance.promote_v1_constraint_hints(hints, atol);
        Ok((instance, report))
    }
}
