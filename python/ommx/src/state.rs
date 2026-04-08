use anyhow::Result;
use ommx::Message;
use pyo3::{exceptions::PyTypeError, prelude::*, types::PyBytes, Bound, PyAny};
use pyo3_stub_gen::runtime::PyRuntimeType;
use std::collections::{BTreeMap, HashMap};

/// Normalize -0.0 to 0.0 to match protobuf serialization behavior
fn normalize_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

/// State wrapper for Python
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct State(pub ommx::v1::State);

// Manual PyClassInfo submission (instead of #[gen_stub_pyclass])
pyo3_stub_gen::inventory::submit! {
    pyo3_stub_gen::type_info::PyClassInfo {
        pyclass_name: "State",
        struct_id: || std::any::TypeId::of::<State>(),
        doc: "State wrapper for Python",
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

// PyStubType: input uses ToState, output uses State
impl pyo3_stub_gen::PyStubType for State {
    fn type_input() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo::locally_defined("ToState", "ommx._ommx_rust".into())
    }
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo::locally_defined("State", "ommx._ommx_rust".into())
    }
}

// FromPyObject: accepts State, Mapping[int, float], Iterable[tuple[int, float]]
impl<'py> FromPyObject<'_, 'py> for State {
    type Error = PyErr;
    fn extract(ob: Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        // Accept State object directly
        if let Ok(state) = ob.cast::<State>() {
            return Ok(state.borrow().clone());
        }
        // Accept dict[int, float] / Mapping[int, float]
        if let Ok(entries) = ob.extract::<HashMap<u64, f64>>() {
            let mut state = ommx::v1::State::default();
            state.entries = entries
                .into_iter()
                .map(|(k, v)| (k, normalize_zero(v)))
                .collect();
            return Ok(Self(state));
        }
        let err = || {
            PyTypeError::new_err(
                "ommx.v1.State can only be initialized with a `State`, `Mapping[int, float]`, or `Iterable[tuple[int, float]]`",
            )
        };
        // Accept Iterable[tuple[int, float]]
        if let Ok(iter) = ob.try_iter() {
            let mut state = ommx::v1::State::default();
            for item in iter {
                let (key, value) = item?.extract::<(u64, f64)>().map_err(|_| err())?;
                state.entries.insert(key, normalize_zero(value));
            }
            return Ok(Self(state));
        }
        Err(err())
    }
}

pyo3_stub_gen::impl_py_runtime_type!(State);

// Dummy types for ToState type alias members

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

impl PyRuntimeType for PyMappingIntFloat {
    fn runtime_type_object(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
        py.import("collections.abc")?.getattr("Mapping")
    }
}

/// Iterable[tuple[int, float]]
enum PyIterableIntFloat {}

impl pyo3_stub_gen::PyStubType for PyIterableIntFloat {
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo {
            import: ["collections.abc".into()].into(),
            name: "collections.abc.Iterable[tuple[int, float]]".into(),
            source_module: None,
            type_refs: Default::default(),
        }
    }
}

impl PyRuntimeType for PyIterableIntFloat {
    fn runtime_type_object(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
        py.import("collections.abc")?.getattr("Iterable")
    }
}

// Type alias: ToState = State | Mapping[int, float] | Iterable[tuple[int, float]]
pyo3_stub_gen::type_alias!(
    "ommx._ommx_rust",
    ToState = State | PyMappingIntFloat | PyIterableIntFloat
);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl State {
    #[new]
    pub fn new(entries: State) -> Self {
        entries
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
    pub fn entries(&self) -> BTreeMap<u64, f64> {
        self.0.entries.iter().map(|(k, v)| (*k, *v)).collect()
    }

    #[setter]
    pub fn set_entries(&mut self, entries: HashMap<u64, f64>) {
        self.0.entries = entries
            .into_iter()
            .map(|(k, v)| (k, normalize_zero(v)))
            .collect();
    }

    pub fn get(&self, key: u64) -> Option<f64> {
        self.0.entries.get(&key).copied()
    }

    pub fn set(&mut self, key: u64, value: f64) {
        self.0.entries.insert(key, normalize_zero(value));
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
