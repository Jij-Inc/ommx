use crate::{error::OmmxPyResult, Instance};
use pyo3::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

/// Unchecked selector-role claim for one member of an SOS1 Big-M formulation.
///
/// Construct claims with {meth}`reused` or {meth}`fresh`. The claim contains
/// stable IDs only; {meth}`~ommx.Instance.promote_sos1_big_m` validates the
/// current variable domains and regular-constraint rows before mutating the
/// instance.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sos1BigMSelectorClaim {
    inner: ommx::Sos1BigMSelectorClaim,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Sos1BigMSelectorClaim {
    /// Claim that the promoted member is itself a full-domain Binary selector.
    #[staticmethod]
    pub fn reused() -> Self {
        Self {
            inner: ommx::Sos1BigMSelectorClaim::Reused,
        }
    }

    /// Claim a separate private Binary selector and its optional Big-M links.
    ///
    /// A link may be omitted only when the current member domain makes that
    /// side redundant. Supplied links are always validated by the Rust
    /// {class}`Instance` owner.
    #[staticmethod]
    #[pyo3(signature = (selector, *, upper_link=None, lower_link=None))]
    pub fn fresh(selector: u64, upper_link: Option<u64>, lower_link: Option<u64>) -> Self {
        Self {
            inner: ommx::Sos1BigMSelectorClaim::Fresh {
                selector: selector.into(),
                upper_link: upper_link.map(Into::into),
                lower_link: lower_link.map(Into::into),
            },
        }
    }

    /// Whether this claim reuses the member itself as its selector.
    #[getter]
    pub fn is_reused(&self) -> bool {
        matches!(self.inner, ommx::Sos1BigMSelectorClaim::Reused)
    }

    /// Claimed fresh selector ID, or ``None`` for a reused selector.
    #[getter]
    pub fn selector(&self) -> Option<u64> {
        match self.inner {
            ommx::Sos1BigMSelectorClaim::Reused => None,
            ommx::Sos1BigMSelectorClaim::Fresh { selector, .. } => Some(selector.into_inner()),
        }
    }

    /// Claimed upper-link constraint ID, if supplied.
    #[getter]
    pub fn upper_link(&self) -> Option<u64> {
        match self.inner {
            ommx::Sos1BigMSelectorClaim::Reused => None,
            ommx::Sos1BigMSelectorClaim::Fresh { upper_link, .. } => {
                upper_link.map(|id| id.into_inner())
            }
        }
    }

    /// Claimed lower-link constraint ID, if supplied.
    #[getter]
    pub fn lower_link(&self) -> Option<u64> {
        match self.inner {
            ommx::Sos1BigMSelectorClaim::Reused => None,
            ommx::Sos1BigMSelectorClaim::Fresh { lower_link, .. } => {
                lower_link.map(|id| id.into_inner())
            }
        }
    }
}

impl From<ommx::Sos1BigMSelectorClaim> for Sos1BigMSelectorClaim {
    fn from(inner: ommx::Sos1BigMSelectorClaim) -> Self {
        Self { inner }
    }
}

/// Untrusted stable-ID request for one checked SOS1 Big-M promotion.
///
/// Map keys are the intended SOS1 member IDs. Bounds, kinds, coefficients, and
/// row contents are intentionally absent so the current {class}`Instance`
/// remains the sole source of truth when the request is validated.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sos1BigMPromotionRequest {
    inner: ommx::Sos1BigMPromotionRequest,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Sos1BigMPromotionRequest {
    #[new]
    #[pyo3(signature = (*, selector_claims, cardinality_constraint))]
    pub fn new(
        selector_claims: BTreeMap<u64, Sos1BigMSelectorClaim>,
        cardinality_constraint: u64,
    ) -> Self {
        Self {
            inner: ommx::Sos1BigMPromotionRequest {
                selector_claims: selector_claims
                    .into_iter()
                    .map(|(member, claim)| (member.into(), claim.inner))
                    .collect(),
                cardinality_constraint: cardinality_constraint.into(),
            },
        }
    }

    /// Claimed selector roles keyed by intended SOS1 member ID.
    #[getter]
    pub fn selector_claims(&self) -> BTreeMap<u64, Sos1BigMSelectorClaim> {
        self.inner
            .selector_claims
            .iter()
            .map(|(&member, &claim)| (member.into_inner(), claim.into()))
            .collect()
    }

    /// Claimed canonical selector-cardinality constraint ID.
    #[getter]
    pub fn cardinality_constraint(&self) -> u64 {
        self.inner.cardinality_constraint.into_inner()
    }
}

/// Read-only result of one checked SOS1 Big-M promotion.
///
/// State reconstruction remains owned by the mutated {class}`Instance`; this
/// value reports the inserted SOS1 constraint and the retained formulation
/// history.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sos1BigMPromotion {
    inner: ommx::Sos1BigMPromotion,
}

impl From<ommx::Sos1BigMPromotion> for Sos1BigMPromotion {
    fn from(inner: ommx::Sos1BigMPromotion) -> Self {
        Self { inner }
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Sos1BigMPromotion {
    /// ID allocated to the promoted active SOS1 constraint.
    #[getter]
    pub fn sos1_constraint_id(&self) -> u64 {
        self.inner.sos1_constraint_id().into_inner()
    }

    /// Members of the promoted SOS1 constraint.
    #[getter]
    pub fn members(&self) -> BTreeSet<u64> {
        self.inner
            .members()
            .iter()
            .map(|id| id.into_inner())
            .collect()
    }

    /// Verified fresh selectors keyed by their associated SOS1 member.
    #[getter]
    pub fn fresh_selectors(&self) -> BTreeMap<u64, u64> {
        self.inner
            .fresh_selectors()
            .iter()
            .map(|(member, selector)| (member.into_inner(), selector.into_inner()))
            .collect()
    }

    /// Verified regular-constraint IDs moved from active to removed.
    #[getter]
    pub fn relaxed_constraint_ids(&self) -> BTreeSet<u64> {
        self.inner
            .relaxed_constraint_ids()
            .iter()
            .map(|id| id.into_inner())
            .collect()
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Instance {
    /// Validate and promote one claimed SOS1 Big-M formulation in place.
    ///
    /// The request supplies stable IDs only. The Rust {class}`Instance` owner
    /// reads the current variable domains and regular rows, validates the full
    /// formulation, and commits the lifecycle move, selector reconstruction,
    /// and SOS1 insertion atomically. Invalid requests leave the instance
    /// unchanged.
    ///
    /// ``atol`` parameterizes the local projected-feasibility check, must be
    /// finite and satisfy ``0 < atol < 1``, and must also be used for subsequent
    /// state reconstruction and evaluation. Continuous member bounds and link
    /// rows use the same inequality-residual feasibility rule, so canonical
    /// unit-scale links may use tight Big-M values `U` for an upper link and
    /// `-L` for a lower link. If omitted, the current default returned by
    /// {func}`~ommx.get_default_atol` is used.
    ///
    /// Raises {class}`RuntimeError` when the claimed formulation is invalid for
    /// the current instance, including a positive-infinite tolerance or a
    /// finite ``atol >= 1``.
    /// Non-positive or NaN values rejected while constructing the tolerance
    /// raise {class}`ValueError`.
    #[pyo3(signature = (request, *, atol=None))]
    pub fn promote_sos1_big_m(
        &mut self,
        py: Python<'_>,
        request: &Sos1BigMPromotionRequest,
        atol: Option<f64>,
    ) -> OmmxPyResult<Sos1BigMPromotion> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        let outcomes = self
            .inner
            .promote_sos1_big_m(std::slice::from_ref(&request.inner), atol);
        // The Rust owner API guarantees one result per input request in the
        // same order. Violating that contract is an internal invariant failure,
        // not a caller-controlled promotion rejection.
        let [promotion] = outcomes
            .try_into()
            .expect("one request must produce exactly one aligned SOS1 promotion outcome");
        let promotion = promotion?;
        Ok(promotion.into())
    }
}
