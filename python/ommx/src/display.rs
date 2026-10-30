use pyo3::{prelude::*, Bound, PyAny};

pub struct FunctionDisplay(ommx::FormattedFunction);

impl FunctionDisplay {
    pub(crate) fn new(formatted: ommx::FormattedFunction) -> Self {
        Self(formatted)
    }
}

impl<'py> IntoPyObject<'py> for FunctionDisplay {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        let function_display = py.import("ommx.display")?.getattr("FunctionDisplay")?;
        function_display.call1((
            self.0.text,
            self.0.total_terms,
            self.0.written_terms,
            self.0.omitted_terms,
            self.0.truncated_by_chars,
        ))
    }
}

impl pyo3_stub_gen::PyStubType for FunctionDisplay {
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo {
            name: "display.FunctionDisplay".to_string(),
            source_module: None,
            import: ["ommx.display".into()].into(),
            type_refs: Default::default(),
        }
    }
}
