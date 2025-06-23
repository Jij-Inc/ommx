use crate::Solution;
use anyhow::Result;
use ommx::{Message, Parse};
use pyo3::{exceptions::PyValueError, prelude::*, types::PyBytes, Bound};
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

    pub fn get(&self, sample_id: u64) -> PyResult<Solution> {
        let sample_id = ommx::SampleID::from(sample_id);
        let solution = self
            .0
            .get(sample_id)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
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
            .sample_ids()
            .iter()
            .filter(|&&sample_id| self.0.is_sample_feasible(sample_id).unwrap_or(false))
            .map(|id| id.into_inner())
            .collect()
    }

    pub fn feasible_unrelaxed_ids(&self) -> BTreeSet<u64> {
        // For now, this is the same as feasible_ids since ommx::SampleSet doesn't distinguish
        self.feasible_ids()
    }

    pub fn best_feasible(&self) -> PyResult<Option<Solution>> {
        let feasible_ids = self.feasible_ids();
        if feasible_ids.is_empty() {
            return Ok(None);
        }

        let best_id = match self.0.sense() {
            ommx::Sense::Minimize => feasible_ids
                .iter()
                .min_by(|&&a, &&b| {
                    let a_obj = self
                        .0
                        .objectives()
                        .get(ommx::SampleID::from(a))
                        .unwrap_or(&f64::INFINITY);
                    let b_obj = self
                        .0
                        .objectives()
                        .get(ommx::SampleID::from(b))
                        .unwrap_or(&f64::INFINITY);
                    a_obj
                        .partial_cmp(b_obj)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .copied(),
            ommx::Sense::Maximize => feasible_ids
                .iter()
                .max_by(|&&a, &&b| {
                    let a_obj = self
                        .0
                        .objectives()
                        .get(ommx::SampleID::from(a))
                        .unwrap_or(&f64::NEG_INFINITY);
                    let b_obj = self
                        .0
                        .objectives()
                        .get(ommx::SampleID::from(b))
                        .unwrap_or(&f64::NEG_INFINITY);
                    a_obj
                        .partial_cmp(b_obj)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .copied(),
        };

        if let Some(id) = best_id {
            let solution = self
                .0
                .get(ommx::SampleID::from(id))
                .map_err(|e| PyValueError::new_err(e.to_string()))?;
            Ok(Some(Solution(solution)))
        } else {
            Ok(None)
        }
    }

    pub fn best_feasible_unrelaxed(&self) -> PyResult<Option<Solution>> {
        // For now, this is the same as best_feasible
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
        self.feasible_unrelaxed()
    }

    /// Get relaxed feasibility status for all samples
    #[getter]
    pub fn feasible_relaxed(&self) -> BTreeMap<u64, bool> {
        self.0
            .sample_ids()
            .iter()
            .map(|&sample_id| {
                let feasible = self
                    .0
                    .is_sample_feasible_relaxed(sample_id)
                    .unwrap_or(false);
                (sample_id.into_inner(), feasible)
            })
            .collect()
    }

    /// Get unrelaxed feasibility status for all samples
    #[getter]
    pub fn feasible_unrelaxed(&self) -> BTreeMap<u64, bool> {
        self.0
            .sample_ids()
            .iter()
            .map(|&sample_id| {
                let feasible = self.0.is_sample_feasible(sample_id).unwrap_or(false);
                (sample_id.into_inner(), feasible)
            })
            .collect()
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
    pub fn extract_decision_variables(
        &self,
        name: &str,
        sample_id: u64,
    ) -> anyhow::Result<Vec<(Vec<i64>, f64)>> {
        let sample_id = ommx::SampleID::from(sample_id);
        let mut result = Vec::new();

        for variable in self.0.decision_variables().values() {
            if variable.metadata.name.as_ref() != Some(&name.to_string()) {
                continue;
            }

            let subscripts = variable.metadata.subscripts.clone();

            // Check for duplicates
            if result.iter().any(|(s, _)| s == &subscripts) {
                anyhow::bail!("Duplicate decision variable subscript: {:?}", subscripts);
            }

            let value = *variable.samples().get(sample_id)?;
            result.push((subscripts, value));
        }

        Ok(result)
    }

    /// Extract constraint values for a given name and sample ID
    pub fn extract_constraints(
        &self,
        name: &str,
        sample_id: u64,
    ) -> anyhow::Result<Vec<(Vec<i64>, f64)>> {
        let sample_id = ommx::SampleID::from(sample_id);
        let mut result = Vec::new();

        for constraint in self.0.constraints().values() {
            if constraint.metadata.name.as_ref() != Some(&name.to_string()) {
                continue;
            }

            let subscripts = constraint.metadata.subscripts.clone();

            // Check for duplicates
            if result.iter().any(|(s, _)| s == &subscripts) {
                anyhow::bail!("Duplicate constraint subscript: {:?}", subscripts);
            }

            let value = *constraint.evaluated_values().get(sample_id)?;
            result.push((subscripts, value));
        }

        Ok(result)
    }
}
