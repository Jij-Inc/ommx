use anyhow::Result;
use ommx::{
    v1::{Constraint, Function, Instance, Linear, Polynomial, Quadratic, State},
    Evaluate, Message,
};
use pyo3::{prelude::*, types::PyBytes};
use std::collections::BTreeSet;

macro_rules! define_evaluate_function {
    ($evaluated:ty, $name:ident) => {
        #[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
        #[pyfunction]
        pub fn $name<'py>(
            function: &Bound<'py, PyBytes>,
            state: &Bound<'py, PyBytes>,
        ) -> Result<(f64, BTreeSet<u64>)> {
            let state = State::decode(state.as_bytes())?;
            let function = <$evaluated>::decode(function.as_bytes())?;
            function.evaluate(&state)
        }
    };
}

define_evaluate_function!(Function, evaluate_function);
define_evaluate_function!(Linear, evaluate_linear);
define_evaluate_function!(Quadratic, evaluate_quadratic);
define_evaluate_function!(Polynomial, evaluate_polynomial);

macro_rules! define_evaluate_object {
    ($evaluated:ty, $name:ident) => {
        #[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
        #[pyfunction]
        pub fn $name<'py>(
            py: Python<'py>,
            function: &Bound<'py, PyBytes>,
            state: &Bound<'py, PyBytes>,
        ) -> Result<(Bound<'py, PyBytes>, BTreeSet<u64>)> {
            let state = State::decode(state.as_bytes())?;
            let function = <$evaluated>::decode(function.as_bytes())?;
            let (evaluated, used_ids) = function.evaluate(&state)?;
            Ok((PyBytes::new_bound(py, &evaluated.encode_to_vec()), used_ids))
        }
    };
}

define_evaluate_object!(Constraint, evaluate_constraint);
define_evaluate_object!(Instance, evaluate_instance);

macro_rules! define_partial_evaluate_function {
    ($evaluated:ty, $name:ident) => {
        #[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
        #[pyfunction]
        pub fn $name<'py>(
            py: Python<'py>,
            function: &Bound<'py, PyBytes>,
            state: &Bound<'py, PyBytes>,
        ) -> Result<(Bound<'py, PyBytes>, BTreeSet<u64>)> {
            let state = State::decode(state.as_bytes())?;
            let mut function = <$evaluated>::decode(function.as_bytes())?;
            let set = function.partial_evaluate(&state)?;
            Ok((PyBytes::new_bound(py, &function.encode_to_vec()), set))
        }
    };
}

define_partial_evaluate_function!(Linear, partial_evaluate_linear);
define_partial_evaluate_function!(Quadratic, partial_evaluate_quadratic);
define_partial_evaluate_function!(Polynomial, partial_evaluate_polynomial);
define_partial_evaluate_function!(Function, partial_evaluate_function);

macro_rules! define_partial_evaluate_object {
    ($evaluated:ty, $name:ident) => {
        #[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
        #[pyfunction]
        pub fn $name<'py>(
            py: Python<'py>,
            obj: &Bound<'py, PyBytes>,
            state: &Bound<'py, PyBytes>,
        ) -> Result<(Bound<'py, PyBytes>, BTreeSet<u64>)> {
            let state = State::decode(state.as_bytes())?;
            let mut obj = <$evaluated>::decode(obj.as_bytes())?;
            let set = obj.partial_evaluate(&state)?;
            Ok((PyBytes::new_bound(py, &obj.encode_to_vec()), set))
        }
    };
}

define_partial_evaluate_object!(Constraint, partial_evaluate_constraint);
define_partial_evaluate_object!(Instance, partial_evaluate_instance);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
#[pyfunction]
pub fn used_decision_variable_ids(function: &Bound<PyBytes>) -> BTreeSet<u64> {
    let function = Function::decode(function.as_bytes()).unwrap();
    function.used_decision_variable_ids()
}
