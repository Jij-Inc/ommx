use anyhow::Result;
use ommx::ATol;
use ommx::{
    v1::{Constraint, Function, Instance, State},
    Evaluate, Message,
};
use pyo3::{prelude::*, types::PyBytes};
use std::collections::BTreeSet;


macro_rules! define_evaluate_object {
    ($evaluated:ty, $name:ident) => {
        #[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
        #[pyfunction]
        pub fn $name<'py>(
            py: Python<'py>,
            function: &Bound<'py, PyBytes>,
            state: &Bound<'py, PyBytes>,
        ) -> Result<Bound<'py, PyBytes>> {
            let state = State::decode(state.as_bytes())?;
            let function = <$evaluated>::decode(function.as_bytes())?;
            let evaluated = function.evaluate(&state, ATol::default())?;
            Ok(PyBytes::new(py, &evaluated.encode_to_vec()))
        }
    };
}

define_evaluate_object!(Constraint, evaluate_constraint);
define_evaluate_object!(Instance, evaluate_instance);



macro_rules! define_partial_evaluate_object {
    ($evaluated:ty, $name:ident) => {
        #[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
        #[pyfunction]
        pub fn $name<'py>(
            py: Python<'py>,
            obj: &Bound<'py, PyBytes>,
            state: &Bound<'py, PyBytes>,
        ) -> Result<Bound<'py, PyBytes>> {
            let state = State::decode(state.as_bytes())?;
            let mut obj = <$evaluated>::decode(obj.as_bytes())?;
            obj.partial_evaluate(&state, ATol::default())?;
            Ok(PyBytes::new(py, &obj.encode_to_vec()))
        }
    };
}

define_partial_evaluate_object!(Constraint, partial_evaluate_constraint);
define_partial_evaluate_object!(Instance, partial_evaluate_instance);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
#[pyfunction]
pub fn used_decision_variable_ids(function: &Bound<PyBytes>) -> BTreeSet<u64> {
    let function = Function::decode(function.as_bytes()).unwrap();
    function
        .required_ids()
        .into_iter()
        .map(|id| id.into_inner())
        .collect()
}
