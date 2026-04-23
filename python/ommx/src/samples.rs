use anyhow::Result;
use ommx::{Message, Parse, SampleID};
use pyo3::{
    exceptions::{PyKeyError, PyTypeError},
    prelude::*,
    types::{PyBytes, PyDict},
    Bound,
};
use std::collections::{BTreeSet, HashMap};

#[pyclass(skip_from_py_object)]
#[derive(Clone, Default)]
pub struct Samples(pub ommx::Sampled<ommx::v1::State>);

// Manual PyClassInfo submission (instead of #[gen_stub_pyclass])
pyo3_stub_gen::inventory::submit! {
    pyo3_stub_gen::type_info::PyClassInfo {
        pyclass_name: "Samples",
        struct_id: || std::any::TypeId::of::<Samples>(),
        doc: "Collection of State samples",
        module: Some("ommx._ommx_rust"),
        bases: &[],
        getters: &[],
        setters: &[],
        has_eq: false,
        has_hash: false,
        has_ord: false,
        has_str: false,
        subclass: false,
    }
}

// PyStubType: input uses ToSamples, output uses Samples
impl pyo3_stub_gen::PyStubType for Samples {
    fn type_input() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo::locally_defined("ToSamples", "ommx._ommx_rust".into())
    }
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo::locally_defined("Samples", "ommx._ommx_rust".into())
    }
}

// FromPyObject: accepts Samples, State, Mapping[int, float], Mapping[int, ToState], Iterable[ToState]
impl<'py> FromPyObject<'_, 'py> for Samples {
    type Error = PyErr;
    fn extract(ob: Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        samples_from_any(ob.to_owned())
    }
}

pyo3_stub_gen::impl_py_runtime_type!(Samples);

// Dummy types for ToSamples type alias members

/// Mapping[int, float]
enum PyMappingIntFloat {}
impl pyo3_stub_gen::PyStubType for PyMappingIntFloat {
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo {
            import: ["collections.abc".into()].into(),
            name: "collections.abc.Mapping[int, float]".into(),
            source_module: None,
            type_refs: Default::default(),
        }
    }
}
impl pyo3_stub_gen::runtime::PyRuntimeType for PyMappingIntFloat {
    fn runtime_type_object(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
        py.import("collections.abc")?.getattr("Mapping")
    }
}

/// Mapping[int, ToState]
enum PyMappingIntToState {}
impl pyo3_stub_gen::PyStubType for PyMappingIntToState {
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo {
            import: ["collections.abc".into()].into(),
            name: "collections.abc.Mapping[int, ToState]".into(),
            source_module: None,
            type_refs: Default::default(),
        }
    }
}
impl pyo3_stub_gen::runtime::PyRuntimeType for PyMappingIntToState {
    fn runtime_type_object(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
        py.import("collections.abc")?.getattr("Mapping")
    }
}

/// collections.abc.Iterable[ToState]
enum PyIterableToState {}
impl pyo3_stub_gen::PyStubType for PyIterableToState {
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo {
            import: ["collections.abc".into()].into(),
            name: "collections.abc.Iterable[ToState]".into(),
            source_module: None,
            type_refs: Default::default(),
        }
    }
}
impl pyo3_stub_gen::runtime::PyRuntimeType for PyIterableToState {
    fn runtime_type_object(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
        py.import("collections.abc")?.getattr("Iterable")
    }
}

// Type alias: ToSamples = State | Samples | Mapping[int, float] | Mapping[int, ToState] | Iterable[ToState]
pyo3_stub_gen::type_alias!(
    "ommx._ommx_rust",
    ToSamples =
        crate::State | Samples | PyMappingIntFloat | PyMappingIntToState | PyIterableToState
);

impl From<ommx::v1::State> for Samples {
    fn from(state: ommx::v1::State) -> Self {
        Self(ommx::Sampled::from(state))
    }
}

fn type_error() -> PyErr {
    PyTypeError::new_err(
        "entries must be a State, Samples, Mapping[int, float], Mapping[int, ToState], or Iterable[ToState]",
    )
}

fn samples_from_any(entries: Bound<PyAny>) -> PyResult<Samples> {
    // pass through if already a Samples object
    if let Ok(s) = entries.cast::<Samples>() {
        return Ok(s.borrow().clone());
    }

    // Check dict first to handle the empty dict case before State's FromPyObject matches it
    if let Ok(state_dict) = entries.extract::<HashMap<u64, f64>>() {
        if state_dict.is_empty() {
            return Ok(Samples::default());
        }
        let mut state = ommx::v1::State::default();
        state.entries = state_dict;
        return Ok(Samples::from(state));
    }
    if let Ok(state) = entries.cast::<crate::State>() {
        return Ok(Samples::from(state.borrow().0.clone()));
    }

    // Try to extract as dict[int, State] or dict[int, dict[int, float]]
    if let Ok(dict) = entries.cast::<PyDict>() {
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
            match (
                state_cand.entries.is_empty(),
                sample_cand.num_samples() == 0,
            ) {
                (true, true) => Samples::default(),
                (false, true) => Samples::from(state_cand),
                (true, false) => Samples(sample_cand),
                (false, false) => {
                    return Err(type_error());
                }
            },
        );
    }

    // Try to extract as iterable of State-like objects
    if let Ok(iter) = entries.try_iter() {
        let mut sampled = ommx::Sampled::default();
        for (i, item) in iter.enumerate() {
            let sample_id = SampleID::from(i as u64);
            let item = item?;
            if let Ok(state) = extract_state(&item) {
                sampled
                    .append(std::iter::once(sample_id), state)
                    .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
                continue;
            }
            return Err(type_error());
        }
        return Ok(Samples(sampled));
    }

    Err(type_error())
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

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Samples {
    #[new]
    pub fn new(entries: Samples) -> Self {
        entries
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
                .ok_or_else(|| PyKeyError::new_err(format!("Unknown sample ID: {sample_id}")))?
                .clone(),
        ))
    }

    /// Append a sample with the given sample IDs and state
    pub fn append(&mut self, sample_ids: Vec<u64>, state: crate::State) -> PyResult<()> {
        let ids: Vec<ommx::SampleID> = sample_ids.into_iter().map(ommx::SampleID::from).collect();
        self.0
            .append(ids.into_iter(), state.0)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
        Ok(())
    }
}
