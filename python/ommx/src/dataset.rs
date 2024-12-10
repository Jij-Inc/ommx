use pyo3::{prelude::*, types::PyDict};
use std::collections::HashMap;

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
#[pyfunction]
pub fn miplib2017_instance_annotations(
    py: Python<'_>,
) -> PyResult<HashMap<String, Bound<'_, PyDict>>> {
    ommx::dataset::miplib2017::instance_annotations()
        .into_iter()
        .map(|(instance, annotations)| {
            let dict = serde_pyobject::to_pyobject(py, &annotations)?;
            Ok((instance, dict.extract()?))
        })
        .collect::<PyResult<_>>()
}
