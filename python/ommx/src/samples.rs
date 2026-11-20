use crate::error::OmmxPyResult;
use ommx::{Message, Parse, SampleID};
use pyo3::{
    exceptions::{PyKeyError, PyTypeError},
    prelude::*,
    types::{PyBytes, PyMapping},
    Bound,
};
use std::collections::BTreeSet;

#[pyo3_stub_gen::derive::gen_stub_pyclass(skip_stub_type, module = "ommx._ommx_rust")]
#[doc = include_str!("docstrings/ommx/Samples.md")]
#[pyclass]
#[derive(Clone, Default)]
pub struct Samples(pub ommx::Sampled<ommx::v1::State>);

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
        Ok(samples_from_any(ob.to_owned())?)
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

fn samples_from_any(entries: Bound<PyAny>) -> OmmxPyResult<Samples> {
    if let Ok(samples) = entries.cast::<Samples>() {
        return Ok(samples.borrow().clone());
    }
    if let Ok(state) = entries.cast::<crate::State>() {
        return Ok(Samples::from(state.borrow().0.clone()));
    }
    if let Ok(mapping) = entries.cast::<PyMapping>() {
        return samples_from_mapping(mapping);
    }

    let iter = entries.try_iter().map_err(|_| type_error())?;
    let mut sampled = ommx::Sampled::default();
    for (i, item) in iter.enumerate() {
        let state = item?.extract::<crate::State>()?;
        sampled.append([SampleID::from(i as u64)], state.0)?;
    }
    Ok(Samples(sampled))
}

fn samples_from_mapping(mapping: &Bound<PyMapping>) -> OmmxPyResult<Samples> {
    let items = mapping.items()?;
    let Some(first) = items.iter().next() else {
        return Ok(Samples::default());
    };

    // The values determine whether the keys are sample IDs or variable IDs.
    let value = first.get_item(1)?;
    if value.is_instance_of::<crate::State>() || value.is_instance_of::<PyMapping>() {
        let mut sampled = ommx::Sampled::default();
        for item in items {
            let (id, state) = item.extract::<(u64, crate::State)>()?;
            sampled.append([SampleID::from(id)], state.0)?;
        }
        return Ok(Samples(sampled));
    }

    let mut state = ommx::v1::State::default();
    for item in items {
        let (id, value) = item.extract::<(u64, f64)>()?;
        state.entries.insert(id, value);
    }
    Ok(Samples::from(state))
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Samples {
    #[new]
    pub fn new(entries: Samples) -> Self {
        entries
    }

    #[staticmethod]
    pub fn from_v1_bytes(bytes: &Bound<PyBytes>) -> OmmxPyResult<Self> {
        let v1_inner: ommx::v1::Samples =
            crate::message_io::decode(bytes.as_bytes(), "ommx.v1.Samples")?;
        let inner = v1_inner.parse(&())?;
        Ok(Self(inner))
    }

    pub fn to_v1_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let v1_samples: ommx::v1::Samples = self.0.clone().into();
        Ok(PyBytes::new(py, &v1_samples.encode_to_vec()))
    }

    /// Number of sample IDs, including separate IDs with identical assignments.
    pub fn num_samples(&self) -> usize {
        self.0.num_samples()
    }

    /// All sample IDs as an independent set of integers.
    pub fn sample_ids(&self) -> BTreeSet<u64> {
        self.0.ids().into_iter().map(|id| id.into_inner()).collect()
    }

    /// Return an independent State for one sample ID.
    ///
    /// Raises KeyError if the sample ID is not present. This returns raw candidate
    /// values, not a feasibility assessment; use {meth}`~ommx.Instance.evaluate_samples`
    /// to obtain evaluated results.
    pub fn get_state(&self, sample_id: u64) -> PyResult<crate::State> {
        let id = ommx::SampleID::from(sample_id);
        Ok(crate::State(
            self.0
                .get(id)
                .ok_or_else(|| PyKeyError::new_err(format!("Unknown sample ID: {sample_id}")))?
                .clone(),
        ))
    }

    /// Append a sample with the given sample IDs and state.
    ///
    /// Raises {class}`ValueError` if an ID already exists in this collection
    /// or occurs more than once in `sample_ids`. The collection is unchanged
    /// when validation fails.
    pub fn append(&mut self, sample_ids: Vec<u64>, state: crate::State) -> OmmxPyResult<()> {
        let ids: Vec<ommx::SampleID> = sample_ids.into_iter().map(ommx::SampleID::from).collect();
        self.0.append(ids, state.0)?;
        Ok(())
    }
}
