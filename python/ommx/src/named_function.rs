use crate::Function;
use anyhow::Result;
use ommx::{Evaluate, Message, NamedFunctionID};
use pyo3::{prelude::*, types::PyBytes, Bound};
use std::collections::HashMap;

/// NamedFunction wrapper for Python
#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[derive(Clone)]
pub struct NamedFunction(pub ommx::NamedFunction);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl NamedFunction {
    #[new]
    #[pyo3(signature = (id, function, name=None, subscripts=Vec::new(), description=None, parameters=HashMap::default()))]
    pub fn new(
        id: u64,
        function: Function,
        name: Option<String>,
        subscripts: Vec<i64>,
        description: Option<String>,
        parameters: HashMap<String, String>,
    ) -> Result<Self> {
        let named_function_id = NamedFunctionID::from(id);

        let named_function = ommx::NamedFunction {
            id: named_function_id,
            function: function.0,
            name,
            subscripts,
            parameters: parameters.into_iter().collect(),
            description,
        };

        Ok(Self(named_function))
    }

    #[getter]
    pub fn id(&self) -> u64 {
        self.0.id.into_inner()
    }

    #[getter]
    pub fn function(&self) -> Function {
        Function(self.0.function.clone())
    }

    #[getter]
    pub fn name(&self) -> Option<String> {
        self.0.name.clone()
    }

    #[getter]
    pub fn subscripts(&self) -> Vec<i64> {
        self.0.subscripts.clone()
    }

    #[getter]
    pub fn parameters(&self) -> HashMap<String, String> {
        self.0.parameters.clone().into_iter().collect()
    }

    #[getter]
    pub fn description(&self) -> Option<String> {
        self.0.description.clone()
    }

    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        Ok(Self(ommx::NamedFunction::from_bytes(bytes.as_bytes())?))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.0.to_bytes())
    }

    #[pyo3(signature = (state, *, atol=None))]
    pub fn evaluate<'py>(
        &self,
        py: Python<'py>,
        state: &Bound<PyBytes>,
        atol: Option<f64>,
    ) -> Result<Bound<'py, PyBytes>> {
        let state = ommx::v1::State::decode(state.as_bytes())?;
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        let evaluated = self.0.evaluate(&state, atol)?;
        let v1_evaluated: ommx::v1::EvaluatedNamedFunction = evaluated.into();
        Ok(PyBytes::new(py, &v1_evaluated.encode_to_vec()))
    }

    #[pyo3(signature = (state, *, atol=None))]
    pub fn partial_evaluate<'py>(
        &mut self,
        py: Python<'py>,
        state: &Bound<PyBytes>,
        atol: Option<f64>,
    ) -> Result<Bound<'py, PyBytes>> {
        let state = ommx::v1::State::decode(state.as_bytes())?;
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        self.0.partial_evaluate(&state, atol)?;
        let inner: ommx::v1::NamedFunction = self.0.clone().into();
        Ok(PyBytes::new(py, &inner.encode_to_vec()))
    }
}
