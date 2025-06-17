use anyhow::Result;
use ommx::Message;
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict},
    Bound, PyAny,
};
use std::collections::HashMap;

/// State wrapper for Python
#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[derive(Clone)]
pub struct State(pub ommx::v1::State);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl State {
    #[new]
    #[pyo3(signature = (entries = HashMap::default()))]
    pub fn new(entries: HashMap<u64, f64>) -> Self {
        let mut state = ommx::v1::State::default();
        state.entries = entries;
        Self(state)
    }

    #[staticmethod]
    pub fn from_dict(entries: &Bound<PyDict>) -> Result<Self> {
        let mut state_entries = HashMap::new();

        for (key, value) in entries.iter() {
            let key_u64: u64 = key.extract()?;
            let value_f64: f64 = value.extract()?;
            state_entries.insert(key_u64, value_f64);
        }

        Ok(Self::new(state_entries))
    }

    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::v1::State::decode(bytes.as_bytes())?;
        Ok(Self(inner))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(py, &self.0.encode_to_vec()))
    }

    #[getter]
    pub fn entries(&self) -> HashMap<u64, f64> {
        self.0.entries.clone()
    }

    #[setter]
    pub fn set_entries(&mut self, entries: HashMap<u64, f64>) {
        self.0.entries = entries;
    }

    pub fn get(&self, key: u64) -> Option<f64> {
        self.0.entries.get(&key).copied()
    }

    pub fn set(&mut self, key: u64, value: f64) {
        self.0.entries.insert(key, value);
    }

    pub fn __len__(&self) -> usize {
        self.0.entries.len()
    }

    pub fn __contains__(&self, key: u64) -> bool {
        self.0.entries.contains_key(&key)
    }

    pub fn keys(&self) -> Vec<u64> {
        self.0.entries.keys().copied().collect()
    }

    pub fn values(&self) -> Vec<f64> {
        self.0.entries.values().copied().collect()
    }

    pub fn items(&self) -> Vec<(u64, f64)> {
        self.0.entries.iter().map(|(&k, &v)| (k, v)).collect()
    }

    pub fn __repr__(&self) -> String {
        format!("State(entries={:?})", self.0.entries)
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}

impl From<ommx::v1::State> for State {
    fn from(state: ommx::v1::State) -> Self {
        Self(state)
    }
}

impl From<State> for ommx::v1::State {
    fn from(state: State) -> Self {
        state.0
    }
}
