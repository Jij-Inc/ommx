use anyhow::Result;
use pyo3::prelude::*;

/// Variable bound wrapper for Python
///
/// Note: This struct is named `VariableBound` in Rust code to avoid conflicts with PyO3's `Bound` type,
/// but is exposed as `Bound` in Python through the `#[pyclass(name = "Bound")]` attribute.
#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass(name = "Bound")]
#[derive(Clone)]
pub struct VariableBound(pub ommx::Bound);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl VariableBound {
    #[new]
    pub fn new(lower: f64, upper: f64) -> Result<Self> {
        Ok(Self(ommx::Bound::new(lower, upper)?))
    }

    #[staticmethod]
    pub fn unbounded() -> Self {
        Self(ommx::Bound::default())
    }

    #[staticmethod]
    pub fn positive() -> Self {
        Self(ommx::Bound::positive())
    }

    #[staticmethod]
    pub fn negative() -> Self {
        Self(ommx::Bound::negative())
    }

    #[staticmethod]
    pub fn of_binary() -> Self {
        Self(ommx::Bound::of_binary())
    }

    #[getter]
    pub fn lower(&self) -> f64 {
        self.0.lower()
    }

    #[getter]
    pub fn upper(&self) -> f64 {
        self.0.upper()
    }

    pub fn width(&self) -> f64 {
        self.0.width()
    }

    pub fn is_finite(&self) -> bool {
        self.0.is_finite()
    }

    pub fn contains(&self, value: f64, atol: f64) -> Result<bool> {
        Ok(self.0.contains(value, ommx::ATol::new(atol)?))
    }

    pub fn nearest_to_zero(&self) -> f64 {
        self.0.nearest_to_zero()
    }

    pub fn intersection(&self, other: &VariableBound) -> Option<VariableBound> {
        self.0.intersection(&other.0).map(VariableBound)
    }

    pub fn __repr__(&self) -> String {
        self.0.to_string()
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    // __deepcopy__ can also be implemented with self.clone()
    // memo argument is required to match Python protocol but not used in this implementation
    // Since this implementation contains no PyObject references, simple clone is sufficient
    fn __deepcopy__(&self, _memo: pyo3::Bound<'_, pyo3::PyAny>) -> Self {
        self.clone()
    }
}
