use pyo3::{exceptions::PyValueError, prelude::*};
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use std::collections::{BTreeMap, BTreeSet};

/// Detector-supplied witness for promoting a regular constraint to one-hot form.
///
/// Construction validates only the certificate's Python shape. Full semantic
/// validation is performed by :meth:`Instance.check_promotion_certificate` or
/// one of the promotion mutation methods against the current instance.
#[gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneHotPromotionCertificate(pub(crate) ommx::OneHotPromotionCertificate);

#[gen_stub_pymethods]
#[pymethods]
impl OneHotPromotionCertificate {
    /// Create a one-hot promotion certificate.
    ///
    /// **Args:**
    ///
    /// - `source_constraint_id`: Active regular constraint claimed to be one-hot.
    /// - `variables`: Claimed one-hot decision-variable IDs. Duplicates are rejected.
    /// - `target_one_hot_constraint_id`: Optional requested target ID. Omit to allocate one.
    #[new]
    #[pyo3(signature = (*, source_constraint_id, variables, target_one_hot_constraint_id=None))]
    pub fn new(
        source_constraint_id: u64,
        variables: Vec<u64>,
        target_one_hot_constraint_id: Option<u64>,
    ) -> PyResult<Self> {
        let variable_count = variables.len();
        let variables: BTreeSet<ommx::VariableID> =
            variables.into_iter().map(ommx::VariableID::from).collect();
        if variables.len() != variable_count {
            return Err(PyValueError::new_err(
                "One-hot promotion certificate variables must be unique",
            ));
        }
        Ok(Self(ommx::OneHotPromotionCertificate {
            source_constraint_id: ommx::ConstraintID::from(source_constraint_id),
            variables,
            target_one_hot_constraint_id: target_one_hot_constraint_id
                .map(ommx::OneHotConstraintID::from),
        }))
    }

    /// Active regular constraint claimed to be one-hot.
    #[getter]
    pub fn source_constraint_id(&self) -> u64 {
        self.0.source_constraint_id.into_inner()
    }

    /// Claimed one-hot variable IDs in sorted order.
    #[getter]
    pub fn variables(&self) -> Vec<u64> {
        self.0.variables.iter().map(|id| id.into_inner()).collect()
    }

    /// Requested target ID, or `None` when OMMX should allocate one.
    #[getter]
    pub fn target_one_hot_constraint_id(&self) -> Option<u64> {
        self.0
            .target_one_hot_constraint_id
            .map(ommx::OneHotConstraintID::into_inner)
    }

    fn __repr__(&self) -> String {
        let target = self
            .target_one_hot_constraint_id()
            .map_or_else(|| "None".to_string(), |id| id.to_string());
        format!(
            "OneHotPromotionCertificate(source_constraint_id={}, variables={:?}, target_one_hot_constraint_id={target})",
            self.source_constraint_id(),
            self.variables(),
        )
    }
}

/// Informational result of checking a promotion certificate.
///
/// This object is not an applicable mutation plan. Promotion methods always
/// re-validate the original certificate against the current instance.
#[gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotionPreview {
    source_constraint_id: u64,
    variables: Vec<u64>,
    target_one_hot_constraint_id: u64,
}

impl From<ommx::PromotionPreview> for PromotionPreview {
    fn from(preview: ommx::PromotionPreview) -> Self {
        Self {
            source_constraint_id: preview.source_constraint_id().into_inner(),
            variables: preview
                .variables()
                .iter()
                .map(|id| id.into_inner())
                .collect(),
            target_one_hot_constraint_id: preview.target_one_hot_constraint_id().into_inner(),
        }
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl PromotionPreview {
    #[getter]
    pub fn source_constraint_id(&self) -> u64 {
        self.source_constraint_id
    }

    #[getter]
    pub fn variables(&self) -> Vec<u64> {
        self.variables.clone()
    }

    #[getter]
    pub fn target_one_hot_constraint_id(&self) -> u64 {
        self.target_one_hot_constraint_id
    }
}

/// Result of one successfully applied promotion.
#[gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotionResult {
    source_constraint_id: u64,
    target_one_hot_constraint_id: u64,
}

impl From<ommx::PromotionResult> for PromotionResult {
    fn from(result: ommx::PromotionResult) -> Self {
        Self {
            source_constraint_id: result.source_constraint_id().into_inner(),
            target_one_hot_constraint_id: result.target_one_hot_constraint_id().into_inner(),
        }
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl PromotionResult {
    #[getter]
    pub fn source_constraint_id(&self) -> u64 {
        self.source_constraint_id
    }

    #[getter]
    pub fn target_one_hot_constraint_id(&self) -> u64 {
        self.target_one_hot_constraint_id
    }
}

/// Result of an all-or-nothing bulk promotion.
#[gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotionReport {
    source_to_target: BTreeMap<u64, u64>,
}

impl From<ommx::PromotionReport> for PromotionReport {
    fn from(report: ommx::PromotionReport) -> Self {
        Self {
            source_to_target: report
                .source_to_target()
                .iter()
                .map(|(source, target)| (source.into_inner(), target.into_inner()))
                .collect(),
        }
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl PromotionReport {
    /// Mapping from promoted regular constraint IDs to one-hot target IDs.
    #[getter]
    pub fn source_to_target(&self) -> BTreeMap<u64, u64> {
        self.source_to_target.clone()
    }
}

/// Re-validated audit record for a previous one-hot promotion.
#[gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotionAudit {
    source_constraint_id: u64,
    variables: Vec<u64>,
    target_one_hot_constraint_id: u64,
    target_is_active: bool,
}

impl From<ommx::PromotionAudit> for PromotionAudit {
    fn from(audit: ommx::PromotionAudit) -> Self {
        Self {
            source_constraint_id: audit.source_constraint_id().into_inner(),
            variables: audit.variables().iter().map(|id| id.into_inner()).collect(),
            target_one_hot_constraint_id: audit.target_one_hot_constraint_id().into_inner(),
            target_is_active: audit.target_is_active(),
        }
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl PromotionAudit {
    #[getter]
    pub fn source_constraint_id(&self) -> u64 {
        self.source_constraint_id
    }

    /// Original one-hot members reconstructed from the retained regular source.
    #[getter]
    pub fn variables(&self) -> Vec<u64> {
        self.variables.clone()
    }

    #[getter]
    pub fn target_one_hot_constraint_id(&self) -> u64 {
        self.target_one_hot_constraint_id
    }

    /// Whether the target one-hot constraint is active rather than removed.
    #[getter]
    pub fn target_is_active(&self) -> bool {
        self.target_is_active
    }
}
