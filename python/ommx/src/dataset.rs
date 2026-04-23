use pyo3::prelude::*;
use std::collections::HashMap;

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
pub fn miplib2017_instance_annotations(py: Python<'_>) -> HashMap<String, HashMap<String, String>> {
    let _guard = crate::TRACING.attach_parent_context(py);
    ommx::dataset::miplib2017::instance_annotations()
        .into_iter()
        .map(|(instance, annotations)| (instance, annotations.into_inner()))
        .collect()
}

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
pub fn qplib_instance_annotations(py: Python<'_>) -> HashMap<String, HashMap<String, String>> {
    let _guard = crate::TRACING.attach_parent_context(py);
    ommx::dataset::qplib::instance_annotations()
        .into_iter()
        .map(|(instance, annotations)| (instance, annotations.into_inner()))
        .collect()
}
