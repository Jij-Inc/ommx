use anyhow::Result;
use ommx::{Message, Parse};
use pyo3::{prelude::*, types::PyBytes, Bound};
use std::collections::{BTreeMap, BTreeSet};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct SampledConstraint(pub ommx::SampledConstraint);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl SampledConstraint {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let v1_constraint = ommx::v1::SampledConstraint::decode(bytes.as_bytes())?;
        let constraint = v1_constraint.parse(&())?;
        Ok(Self(constraint))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let v1_constraint: ommx::v1::SampledConstraint = self.0.clone().into();
        Ok(PyBytes::new(py, &v1_constraint.encode_to_vec()))
    }

    /// Get the constraint ID
    #[getter]
    pub fn id(&self) -> u64 {
        self.0.id().into_inner()
    }

    /// Get the constraint equality type
    #[getter]
    pub fn equality(&self) -> crate::Equality {
        (*self.0.equality()).into()
    }

    /// Get the constraint name
    #[getter]
    pub fn name(&self) -> Option<String> {
        self.0.metadata.name.clone()
    }

    /// Get the subscripts
    #[getter]
    pub fn subscripts(&self) -> Vec<i64> {
        self.0.metadata.subscripts.clone()
    }

    /// Get the description
    #[getter]
    pub fn description(&self) -> Option<String> {
        self.0.metadata.description.clone()
    }

    /// Get the removal reason
    #[getter]
    pub fn removed_reason(&self) -> Option<String> {
        self.0.removed_reason().clone()
    }

    /// Get the removal reason parameters
    #[getter]
    pub fn removed_reason_parameters(&self) -> std::collections::HashMap<String, String> {
        self.0
            .removed_reason_parameters()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Get the used decision variable IDs
    #[getter]
    pub fn used_decision_variable_ids(&self) -> BTreeSet<u64> {
        self.0
            .used_decision_variable_ids()
            .iter()
            .map(|id| id.into_inner())
            .collect()
    }

    /// Get the evaluated values for all samples
    #[getter]
    pub fn evaluated_values(&self) -> BTreeMap<u64, f64> {
        self.0
            .evaluated_values()
            .iter()
            .map(|(&sample_id, value)| (sample_id.into_inner(), *value))
            .collect()
    }

    /// Get the feasibility status for all samples  
    #[getter]
    pub fn feasible(&self) -> BTreeMap<u64, bool> {
        self.0
            .feasible()
            .iter()
            .map(|(&sample_id, feasible)| (sample_id.into_inner(), *feasible))
            .collect()
    }

    /// Check if a field is set (HasField equivalent)
    pub fn has_field(&self, field_name: &str) -> bool {
        match field_name {
            "name" => self.0.metadata.name.is_some(),
            "description" => self.0.metadata.description.is_some(),
            "removed_reason" => self.0.removed_reason().is_some(),
            _ => false,
        }
    }
}
