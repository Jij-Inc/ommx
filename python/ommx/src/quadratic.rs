use crate::{Linear, Polynomial, Rng};

use anyhow::{anyhow, Result};
use approx::AbsDiffEq;
use ommx::{
    v1, ATol, Coefficient, CoefficientError, Evaluate, Message, Monomial, Parse, VariableIDPair,
};
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict, PyTuple},
    Bound, PyAny,
};
use std::collections::BTreeMap;

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[derive(Clone)]
pub struct Quadratic(pub ommx::Quadratic);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Quadratic {
    #[new]
    #[pyo3(signature = (columns, rows, values, linear=None))]
    pub fn new(
        columns: Vec<u64>,
        rows: Vec<u64>,
        values: Vec<f64>,
        linear: Option<Linear>,
    ) -> Result<Self> {
        // Validate that all input vectors have the same length
        if columns.len() != rows.len() || columns.len() != values.len() {
            return Err(anyhow!(
                "Input vectors must have the same length: columns={}, rows={}, values={}",
                columns.len(),
                rows.len(),
                values.len()
            ));
        }

        let mut out = ommx::Quadratic::default();
        for ((col_id, row_id), value) in columns
            .into_iter()
            .zip(rows.into_iter())
            .zip(values.into_iter())
        {
            match TryInto::<Coefficient>::try_into(value) {
                Ok(coeff) => {
                    out.add_term(
                        ommx::QuadraticMonomial::Pair(VariableIDPair::new(
                            col_id.into(),
                            row_id.into(),
                        )),
                        coeff,
                    );
                }
                Err(CoefficientError::Zero) => {
                    // Skip zero coefficients
                    continue;
                }
                Err(e) => {
                    return Err(e.into()); // Return error for NaN or infinite
                }
            }
        }
        // Add linear part if provided
        if let Some(linear) = linear {
            out += &linear.0;
        }
        Ok(Self(out))
    }

    #[staticmethod]
    pub fn decode(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = v1::Quadratic::decode(bytes.as_bytes())?;
        let parsed = Parse::parse(inner, &())?;
        Ok(Self(parsed))
    }

    pub fn encode<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let inner: v1::Quadratic = self.0.clone().into();
        let bytes = Message::encode_to_vec(&inner);
        Ok(PyBytes::new(py, &bytes))
    }

    #[pyo3(signature = (other, atol=ATol::default().into_inner()))]
    pub fn almost_equal(&self, other: &Quadratic, atol: f64) -> bool {
        self.0.abs_diff_eq(&other.0, ommx::ATol::new(atol).unwrap())
    }

    pub fn __repr__(&self) -> String {
        format!("Quadratic({})", self.0)
    }

    pub fn __add__(&self, rhs: &Quadratic) -> Quadratic {
        Quadratic(&self.0 + &rhs.0)
    }

    pub fn __sub__(&self, rhs: &Quadratic) -> Quadratic {
        Quadratic(&self.0 - &rhs.0)
    }

    pub fn add_assign(&mut self, rhs: &Quadratic) {
        self.0 += &rhs.0;
    }

    pub fn __mul__(&self, rhs: &Quadratic) -> Polynomial {
        Polynomial(&self.0 * &rhs.0)
    }

    pub fn add_scalar(&self, scalar: f64) -> Result<Quadratic> {
        match TryInto::<Coefficient>::try_into(scalar) {
            Ok(coeff) => Ok(Quadratic(&self.0 + coeff)),
            Err(CoefficientError::Zero) => Ok(Quadratic(self.0.clone())), // Return unchanged if scalar is zero
            Err(e) => Err(e.into()), // Return error for NaN or infinite
        }
    }

    pub fn add_linear(&self, linear: &Linear) -> Quadratic {
        Quadratic(&self.0 + &linear.0)
    }

    pub fn mul_scalar(&self, scalar: f64) -> Result<Quadratic> {
        match TryInto::<Coefficient>::try_into(scalar) {
            Ok(coeff) => Ok(Quadratic(self.0.clone() * coeff)),
            Err(CoefficientError::Zero) => Ok(Quadratic(ommx::Quadratic::default())), // Return zero if scalar is zero
            Err(e) => Err(e.into()), // Return error for NaN or infinite
        }
    }

    pub fn mul_linear(&self, linear: &Linear) -> Polynomial {
        Polynomial(&self.0 * &linear.0)
    }

    #[getter]
    pub fn linear_terms(&self) -> BTreeMap<u64, f64> {
        self.0
            .linear_terms()
            .map(|(id, coeff)| (id.into_inner(), coeff.into_inner()))
            .collect()
    }

    #[getter]
    pub fn constant_term(&self) -> f64 {
        self.0.constant_term()
    }

    #[getter]
    pub fn quadratic_terms(&self) -> BTreeMap<(u64, u64), f64> {
        self.0
            .quadratic_terms()
            .map(|(pair, coeff)| {
                (
                    (pair.lower().into_inner(), pair.upper().into_inner()),
                    coeff.into_inner(),
                )
            })
            .collect()
    }

    pub fn terms<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let result = PyDict::new(py);
        for (monomial, coeff) in self.0.iter() {
            let u64_ids: Vec<u64> = monomial.ids().map(|id| id.into_inner()).collect();
            let py_tuple = PyTuple::new(py, &u64_ids)?;
            result.set_item(py_tuple, coeff.into_inner())?;
        }
        Ok(result)
    }

    #[staticmethod]
    #[pyo3(signature = (
        rng,
        num_terms=ommx::QuadraticParameters::default().num_terms(),
        max_id=ommx::QuadraticParameters::default().max_id().into_inner()
    ))]
    pub fn random(rng: &Rng, num_terms: usize, max_id: u64) -> Result<Self> {
        let mut rng = rng.lock().map_err(|_| anyhow!("Cannot get lock for RNG"))?;
        let inner: ommx::Quadratic = ommx::random::random(
            &mut rng,
            ommx::QuadraticParameters::new(num_terms, max_id.into())?,
        );
        Ok(Self(inner))
    }

    pub fn evaluate(&self, state: &Bound<PyBytes>) -> Result<f64> {
        use ommx::{Evaluate, Message};
        let state = ommx::v1::State::decode(state.as_bytes())?;
        self.0.evaluate(&state, ommx::ATol::default())
    }

    pub fn partial_evaluate(&self, state: &Bound<PyBytes>) -> Result<Quadratic> {
        use ommx::Message;
        let state = ommx::v1::State::decode(state.as_bytes())?;
        let mut inner = self.0.clone();
        inner.partial_evaluate(&state, ommx::ATol::default())?;
        Ok(Quadratic(inner))
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    // __deepcopy__ can also be implemented with self.clone()
    // memo argument is required to match Python protocol but not used in this implementation
    // Since this implementation contains no PyObject references, simple clone is sufficient
    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}
