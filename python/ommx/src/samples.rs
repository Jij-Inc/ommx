use anyhow::Result;
use ommx::{Message, Parse, SampleID};
use pyo3::{
    exceptions::{PyKeyError, PyTypeError},
    prelude::*,
    types::{PyBytes, PyDict},
    Bound,
};
use std::collections::{BTreeSet, HashMap};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[derive(Clone, Default)]
pub struct Samples(pub ommx::Sampled<ommx::v1::State>);

impl From<ommx::v1::State> for Samples {
    fn from(state: ommx::v1::State) -> Self {
        Self(ommx::Sampled::from(state))
    }
}

fn type_error() -> PyErr {
    PyTypeError::new_err("entries must be a State, dict[int, State], or iterable[State]")
}

fn extract_state(value: &Bound<PyAny>) -> Result<ommx::v1::State, PyErr> {
    if let Ok(state) = value.extract::<crate::State>() {
        return Ok(state.0);
    }
    if let Ok(state_dict) = value.extract::<HashMap<u64, f64>>() {
        let mut state = ommx::v1::State::default();
        state.entries = state_dict;
        return Ok(state);
    }
    Err(type_error())
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Samples {
    #[new]
    pub fn new(entries: Bound<PyAny>) -> PyResult<Self> {
        // pass through
        if let Ok(state) = entries.extract::<Self>() {
            return Ok(state);
        }

        // Almost same as `extract_state`, but we need to handle empty case
        if let Ok(state) = entries.extract::<crate::State>() {
            return Ok(Self::from(state.0));
        }
        if let Ok(state_dict) = entries.extract::<HashMap<u64, f64>>() {
            if state_dict.is_empty() {
                return Ok(Self::default());
            }
            let mut state = ommx::v1::State::default();
            state.entries = state_dict;
            return Ok(Self::from(state));
        }

        // Try to extract as dict[int, State] or dict[int, dict[int, float]]
        if let Ok(dict) = entries.downcast::<PyDict>() {
            let mut state_cand = ommx::v1::State::default();
            let mut sample_cand: ommx::Sampled<ommx::v1::State> = ommx::Sampled::default();
            for (key, value) in dict.iter() {
                let sample_id: u64 = key.extract().map_err(|_| type_error())?;

                if let Ok(value) = value.extract::<f64>() {
                    state_cand.entries.insert(sample_id, value);
                    continue;
                }
                if let Ok(state) = extract_state(&value) {
                    sample_cand
                        .append(std::iter::once(SampleID::from(sample_id)), state)
                        .unwrap(); // safe unwrap since key is unique
                    continue;
                }
                return Err(type_error());
            }
            return Ok(
                match (state_cand.entries.is_empty(), sample_cand.is_empty()) {
                    (true, true) => Self::default(),
                    (false, true) => Self::from(state_cand),
                    (true, false) => Self(sample_cand),
                    (false, false) => {
                        return Err(type_error());
                    }
                },
            );
        }

        // Try to extract as iterable[State]
        if let Ok(iter) = entries.try_iter() {
            let mut sampled = ommx::Sampled::default();
            for (i, item) in iter.enumerate() {
                let sample_id = SampleID::from(i as u64);
                let item = item?;
                if let Ok(state) = extract_state(&item) {
                    sampled
                        .append(std::iter::once(sample_id), state)
                        .map_err(|e| {
                            PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string())
                        })?;
                    continue;
                }
                return Err(type_error());
            }
            return Ok(Self(sampled));
        }

        Err(type_error())
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

    /// Get the state for a specific sample ID
    pub fn get_state(&self, sample_id: u64) -> PyResult<crate::State> {
        let id = ommx::SampleID::from(sample_id);
        Ok(crate::State(
            self.0
                .get(id)
                .map_err(|e| PyKeyError::new_err(e.to_string()))?
                .clone(),
        ))
    }
}
