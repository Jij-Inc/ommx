use derive_more::Deref;
use pyo3::prelude::*;
use std::sync::{Arc, Mutex};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[derive(Deref, Default)]
pub struct Rng(Arc<Mutex<ommx::random::Rng>>);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Rng {
    /// Create a new random number generator with a deterministic seed.
    #[new]
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(ommx::random::Rng::deterministic())))
    }
}
