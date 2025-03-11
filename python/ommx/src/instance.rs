use anyhow::Result;
use ommx::{Evaluate, Message};
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict},
};
use std::collections::{BTreeSet, HashMap};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct Instance(ommx::v1::Instance);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Instance {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::v1::Instance::decode(bytes.as_bytes())?;
        inner.validate()?;
        Ok(Self(inner))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(py, &self.0.encode_to_vec()))
    }

    pub fn validate(&self) -> Result<()> {
        self.0.validate()
    }

    pub fn as_pubo_format<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyDict>> {
        let pubo = self.0.as_pubo_format()?;
        Ok(serde_pyobject::to_pyobject(py, &pubo)?.extract()?)
    }

    pub fn as_qubo_format<'py>(&self, py: Python<'py>) -> Result<(Bound<'py, PyDict>, f64)> {
        let (qubo, constant) = self.0.as_qubo_format()?;
        Ok((serde_pyobject::to_pyobject(py, &qubo)?.extract()?, constant))
    }

    pub fn as_parametric_instance(&self) -> ParametricInstance {
        ParametricInstance(self.0.clone().into())
    }

    pub fn penalty_method(&self) -> Result<ParametricInstance> {
        Ok(ParametricInstance(self.0.clone().penalty_method()?))
    }

    pub fn uniform_penalty_method(&self) -> Result<ParametricInstance> {
        Ok(ParametricInstance(self.0.clone().uniform_penalty_method()?))
    }

    pub fn evaluate_samples(&self, samples: &Samples) -> Result<SampleSet> {
        Ok(SampleSet(self.0.evaluate_samples(&samples.0)?.0))
    }

    pub fn relax_constraint(
        &mut self,
        constraint_id: u64,
        removed_reason: String,
        removed_reason_parameters: HashMap<String, String>,
    ) -> Result<()> {
        self.0
            .relax_constraint(constraint_id, removed_reason, removed_reason_parameters)
    }

    pub fn restore_constraint(&mut self, constraint_id: u64) -> Result<()> {
        self.0.restore_constraint(constraint_id)
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct ParametricInstance(ommx::v1::ParametricInstance);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl ParametricInstance {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::v1::ParametricInstance::decode(bytes.as_bytes())?;
        inner.validate()?;
        Ok(Self(inner))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(py, &self.0.encode_to_vec()))
    }

    pub fn validate(&self) -> Result<()> {
        self.0.validate()
    }

    pub fn with_parameters(&self, parameters: &Parameters) -> Result<Instance> {
        let instance = self.0.clone().with_parameters(parameters.0.clone())?;
        Ok(Instance(instance))
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct Parameters(ommx::v1::Parameters);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Parameters {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::v1::Parameters::decode(bytes.as_bytes())?;
        Ok(Self(inner))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(py, &self.0.encode_to_vec()))
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct Solution(ommx::v1::Solution);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Solution {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::v1::Solution::decode(bytes.as_bytes())?;
        Ok(Self(inner))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(py, &self.0.encode_to_vec()))
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct Samples(ommx::v1::Samples);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Samples {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::v1::Samples::decode(bytes.as_bytes())?;
        Ok(Self(inner))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(py, &self.0.encode_to_vec()))
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct SampleSet(ommx::v1::SampleSet);

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
}
