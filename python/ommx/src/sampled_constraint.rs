use pyo3::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
pub struct SampledConstraint(pub ommx::SampledConstraint);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl SampledConstraint {
    /// Get the constraint equality type
    #[getter]
    pub fn equality(&self) -> crate::Equality {
        self.0.equality.into()
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

    /// Get the provenance chain.
    ///
    /// See {attr}`~ommx.v1.Constraint.provenance` for semantics.
    #[getter]
    pub fn provenance(&self) -> Vec<crate::Provenance> {
        crate::provenance_list(&self.0.metadata)
    }

    /// Get the used decision variable IDs
    #[getter]
    pub fn used_decision_variable_ids(&self) -> BTreeSet<u64> {
        self.0
            .stage
            .used_decision_variable_ids
            .iter()
            .map(|id| id.into_inner())
            .collect()
    }

    /// Get the evaluated values for all samples
    #[getter]
    pub fn evaluated_values(&self) -> BTreeMap<u64, f64> {
        self.0
            .stage
            .evaluated_values
            .iter()
            .map(|(&sample_id, value)| (sample_id.into_inner(), *value))
            .collect()
    }

    /// Get the feasibility status for all samples  
    #[getter]
    pub fn feasible(&self) -> BTreeMap<u64, bool> {
        self.0
            .stage
            .feasible
            .iter()
            .map(|(&sample_id, feasible)| (sample_id.into_inner(), *feasible))
            .collect()
    }
}
