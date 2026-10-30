//! Typed Python output for name-based extraction on model and result roots.

use pyo3::{
    prelude::*,
    types::{PyDict, PyTuple},
};
use pyo3_stub_gen::{PyStubType, TypeInfo};
use std::collections::BTreeMap;

// Shared by the root extraction bindings: Python keys must be tuples, while
// Rust domain queries use vectors for modeling subscripts.
pub(crate) struct SubscriptMap<T>(pub BTreeMap<Vec<i64>, T>);

impl<T: PyStubType> PyStubType for SubscriptMap<T> {
    fn type_output() -> TypeInfo {
        let mut value = T::type_output();
        value.name = format!(
            "builtins.dict[builtins.tuple[builtins.int, ...], {}]",
            value.name
        );
        value.source_module = None;
        value.import.insert("builtins".into());
        value
    }
}

impl<'py, T: IntoPyObject<'py>> IntoPyObject<'py> for SubscriptMap<T> {
    type Target = PyDict;
    type Output = Bound<'py, PyDict>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        let dict = PyDict::new(py);
        for (subscripts, value) in self.0 {
            dict.set_item(PyTuple::new(py, subscripts)?, value)?;
        }
        Ok(dict)
    }
}
