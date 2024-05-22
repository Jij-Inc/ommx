use anyhow::Result;
use ommx::{
    v1::{Function, State},
    Evaluate, Message,
};
use pyo3::{prelude::*, types::PyBytes};
use std::collections::BTreeSet;

#[pyfunction]
pub fn evaluate_function<'py>(
    function: &Bound<'py, PyBytes>,
    state: &Bound<'py, PyBytes>,
) -> Result<(f64, BTreeSet<u64>)> {
    let state = State::decode(state.as_bytes())?;
    let function = Function::decode(function.as_bytes())?;
    function.evaluate(&state)
}
