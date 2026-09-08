use crate::{error::OmmxPyResult, Instance};
use pyo3::{exceptions::PyValueError, prelude::*, types::PyString};
use std::collections::BTreeMap;

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

/// Untrusted stable-ID request for a batch of SOS1 Big-M promotions.
///
/// Outer keys are regular cardinality constraint IDs. Each value maps intended
/// SOS1 member IDs to selector claims for that formulation. A cardinality ID
/// occurs at most once. Bounds, kinds, coefficients, and row contents are
/// read from the current {class}`Instance` when the batch is validated.
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
    pub fn new(selector_claims: BTreeMap<u64, BTreeMap<u64, Sos1BigMSelectorClaim>>) -> Self {
        Self {
            inner: selector_claims
                .into_iter()
                .map(|(cardinality, claims)| {
                    (
                        cardinality.into(),
                        claims
                            .into_iter()
                            .map(|(member, claim)| (member.into(), claim.inner))
                            .collect(),
                    )
                })
                .collect(),
        }
    }

    /// Formulation claims keyed by cardinality constraint ID, then member ID.
    #[getter]
    pub fn selector_claims(&self) -> BTreeMap<u64, BTreeMap<u64, Sos1BigMSelectorClaim>> {
        self.inner
            .iter()
            .map(|(&cardinality, claims)| {
                (
                    cardinality.into_inner(),
                    claims
                        .iter()
                        .map(|(&member, &claim)| (member.into_inner(), claim.into()))
                        .collect(),
                )
            })
            .collect()
    }
}

/// Read-only report for an entire SOS1 Big-M promotion batch.
///
/// Each cardinality constraint ID in the request has exactly one outcome:
/// promotion to an SOS1 constraint, or rejection with a diagnostic.
/// The ``promoted`` and ``rejections`` properties return snapshots derived
/// from that single outcome map. Their keys are disjoint and their union is
/// exactly the request's cardinality constraint IDs.
///
/// Both application modes return this report type. Strict mode returns it
/// only when all formulations were promoted. Membership, retained formulation
/// history, and selector reconstruction remain owned by the mutated
/// {class}`Instance`.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(frozen)]
#[derive(Debug)]
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
    /// Number of formulations in the original batch request.
    #[getter]
    pub fn request_count(&self) -> usize {
        self.inner.len()
    }

    /// Applied promotions: cardinality constraint ID to allocated SOS1 ID.
    #[getter]
    pub fn promoted(&self) -> BTreeMap<u64, u64> {
        self.inner
            .iter()
            .filter_map(|(&cardinality, outcome)| {
                outcome
                    .as_ref()
                    .ok()
                    .map(|id| (cardinality.into_inner(), id.into_inner()))
            })
            .collect()
    }

    /// Rejected formulations: cardinality constraint ID to diagnostic message.
    ///
    /// These are diagnostic strings, not exception objects. Rejected
    /// formulations were not applied.
    #[getter]
    pub fn rejections(&self) -> BTreeMap<u64, String> {
        self.inner
            .iter()
            .filter_map(|(&cardinality, outcome)| {
                outcome
                    .as_ref()
                    .err()
                    .map(|error| (cardinality.into_inner(), format!("{error:#}")))
            })
            .collect()
    }
}

/// Python-owned choice of batch application policy.
///
/// This binding-only type validates the two accepted string literals during
/// argument extraction; the Rust Instance owns both underlying operations.
#[derive(Debug, Clone, Copy)]
pub enum Sos1BigMPromotionMode {
    BestEffort,
    Strict,
}

impl<'py> FromPyObject<'_, 'py> for Sos1BigMPromotionMode {
    type Error = PyErr;

    fn extract(ob: pyo3::Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        match ob.extract::<&str>()? {
            "best_effort" => Ok(Self::BestEffort),
            "strict" => Ok(Self::Strict),
            mode => Err(PyValueError::new_err(format!(
                "Unknown SOS1 promotion mode {mode:?}: expected 'best_effort' or 'strict'"
            ))),
        }
    }
}

impl<'py> IntoPyObject<'py> for Sos1BigMPromotionMode {
    type Target = PyString;
    type Output = Bound<'py, PyString>;
    type Error = std::convert::Infallible;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        Ok(PyString::new(
            py,
            match self {
                Self::BestEffort => "best_effort",
                Self::Strict => "strict",
            },
        ))
    }
}

impl pyo3_stub_gen::PyStubType for Sos1BigMPromotionMode {
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo {
            name: r#"typing.Literal["best_effort", "strict"]"#.to_string(),
            source_module: None,
            import: ["typing".into()].into(),
            type_refs: Default::default(),
        }
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Instance {
    /// Validate and promote a batch of claimed SOS1 Big-M formulations.
    ///
    /// The batch request is keyed by regular cardinality constraint ID.
    /// The Rust {class}`Instance` checks every formulation against the same
    /// unchanged instance and reconciles conflicts before applying the plan.
    /// Both modes return one {class}`~ommx.Sos1BigMPromotion` batch report:
    ///
    /// - ``mode="best_effort"`` (default) applies independent valid formulations.
    ///   The report's ``promoted`` map associates each successful cardinality
    ///   ID with its allocated SOS1 ID; ``rejections`` maps rejected
    ///   cardinality IDs to diagnostic strings.
    /// - ``mode="strict"`` requires every formulation to be valid. Any
    ///   rejection raises {class}`~ommx.Sos1BigMPromotionBatchRejectedError`
    ///   before mutation. The exception's ``request_count`` is the batch
    ///   size and ``rejections`` contains all rejected cardinality IDs and
    ///   their diagnostics. A successful strict report has no rejections.
    ///
    /// Overlapping formulation rows are rejected; independent formulations
    /// may share SOS1 members. An empty request returns an empty report.
    /// Planning and application do not clone the instance.
    ///
    /// ``atol`` parameterizes the local projected-feasibility check and must
    /// also be used for subsequent state reconstruction and evaluation.
    /// Continuous bounds and link rows use the same inequality-residual rule,
    /// so canonical unit-scale links may use tight Big-M values `U` and `-L`.
    /// If omitted, {func}`~ommx.get_default_atol` supplies the default.
    /// Positive-infinite tolerances and finite ``atol >= 1`` reject every
    /// formulation in a non-empty batch under the selected mode.
    /// Non-positive or NaN tolerances, and unknown mode strings, raise
    /// {class}`ValueError` before planning, even for an empty batch.
    #[pyo3(signature = (request, *, mode=Sos1BigMPromotionMode::BestEffort, atol=None))]
    pub fn promote_sos1_big_m(
        &mut self,
        py: Python<'_>,
        request: &Sos1BigMPromotionRequest,
        mode: Sos1BigMPromotionMode,
        atol: Option<f64>,
    ) -> OmmxPyResult<Sos1BigMPromotion> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        let report = match mode {
            Sos1BigMPromotionMode::BestEffort => {
                self.inner.promote_sos1_big_m(&request.inner, atol)
            }
            Sos1BigMPromotionMode::Strict => self
                .inner
                .promote_sos1_big_m_if_fully_valid(&request.inner, atol)?,
        };
        Ok(report.into())
    }
}
