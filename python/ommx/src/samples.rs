use crate::error::OmmxPyResult;
use ommx::{Message, Parse, SampleID};
use pyo3::{
    exceptions::{PyKeyError, PyTypeError, PyValueError},
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

    let iter = entries.try_iter()?;
    let mut sampled = ommx::Sampled::default();
    for (i, item) in iter.enumerate() {
        let state = item?.extract::<crate::State>()?;
        sampled.append([SampleID::from(i as u64)], state.0)?;
    }
    Ok(Samples(sampled))
}

fn samples_from_mapping(mapping: &Bound<PyMapping>) -> OmmxPyResult<Samples> {
    let mut items = mapping
        .call_method0("items")?
        .try_iter()?
        .map(|item| item?.extract::<(u64, Bound<PyAny>)>());
    let mut state = crate::State(Default::default());
    // Keep only the leading values that can be interpreted as both numbers and
    // states. The first unambiguous value determines how to read the remainder.
    let mut ambiguous = Vec::new();
    while let Some(item) = items.next() {
        let (id, value) = item?;
        let state_like =
            value.is_instance_of::<crate::State>() || value.is_instance_of::<PyMapping>();
        match value.extract::<f64>() {
            Ok(number) => {
                state.set(id, number);
                if !state_like {
                    break;
                }
                ambiguous.push((id, value));
            }
            Err(err)
                if state_like
                    && (err.is_instance_of::<PyTypeError>(value.py())
                        || err.is_instance_of::<PyValueError>(value.py())) =>
            {
                ambiguous.push((id, value));
                let mut sampled = ommx::Sampled::default();
                for item in ambiguous.into_iter().map(Ok).chain(items) {
                    let (id, value) = item?;
                    sampled.append([SampleID::from(id)], value.extract::<crate::State>()?.0)?;
                }
                return Ok(Samples(sampled));
            }
            Err(err) => return Err(err.into()),
        }
    }

    drop(ambiguous);
    for item in items {
        let (id, value) = item?;
        state.set(id, value.extract()?);
    }
    if state.0.entries.is_empty() {
        Ok(Samples::default())
    } else {
        Ok(Samples::from(state.0))
    }
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
