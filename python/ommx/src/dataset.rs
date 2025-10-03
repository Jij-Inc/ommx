use pyo3::prelude::*;
use std::collections::HashMap;

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
#[pyfunction]
pub fn miplib2017_instance_annotations() -> HashMap<String, HashMap<String, String>> {
    ommx::dataset::miplib2017::instance_annotations()
        .into_iter()
        .map(|(instance, annotations)| (instance, annotations.into_inner()))
        .collect()
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
#[pyfunction]
pub fn qplib_instance_annotations() -> HashMap<String, HashMap<String, String>> {
    ommx::dataset::qplib::instance_annotations()
        .into_iter()
        .map(|(instance, annotations)| (instance, annotations.into_inner()))
        .collect()
}
