//! Thin wrapper around `pandas.DataFrame` for type-safe PyO3 bindings.
//!
//! This allows returning `Bound<'py, PyDataFrame>` from Rust functions,
//! which pyo3-stub-gen maps to `pandas.DataFrame` in stubs.

use pyo3::{
    prelude::*,
    sync::PyOnceLock,
    types::{PyAny, PyType},
    Bound, Py, PyTypeCheck,
};

/// A transparent wrapper around `pandas.DataFrame`.
///
/// This is not a `#[pyclass]` — it wraps an existing Python type
/// similar to how PyO3 wraps `datetime.datetime` as `PyDateTime`.
#[repr(transparent)]
pub struct PyDataFrame(PyAny);

static DATAFRAME_TYPE: PyOnceLock<Py<PyType>> = PyOnceLock::new();

fn get_dataframe_type(py: Python<'_>) -> PyResult<&Bound<'_, PyType>> {
    Ok(DATAFRAME_TYPE
        .get_or_try_init(py, || -> PyResult<Py<PyType>> {
            let pandas = py.import("pandas")?;
            let df_type = pandas.getattr("DataFrame")?;
            Ok(df_type.cast_into::<PyType>()?.unbind())
        })?
        .bind(py))
}

unsafe impl PyTypeCheck for PyDataFrame {
    const NAME: &'static str = "DataFrame";

    fn type_check(object: &Bound<'_, PyAny>) -> bool {
        let py = object.py();
        match get_dataframe_type(py) {
            Ok(t) => object.is_instance(t).unwrap_or(false),
            Err(_) => false,
        }
    }

    fn classinfo_object(py: Python<'_>) -> Bound<'_, PyAny> {
        get_dataframe_type(py)
            .expect("Failed to import pandas.DataFrame")
            .clone()
            .into_any()
    }
}

impl pyo3_stub_gen::PyStubType for PyDataFrame {
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo {
            name: "pandas.DataFrame".to_string(),
            source_module: None,
            import: ["pandas".into()].into(),
            type_refs: Default::default(),
        }
    }
}
