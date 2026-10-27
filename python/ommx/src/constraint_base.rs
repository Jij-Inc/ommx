//! Open Python interfaces for constraint families. Domain state remains on the
//! owning roots; these bases neither store rows nor grant mutation authority.

use pyo3::{exceptions::PyNotImplementedError, prelude::*};
use std::collections::HashMap;

/// Common base of constraint IDs, including families introduced by future SDKs.
///
/// Consumers must handle the concrete ID types they support and reject all
/// others. This base alone is not a usable constraint identity. Integers are
/// not instances of this class and are never accepted by mixed-family lookups.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(subclass, frozen)]
#[derive(Default)]
pub struct ConstraintIDBase;

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl ConstraintIDBase {
    #[new]
    fn new() -> Self {
        Self
    }
}

/// Common metadata interface for rows fetched from constraint collections.
///
/// The set of concrete constraint families is open. Inspect a concrete ID or
/// row type before using family-specific mathematical operations, and reject
/// unsupported families rather than dropping their constraints. This interface
/// does not imply whether the row is attached or an owned result snapshot.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(subclass)]
#[derive(Default)]
pub struct ConstraintView;

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl ConstraintView {
    #[getter]
    fn name(&self) -> PyResult<Option<String>> {
        Err(PyNotImplementedError::new_err(
            "Constraint family must implement name",
        ))
    }
    #[getter]
    fn description(&self) -> PyResult<Option<String>> {
        Err(PyNotImplementedError::new_err(
            "Constraint family must implement description",
        ))
    }
    #[getter]
    fn subscripts(&self) -> PyResult<Vec<i64>> {
        Err(PyNotImplementedError::new_err(
            "Constraint family must implement subscripts",
        ))
    }
    #[getter]
    fn parameters(&self) -> PyResult<HashMap<String, String>> {
        Err(PyNotImplementedError::new_err(
            "Constraint family must implement parameters",
        ))
    }
    #[getter]
    fn provenance(&self) -> PyResult<Vec<crate::Provenance>> {
        Err(PyNotImplementedError::new_err(
            "Constraint family must implement provenance",
        ))
    }
}

// Every binding family uses this initializer to preserve its concrete Python
// class while adding the common base. No domain data is moved into the base.
macro_rules! constraint_subclass {
    ($class:ty, $base:ty) => {
        impl From<$class> for pyo3::PyClassInitializer<$class> {
            fn from(value: $class) -> Self {
                pyo3::PyClassInitializer::from(<$base>::default()).add_subclass(value)
            }
        }
        impl<'py> pyo3::IntoPyObject<'py> for $class {
            type Target = Self;
            type Output = pyo3::Bound<'py, Self>;
            type Error = pyo3::PyErr;
            fn into_pyobject(self, py: pyo3::Python<'py>) -> pyo3::PyResult<Self::Output> {
                pyo3::Bound::new(py, self)
            }
        }
    };
}
// Shared only with the ID and row binding modules implementing these bases.
pub(crate) use constraint_subclass;
