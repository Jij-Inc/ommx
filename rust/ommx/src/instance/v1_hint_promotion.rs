//! Best-effort promotion of legacy v1 constraint hints during deserialization.
//!
//! Legacy hints are advisory, untrusted wire-format metadata. This module keeps
//! the ordinary v1 parser's ignore behavior unchanged and provides an explicit
//! alternate entry point that attempts every supported hint. A rejected hint
//! never prevents an otherwise valid [`Instance`] from being returned; callers
//! receive the original hint and its error in a structured report.

use super::{
    Instance, OneHotPromotion, OneHotPromotionRequest, Sos1BigMPromotion, Sos1BigMPromotionRequest,
    Sos1BigMSelectorClaim,
};
use crate::{message_io, v1, ATol, ConstraintID, Parse};
use std::collections::{BTreeMap, BTreeSet};

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
        /// Verified promotion result.
        promotion: OneHotPromotion,
    },
    /// The hint could not be verified or conflicted with another valid hint.
    Rejected {
        /// Original index within the one-hot hint family.
        index: usize,
        /// Original untrusted wire-format hint.
        hint: v1::OneHot,
        /// Validation or conflict error.
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

    /// Verified promotion result, or `None` when the hint was rejected.
    pub fn promotion(&self) -> Option<&OneHotPromotion> {
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum CandidateKey {
    OneHot(usize),
    Sos1(usize),
}

impl CandidateKey {
    fn family_name(self) -> &'static str {
        match self {
            Self::OneHot(_) => "one-hot",
            Self::Sos1(_) => "SOS1",
        }
    }

    fn index(self) -> usize {
        match self {
            Self::OneHot(index) | Self::Sos1(index) => index,
        }
    }
}

enum PromotionCandidate {
    OneHot {
        index: usize,
        hint: v1::OneHot,
        request: OneHotPromotionRequest,
    },
    Sos1 {
        index: usize,
        hint: v1::Sos1,
        request: Sos1BigMPromotionRequest,
    },
}

impl PromotionCandidate {
    fn key(&self) -> CandidateKey {
        match self {
            Self::OneHot { index, .. } => CandidateKey::OneHot(*index),
            Self::Sos1 { index, .. } => CandidateKey::Sos1(*index),
        }
    }

    fn regular_constraint_ids(&self) -> BTreeSet<ConstraintID> {
        match self {
            Self::OneHot { request, .. } => BTreeSet::from([request.source_constraint_id]),
            Self::Sos1 { request, .. } => {
                let mut ids = BTreeSet::from([request.cardinality_constraint]);
                for claim in request.selector_claims.values() {
                    if let Sos1BigMSelectorClaim::Fresh {
                        upper_link,
                        lower_link,
                        ..
                    } = claim
                    {
                        ids.extend(upper_link.iter().copied());
                        ids.extend(lower_link.iter().copied());
                    }
                }
                ids
            }
        }
    }
}

fn conflicting_regular_rows_error(
    key: CandidateKey,
    regular_constraint_ids: BTreeSet<ConstraintID>,
) -> crate::Error {
    crate::error!(
        {
            family = key.family_name(),
            index = key.index(),
            ?regular_constraint_ids
        },
        "Legacy v1 constraint hint conflicts with another individually valid hint over consumed regular rows {regular_constraint_ids:?}"
    )
}

impl Instance {
    /// Deserialize a v1 instance and attempt every legacy constraint hint.
    ///
    /// This is an explicit best-effort alternative to [`Instance::from_v1_bytes`].
    /// The ordinary parser continues to ignore all legacy hints. This method
    /// first parses the same base [`Instance`], then checks every one-hot and
    /// SOS1 hint independently against that original instance. Invalid hints
    /// are retained as rejected outcomes rather than causing the byte parse to
    /// fail.
    ///
    /// Individually valid candidates that claim the same consumed regular
    /// constraint ID are all rejected. This avoids choosing a winner based on
    /// input order. All remaining one-hot candidates are applied in their
    /// original order, followed by all remaining SOS1 candidates in their
    /// original order. Every individual promotion remains atomic and
    /// history-preserving.
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
        let mut hints = raw.constraint_hints.take().unwrap_or_default();
        let mut instance = Parse::parse(raw, &())?;

        let one_hot_hints = std::mem::take(&mut hints.one_hot_constraints);
        let sos1_hints = std::mem::take(&mut hints.sos1_constraints);
        let mut one_hot_outcomes: Vec<Option<V1OneHotHintPromotionOutcome>> =
            std::iter::repeat_with(|| None)
                .take(one_hot_hints.len())
                .collect();
        let mut sos1_outcomes: Vec<Option<V1Sos1HintPromotionOutcome>> =
            std::iter::repeat_with(|| None)
                .take(sos1_hints.len())
                .collect();
        let mut candidates = Vec::new();

        // A malformed or stale hint must not make a valid hint look
        // conflicting. Only requests that can complete against an untouched
        // clone of the original instance enter the conflict graph.
        for (index, hint) in one_hot_hints.into_iter().enumerate() {
            let request = match OneHotPromotionRequest::from_v1_hint(&hint) {
                Ok(request) => request,
                Err(error) => {
                    one_hot_outcomes[index] =
                        Some(V1OneHotHintPromotionOutcome::Rejected { index, hint, error });
                    continue;
                }
            };
            let mut trial = instance.clone();
            if let Err(error) = trial.promote_one_hot(&request) {
                one_hot_outcomes[index] =
                    Some(V1OneHotHintPromotionOutcome::Rejected { index, hint, error });
                continue;
            }
            candidates.push(PromotionCandidate::OneHot {
                index,
                hint,
                request,
            });
        }

        for (index, hint) in sos1_hints.into_iter().enumerate() {
            let request = match Sos1BigMPromotionRequest::from_v1_hint(&instance, &hint) {
                Ok(request) => request,
                Err(error) => {
                    sos1_outcomes[index] =
                        Some(V1Sos1HintPromotionOutcome::Rejected { index, hint, error });
                    continue;
                }
            };
            let mut trial = instance.clone();
            if let Err(error) = trial.promote_sos1_big_m(&request, atol) {
                sos1_outcomes[index] =
                    Some(V1Sos1HintPromotionOutcome::Rejected { index, hint, error });
                continue;
            }
            candidates.push(PromotionCandidate::Sos1 {
                index,
                hint,
                request,
            });
        }

        let mut regular_claimants: BTreeMap<ConstraintID, Vec<CandidateKey>> = BTreeMap::new();
        for candidate in &candidates {
            let key = candidate.key();
            for id in candidate.regular_constraint_ids() {
                regular_claimants.entry(id).or_default().push(key);
            }
        }

        let mut conflicts: BTreeMap<CandidateKey, BTreeSet<ConstraintID>> = BTreeMap::new();
        for (id, claimants) in regular_claimants {
            if claimants.len() > 1 {
                for claimant in claimants {
                    conflicts.entry(claimant).or_default().insert(id);
                }
            }
        }

        for candidate in candidates {
            let key = candidate.key();
            if let Some(conflicts) = conflicts.remove(&key) {
                let error = conflicting_regular_rows_error(key, conflicts);
                match candidate {
                    PromotionCandidate::OneHot { index, hint, .. } => {
                        one_hot_outcomes[index] =
                            Some(V1OneHotHintPromotionOutcome::Rejected { index, hint, error });
                    }
                    PromotionCandidate::Sos1 { index, hint, .. } => {
                        sos1_outcomes[index] =
                            Some(V1Sos1HintPromotionOutcome::Rejected { index, hint, error });
                    }
                }
                continue;
            }

            match candidate {
                PromotionCandidate::OneHot {
                    index,
                    hint,
                    request,
                } => {
                    one_hot_outcomes[index] = Some(match instance.promote_one_hot(&request) {
                        Ok(promotion) => V1OneHotHintPromotionOutcome::Promoted {
                            index,
                            hint,
                            promotion,
                        },
                        Err(error) => V1OneHotHintPromotionOutcome::Rejected { index, hint, error },
                    });
                }
                PromotionCandidate::Sos1 {
                    index,
                    hint,
                    request,
                } => {
                    sos1_outcomes[index] =
                        Some(match instance.promote_sos1_big_m(&request, atol) {
                            Ok(promotion) => V1Sos1HintPromotionOutcome::Promoted {
                                index,
                                hint,
                                promotion,
                            },
                            Err(error) => {
                                V1Sos1HintPromotionOutcome::Rejected { index, hint, error }
                            }
                        });
                }
            }
        }

        let one_hot_outcomes = one_hot_outcomes
            .into_iter()
            .map(|outcome| outcome.expect("every one-hot hint must receive one outcome"))
            .collect();
        let sos1_outcomes = sos1_outcomes
            .into_iter()
            .map(|outcome| outcome.expect("every SOS1 hint must receive one outcome"))
            .collect();

        Ok((
            instance,
            V1ConstraintHintPromotionReport {
                one_hot_outcomes,
                sos1_outcomes,
            },
        ))
    }
}
