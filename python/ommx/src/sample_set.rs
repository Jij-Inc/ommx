use crate::Solution;
use anyhow::Result;
use ommx::{Message, Parse};
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict, PyTuple},
    Bound, PyResult, Python,
};
use std::collections::{BTreeMap, BTreeSet};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct SampleSet(pub ommx::SampleSet);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl SampleSet {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let v1_inner = ommx::v1::SampleSet::decode(bytes.as_bytes())?;
        let inner = v1_inner.parse(&())?;
        Ok(Self(inner))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let v1_sample_set: ommx::v1::SampleSet = self.0.clone().into();
        Ok(PyBytes::new(py, &v1_sample_set.encode_to_vec()))
    }

    pub fn get(&self, sample_id: u64) -> Result<Solution> {
        let sample_id = ommx::SampleID::from(sample_id);
        let solution = self.0.get(sample_id)?;
        Ok(Solution(solution))
    }

    pub fn num_samples(&self) -> usize {
        self.0.sample_ids().len()
    }

    pub fn sample_ids(&self) -> BTreeSet<u64> {
        self.0
            .sample_ids()
            .iter()
            .map(|id| id.into_inner())
            .collect()
    }

    pub fn feasible_ids(&self) -> BTreeSet<u64> {
        self.0
            .feasible_ids()
            .iter()
            .map(|id| id.into_inner())
            .collect()
    }

    pub fn feasible_relaxed_ids(&self) -> BTreeSet<u64> {
        self.0
            .feasible_relaxed_ids()
            .iter()
            .map(|id| id.into_inner())
            .collect()
    }

    pub fn feasible_unrelaxed_ids(&self) -> BTreeSet<u64> {
        // For now, this is the same as feasible_ids since ommx::SampleSet doesn't distinguish
        self.feasible_ids()
    }

    #[getter]
    pub fn best_feasible_id(&self) -> Result<u64> {
        Ok(self.0.best_feasible_id()?.into_inner())
    }

    #[getter]
    pub fn best_feasible_relaxed_id(&self) -> Result<u64> {
        Ok(self.0.best_feasible_relaxed_id()?.into_inner())
    }

    #[getter]
    pub fn best_feasible(&self) -> Result<Solution> {
        Ok(Solution(self.0.best_feasible()?))
    }

    #[getter]
    pub fn best_feasible_relaxed(&self) -> Result<Solution> {
        Ok(Solution(self.0.best_feasible_relaxed()?))
    }

    #[getter]
    pub fn best_feasible_unrelaxed(&self) -> Result<Solution> {
        // Exactly the same as best_feasible
        self.best_feasible()
    }

    /// Get objectives for all samples
    #[getter]
    pub fn objectives(&self) -> BTreeMap<u64, f64> {
        self.0
            .objectives()
            .iter()
            .map(|(sample_id, objective)| (sample_id.into_inner(), *objective))
            .collect()
    }

    /// Get feasibility status for all samples
    #[getter]
    pub fn feasible(&self) -> BTreeMap<u64, bool> {
        self.0
            .feasible()
            .iter()
            .map(|(sample_id, &is_feasible)| (sample_id.into_inner(), is_feasible))
            .collect()
    }

    /// Get relaxed feasibility status for all samples
    #[getter]
    pub fn feasible_relaxed(&self) -> BTreeMap<u64, bool> {
        self.0
            .feasible_relaxed()
            .iter()
            .map(|(sample_id, &is_feasible)| (sample_id.into_inner(), is_feasible))
            .collect()
    }

    /// Get unrelaxed feasibility status for all samples
    #[getter]
    pub fn feasible_unrelaxed(&self) -> BTreeMap<u64, bool> {
        self.feasible()
    }

    /// Get the optimization sense (minimize or maximize)
    #[getter]
    pub fn sense(&self) -> crate::Sense {
        match self.0.sense() {
            ommx::Sense::Minimize => crate::Sense::Minimize,
            ommx::Sense::Maximize => crate::Sense::Maximize,
        }
    }

    /// Get constraints for compatibility with existing Python code
    #[getter]
    pub fn constraints(&self) -> Vec<crate::SampledConstraint> {
        self.0
            .constraints()
            .iter()
            .map(|(_, constraint)| crate::SampledConstraint(constraint.clone()))
            .collect()
    }

    /// Get decision variables for compatibility with existing Python code
    #[getter]
    pub fn decision_variables(&self) -> Vec<crate::SampledDecisionVariable> {
        self.0
            .decision_variables()
            .iter()
            .map(|(_, variable)| crate::SampledDecisionVariable(variable.clone()))
            .collect()
    }

    /// Get sample IDs as a list (property version)
    #[getter]
    pub fn sample_ids_list(&self) -> Vec<u64> {
        self.0
            .sample_ids()
            .iter()
            .map(|&sample_id| sample_id.into_inner())
            .collect()
    }

    /// Extract decision variable values for a given name and sample ID
    pub fn extract_decision_variables<'py>(
        &self,
        py: Python<'py>,
        name: &str,
        sample_id: u64,
    ) -> Result<Bound<'py, PyDict>> {
        let sample_id = ommx::SampleID::from(sample_id);
        let extracted = self.0.extract_decision_variables(name, sample_id)?;
        let dict = PyDict::new(py);
        for (subscripts, value) in extracted {
            // Convert Vec<i64> to tuple for use as dict key
            let key = PyTuple::new(py, &subscripts)?;
            dict.set_item(key, value)?;
        }
        Ok(dict)
    }

    /// Extract constraint values for a given name and sample ID
    pub fn extract_constraints<'py>(
        &self,
        py: Python<'py>,
        name: &str,
        sample_id: u64,
    ) -> Result<Bound<'py, PyDict>> {
        let sample_id = ommx::SampleID::from(sample_id);
        let extracted = self.0.extract_constraints(name, sample_id)?;
        let dict = PyDict::new(py);
        for (subscripts, value) in extracted {
            let key = PyTuple::new(py, &subscripts)?;
            dict.set_item(key, value)?;
        }
        Ok(dict)
    }
}
