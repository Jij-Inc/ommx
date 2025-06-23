use anyhow::Result;
use ommx::{Message, Parse, SampleID};
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict, PyList, PyString},
    Bound,
};
use std::collections::{BTreeSet, HashMap};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[derive(Clone)]
pub struct Samples(pub ommx::Sampled<ommx::v1::State>);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Samples {
    #[new]
    pub fn new(entries: Bound<PyAny>) -> PyResult<Self> {
        // pass through
        if let Ok(state) = entries.extract::<Self>() {
            return Ok(state);
        }

        let mut sampled = ommx::Sampled::default();

        // Try to extract as a State (dict[int, float])
        if let Ok(state_dict) = entries.extract::<HashMap<u64, f64>>() {
            if state_dict.is_empty() {
                return Ok(Self(sampled));
            }
            let mut state = ommx::v1::State::default();
            state.entries = state_dict;
            sampled
                .append(std::iter::once(SampleID::from(0u64)), state)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
            return Ok(Self(sampled));
        }

        // Try to extract as dict[int, State] or dict[int, dict[int, float]]
        if let Ok(dict) = entries.downcast::<PyDict>() {
            for (key, value) in dict.iter() {
                let sample_id: u64 = key.extract().map_err(|_| {
                    PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                        "entries must be a State, dict[int, State], or iterable[State]",
                    )
                })?;

                // Try to extract value as State (dict[int, float])
                if let Ok(state_dict) = value.extract::<HashMap<u64, f64>>() {
                    let mut state = ommx::v1::State::default();
                    state.entries = state_dict;
                    sampled
                        .append(std::iter::once(SampleID::from(sample_id)), state)
                        .map_err(|e| {
                            PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string())
                        })?;
                } else {
                    return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                        "Dictionary values must be State objects or dict[int, float]",
                    ));
                }
            }
            return Ok(Self(sampled));
        }

        // Check if it's a string (which is iterable but should be rejected)
        if entries.downcast::<PyString>().is_ok() {
            return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "entries must be a State, dict[int, State], or iterable[State]",
            ));
        }

        // Try to extract as iterable[State]
        if let Ok(iter) = entries.try_iter() {
            for (i, item) in iter.enumerate() {
                let sample_id = i as u64;
                let item = item?;

                // Try to extract item as State (dict[int, float])
                if let Ok(state_dict) = item.extract::<HashMap<u64, f64>>() {
                    let mut state = ommx::v1::State::default();
                    state.entries = state_dict;
                    sampled
                        .append(std::iter::once(SampleID::from(sample_id)), state)
                        .map_err(|e| {
                            PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string())
                        })?;
                } else {
                    return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                        "Iterable items must be State objects or dict[int, float]",
                    ));
                }
            }
            return Ok(Self(sampled));
        }

        // If none of the above worked, return an error
        Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
            "entries must be a State, dict[int, State], or iterable[State]",
        ))
    }

    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let v1_inner = ommx::v1::Samples::decode(bytes.as_bytes())?;
        let inner = v1_inner.parse(&())?;
        Ok(Self(inner))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let v1_samples: ommx::v1::Samples = self.0.clone().into();
        Ok(PyBytes::new(py, &v1_samples.encode_to_vec()))
    }

    /// Get the number of samples
    pub fn num_samples(&self) -> usize {
        self.0.num_samples()
    }

    /// Get all sample IDs
    pub fn sample_ids(&self) -> BTreeSet<u64> {
        self.0.ids().into_iter().map(|id| id.into_inner()).collect()
    }
}
