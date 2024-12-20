use anyhow::Result;
use ommx::Message;
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict},
};

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
        Ok(PyBytes::new_bound(py, &self.0.encode_to_vec()))
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
        Ok(SampleSet(self.0.evaluate_samples(&samples.0)?))
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
        Ok(PyBytes::new_bound(py, &self.0.encode_to_vec()))
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
        Ok(PyBytes::new_bound(py, &self.0.encode_to_vec()))
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
        Ok(PyBytes::new_bound(py, &self.0.encode_to_vec()))
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
        Ok(PyBytes::new_bound(py, &self.0.encode_to_vec()))
    }
}
