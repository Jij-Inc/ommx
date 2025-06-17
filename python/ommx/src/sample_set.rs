use crate::Solution;
use anyhow::Result;
use ommx::Message;
use pyo3::{prelude::*, types::PyBytes, Bound};
use std::collections::{BTreeMap, BTreeSet};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct SampleSet(pub ommx::v1::SampleSet);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl SampleSet {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::v1::SampleSet::decode(bytes.as_bytes())?;
        Ok(Self(inner))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(py, &self.0.encode_to_vec()))
    }

    pub fn get(&self, sample_id: u64) -> PyResult<Solution> {
        Ok(self.0.get(sample_id).map(Solution)?)
    }

    pub fn num_samples(&self) -> PyResult<usize> {
        Ok(self.0.num_samples()?)
    }

    pub fn sample_ids(&self) -> BTreeSet<u64> {
        self.0.sample_ids()
    }

    pub fn feasible_ids(&self) -> BTreeSet<u64> {
        self.0.feasible_ids()
    }

    pub fn feasible_unrelaxed_ids(&self) -> BTreeSet<u64> {
        self.0.feasible_unrelaxed_ids()
    }

    pub fn best_feasible(&self) -> PyResult<Solution> {
        Ok(self.0.best_feasible().map(Solution)?)
    }

    pub fn best_feasible_unrelaxed(&self) -> PyResult<Solution> {
        Ok(self.0.best_feasible_unrelaxed().map(Solution)?)
    }

    /// Get objectives for all samples
    #[getter]
    pub fn objectives(&self) -> BTreeMap<u64, f64> {
        self.0.sample_ids().into_iter()
            .filter_map(|id| {
                self.0.get(id).ok().map(|solution| (id, solution.objective))
            })
            .collect()
    }

    /// Get feasibility status for all samples
    #[getter]
    pub fn feasible(&self) -> BTreeMap<u64, bool> {
        self.0.sample_ids().into_iter()
            .filter_map(|id| {
                self.0.get(id).ok().map(|solution| (id, solution.feasible))
            })
            .collect()
    }

    /// Get relaxed feasibility status for all samples
    #[getter]
    pub fn feasible_relaxed(&self) -> BTreeMap<u64, Option<bool>> {
        self.0.sample_ids().into_iter()
            .filter_map(|id| {
                self.0.get(id).ok().map(|solution| (id, solution.feasible_relaxed))
            })
            .collect()
    }

    /// Get unrelaxed feasibility status for all samples
    #[getter]
    pub fn feasible_unrelaxed(&self) -> BTreeMap<u64, bool> {
        self.0.sample_ids().into_iter()
            .filter_map(|id| {
                self.0.get(id).ok().map(|solution| (id, solution.feasible_unrelaxed))
            })
            .collect()
    }
}