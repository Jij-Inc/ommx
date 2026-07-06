use anyhow::Result;
use pyo3::{prelude::*, Bound, PyAny};

pub(crate) fn function_display<'py>(
    py: Python<'py>,
    formatted: ommx::FormattedFunction,
) -> Result<Bound<'py, PyAny>> {
    let function_display = py.import("ommx.display")?.getattr("FunctionDisplay")?;
    Ok(function_display.call1((
        formatted.text,
        formatted.total_terms,
        formatted.written_terms,
        formatted.omitted_terms,
        formatted.truncated_by_chars,
    ))?)
}
