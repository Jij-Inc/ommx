use crate::Rng;

use anyhow::{anyhow, Result};
use approx::AbsDiffEq;
use ommx::{v1, ATol, Coefficient, CoefficientError, Evaluate, Message, Monomial, Parse};
use ommx::{LinearMonomial, MonomialDyn};
use pyo3::{prelude::*, types::PyBytes};
use std::collections::BTreeMap;
use std::collections::BTreeSet;

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[derive(Clone)]
pub struct Linear(ommx::Linear);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Linear {
    #[new]
    #[pyo3(signature = (terms, constant=0.0))]
    pub fn new(terms: BTreeMap<u64, f64>, constant: f64) -> Result<Self> {
        let linear = ommx::v1::Linear::new(terms.into_iter(), constant);
        let parsed = ommx::Parse::parse(linear, &())?;
        Ok(Self(parsed))
    }

    #[staticmethod]
    pub fn single_term(id: u64, coefficient: f64) -> Result<Self> {
        match TryInto::<Coefficient>::try_into(coefficient) {
            Ok(coeff) => Ok(Self(ommx::Linear::single_term(id.into(), coeff))),
            Err(CoefficientError::Zero) => Ok(Self(ommx::Linear::default())),
            Err(e) => Err(e.into()),
        }
    }

    #[staticmethod]
    pub fn constant(constant: f64) -> Result<Self> {
        match TryInto::<Coefficient>::try_into(constant) {
            Ok(coeff) => Ok(Self(ommx::Linear::single_term(
                LinearMonomial::Constant,
                coeff,
            ))),
            Err(CoefficientError::Zero) => Ok(Self(ommx::Linear::default())), // Return zero if constant is zero
            Err(e) => Err(e.into()), // Return error for NaN or infinite
        }
    }

    #[staticmethod]
    #[pyo3(signature = (
        rng,
        num_terms=ommx::LinearParameters::default().num_terms(),
        max_id=ommx::LinearParameters::default().max_id().into_inner()
    ))]
    pub fn random(rng: &Rng, num_terms: usize, max_id: u64) -> Result<Self> {
        let mut rng = rng.lock().map_err(|_| anyhow!("Cannot get lock for RNG"))?;
        let inner: ommx::Linear = ommx::random::random(
            &mut rng,
            ommx::LinearParameters::new(num_terms, max_id.into())?,
        );
        Ok(Self(inner))
    }

    #[staticmethod]
    pub fn decode(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = v1::Linear::decode(bytes.as_bytes())?;
        Ok(Self(Parse::parse(inner, &())?))
    }

    pub fn encode<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let inner: v1::Linear = self.0.clone().into();
        let bytes = Message::encode_to_vec(&inner);
        Ok(PyBytes::new(py, &bytes))
    }

    pub fn linear_terms(&self) -> BTreeMap<u64, f64> {
        self.0
            .iter()
            .filter_map(|(id, coeff)| match id {
                LinearMonomial::Variable(id) => Some((id.into_inner(), coeff.into_inner())),
                _ => None,
            })
            .collect()
    }

    pub fn constant_term(&self) -> f64 {
        self.0
            .get(&LinearMonomial::Constant)
            .map(|coeff| coeff.into_inner())
            .unwrap_or(0.0)
    }

    pub fn almost_equal(&self, other: &Linear, atol: f64) -> Result<bool> {
        Ok(self.0.abs_diff_eq(&other.0, ommx::ATol::new(atol)?))
    }

    pub fn __repr__(&self) -> String {
        format!("Linear({})", self.0)
    }

    pub fn __add__(&self, rhs: &Linear) -> Linear {
        Linear(&self.0 + &rhs.0)
    }

    pub fn __sub__(&self, rhs: &Linear) -> Linear {
        Linear(&self.0 - &rhs.0)
    }

    pub fn __mul__(&self, rhs: &Linear) -> Quadratic {
        Quadratic(&self.0 * &rhs.0)
    }

    pub fn add_scalar(&self, scalar: f64) -> Result<Linear> {
        match TryInto::<Coefficient>::try_into(scalar) {
            Ok(coeff) => Ok(Linear(&self.0 + coeff)),
            Err(CoefficientError::Zero) => Ok(Linear(self.0.clone())), // Return unchanged if scalar is zero
            Err(e) => Err(e.into()), // Return error for NaN or infinite
        }
    }

    pub fn mul_scalar(&self, scalar: f64) -> Result<Linear> {
        match TryInto::<Coefficient>::try_into(scalar) {
            Ok(coeff) => Ok(Linear(self.0.clone() * coeff)),
            Err(CoefficientError::Zero) => Ok(Linear(ommx::Linear::default())), // Return zero if scalar is zero
            Err(e) => Err(e.into()), // Return error for NaN or infinite
        }
    }

    pub fn evaluate(&self, state: &Bound<PyBytes>) -> Result<f64> {
        use ommx::{Evaluate, Message};
        let state = ommx::v1::State::decode(state.as_bytes())?;
        self.0.evaluate(&state, ommx::ATol::default())
    }

    pub fn partial_evaluate(&self, state: &Bound<PyBytes>) -> Result<Linear> {
        use ommx::Message;
        let state = ommx::v1::State::decode(state.as_bytes())?;
        let mut inner = self.0.clone();
        inner.partial_evaluate(&state, ommx::ATol::default())?;
        Ok(Linear(inner))
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct Quadratic(ommx::Quadratic);

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
        // Convert to VariableID and Coefficient, filtering out zero values
        let col_ids: Vec<_> = columns.into_iter().map(|id| id.into()).collect();
        let row_ids: Vec<_> = rows.into_iter().map(|id| id.into()).collect();

        let mut filtered_cols = Vec::new();
        let mut filtered_rows = Vec::new();
        let mut filtered_coeffs = Vec::new();

        for ((col_id, row_id), value) in col_ids.into_iter().zip(row_ids).zip(values) {
            match TryInto::<Coefficient>::try_into(value) {
                Ok(coeff) => {
                    filtered_cols.push(col_id);
                    filtered_rows.push(row_id);
                    filtered_coeffs.push(coeff);
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

        let mut quadratic =
            ommx::Quadratic::from_coo(filtered_cols, filtered_rows, filtered_coeffs)?;

        // Add linear part if provided
        if let Some(linear) = linear {
            quadratic = quadratic + &linear.0;
        }

        Ok(Self(quadratic))
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

    pub fn linear_terms(&self) -> BTreeMap<u64, f64> {
        self.0
            .linear_terms()
            .into_iter()
            .map(|(id, coeff)| (id.into_inner(), coeff.into_inner()))
            .collect()
    }

    pub fn constant_term(&self) -> f64 {
        self.0.constant_term()
    }

    pub fn quadratic_terms(&self) -> BTreeMap<(u64, u64), f64> {
        self.0
            .quadratic_terms()
            .into_iter()
            .map(|(pair, coeff)| {
                (
                    (pair.lower().into_inner(), pair.upper().into_inner()),
                    coeff.into_inner(),
                )
            })
            .collect()
    }

    pub fn terms(&self) -> BTreeMap<Vec<u64>, f64> {
        self.0
            .iter()
            .map(|(monomial, coeff)| {
                let u64_ids: Vec<u64> = monomial.ids().map(|id| id.into_inner()).collect();
                (u64_ids, coeff.into_inner())
            })
            .collect()
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
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct Polynomial(ommx::Polynomial);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Polynomial {
    #[new]
    #[pyo3(signature = (terms, atol=ATol::default().into_inner()))]
    pub fn new(terms: BTreeMap<Vec<u64>, f64>, atol: f64) -> Result<Self> {
        let mut out = ommx::Polynomial::default();
        for (ids, coeff) in terms {
            if coeff.abs() > atol {
                let key = MonomialDyn::from_iter(ids.into_iter().map(|id| id.into()));
                out.add_term(key, coeff.try_into()?);
            }
        }
        Ok(Self(out))
    }

    #[staticmethod]
    pub fn decode(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = v1::Polynomial::decode(bytes.as_bytes())?;
        let parsed = Parse::parse(inner, &())?;
        Ok(Self(parsed))
    }

    pub fn encode<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let inner: v1::Polynomial = self.0.clone().into();
        let bytes = Message::encode_to_vec(&inner);
        Ok(PyBytes::new(py, &bytes))
    }

    pub fn almost_equal(&self, other: &Polynomial, atol: f64) -> bool {
        self.0.abs_diff_eq(&other.0, ommx::ATol::new(atol).unwrap())
    }

    pub fn __repr__(&self) -> String {
        format!("Polynomial({})", self.0)
    }

    pub fn __add__(&self, rhs: &Polynomial) -> Polynomial {
        Polynomial(&self.0 + &rhs.0)
    }

    pub fn __sub__(&self, rhs: &Polynomial) -> Polynomial {
        Polynomial(&self.0 - &rhs.0)
    }

    pub fn __mul__(&self, rhs: &Polynomial) -> Polynomial {
        Polynomial(&self.0 * &rhs.0)
    }

    pub fn add_scalar(&self, scalar: f64) -> Result<Polynomial> {
        match TryInto::<Coefficient>::try_into(scalar) {
            Ok(coeff) => Ok(Polynomial(&self.0 + coeff)),
            Err(CoefficientError::Zero) => Ok(Polynomial(self.0.clone())), // Return unchanged if scalar is zero
            Err(e) => Err(e.into()), // Return error for NaN or infinite
        }
    }

    pub fn add_linear(&self, linear: &Linear) -> Polynomial {
        Polynomial(&self.0 + &linear.0)
    }

    pub fn add_quadratic(&self, quadratic: &Quadratic) -> Polynomial {
        Polynomial(&self.0 + &quadratic.0)
    }

    pub fn mul_scalar(&self, scalar: f64) -> Result<Polynomial> {
        match TryInto::<Coefficient>::try_into(scalar) {
            Ok(coeff) => Ok(Polynomial(self.0.clone() * coeff)),
            Err(CoefficientError::Zero) => Ok(Polynomial(ommx::Polynomial::default())), // Return zero if scalar is zero
            Err(e) => Err(e.into()), // Return error for NaN or infinite
        }
    }

    pub fn mul_linear(&self, linear: &Linear) -> Polynomial {
        Polynomial(&self.0 * &linear.0)
    }

    pub fn mul_quadratic(&self, quadratic: &Quadratic) -> Polynomial {
        Polynomial(&self.0 * &quadratic.0)
    }

    pub fn terms(&self) -> BTreeMap<Vec<u64>, f64> {
        self.0
            .iter()
            .map(|(ids, coeff)| {
                let u64_ids: Vec<u64> = ids.into_iter().map(|id| id.into_inner()).collect();
                (u64_ids, coeff.into_inner())
            })
            .collect()
    }

    #[staticmethod]
    #[pyo3(signature = (
        rng,
        num_terms=ommx::PolynomialParameters::default().num_terms(),
        max_degree=ommx::PolynomialParameters::default().max_degree().into_inner(),
        max_id=ommx::PolynomialParameters::default().max_id().into_inner()
    ))]
    pub fn random(rng: &Rng, num_terms: usize, max_degree: u32, max_id: u64) -> Result<Self> {
        let mut rng = rng.lock().map_err(|_| anyhow!("Cannot get lock for RNG"))?;
        let inner: ommx::Polynomial = ommx::random::random(
            &mut rng,
            ommx::PolynomialParameters::new(num_terms, max_degree.into(), max_id.into())?,
        );
        Ok(Self(inner))
    }

    pub fn evaluate(&self, state: &Bound<PyBytes>) -> Result<f64> {
        use ommx::{Evaluate, Message};
        let state = ommx::v1::State::decode(state.as_bytes())?;
        self.0.evaluate(&state, ommx::ATol::default())
    }

    pub fn partial_evaluate(&self, state: &Bound<PyBytes>) -> Result<Polynomial> {
        use ommx::Message;
        let state = ommx::v1::State::decode(state.as_bytes())?;
        let mut inner = self.0.clone();
        inner.partial_evaluate(&state, ommx::ATol::default())?;
        Ok(Polynomial(inner))
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct Function(ommx::Function);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Function {
    #[staticmethod]
    pub fn from_scalar(scalar: f64) -> Result<Self> {
        match TryInto::<Coefficient>::try_into(scalar) {
            Ok(coeff) => Ok(Self(ommx::Function::from(coeff))),
            Err(CoefficientError::Zero) => Ok(Self(ommx::Function::default())), // Return zero function if scalar is zero
            Err(e) => Err(e.into()), // Return error for NaN or infinite
        }
    }

    #[staticmethod]
    pub fn from_linear(linear: &Linear) -> Self {
        Self(ommx::Function::from(linear.0.clone()))
    }

    #[staticmethod]
    pub fn from_quadratic(quadratic: &Quadratic) -> Self {
        Self(ommx::Function::from(quadratic.0.clone()))
    }

    #[staticmethod]
    pub fn from_polynomial(polynomial: &Polynomial) -> Self {
        Self(ommx::Function::from(polynomial.0.clone()))
    }

    #[staticmethod]
    pub fn decode(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = v1::Function::decode(bytes.as_bytes())?;
        let parsed = Parse::parse(inner, &())?;
        Ok(Self(parsed))
    }

    pub fn encode<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let inner: v1::Function = self.0.clone().into();
        let bytes = Message::encode_to_vec(&inner);
        Ok(PyBytes::new(py, &bytes))
    }

    pub fn almost_equal(&self, other: &Function, atol: f64) -> bool {
        self.0.abs_diff_eq(&other.0, ommx::ATol::new(atol).unwrap())
    }

    pub fn __repr__(&self) -> String {
        format!("Function({})", self.0)
    }

    pub fn __add__(&self, rhs: &Function) -> Function {
        Function(&self.0 + &rhs.0)
    }

    pub fn __sub__(&self, rhs: &Function) -> Function {
        Function(&self.0 - &rhs.0)
    }

    pub fn __mul__(&self, rhs: &Function) -> Function {
        Function(&self.0 * &rhs.0)
    }

    pub fn add_scalar(&self, scalar: f64) -> Result<Function> {
        match TryInto::<Coefficient>::try_into(scalar) {
            Ok(coeff) => Ok(Function(&self.0 + coeff)),
            Err(CoefficientError::Zero) => Ok(Function(self.0.clone())), // Return unchanged if scalar is zero
            Err(e) => Err(e.into()), // Return error for NaN or infinite
        }
    }

    pub fn add_linear(&self, linear: &Linear) -> Function {
        Function(&self.0 + &linear.0)
    }

    pub fn add_quadratic(&self, quadratic: &Quadratic) -> Function {
        Function(&self.0 + &quadratic.0)
    }

    pub fn add_polynomial(&self, polynomial: &Polynomial) -> Function {
        Function(&self.0 + &polynomial.0)
    }

    pub fn mul_scalar(&self, scalar: f64) -> Result<Function> {
        match TryInto::<Coefficient>::try_into(scalar) {
            Ok(coeff) => Ok(Function(&self.0 * coeff)),
            Err(CoefficientError::Zero) => Ok(Function(ommx::Function::default())), // Return zero if scalar is zero
            Err(e) => Err(e.into()), // Return error for NaN or infinite
        }
    }

    pub fn mul_linear(&self, linear: &Linear) -> Function {
        Function(&self.0 * &linear.0)
    }

    pub fn mul_quadratic(&self, quadratic: &Quadratic) -> Function {
        Function(&self.0 * &quadratic.0)
    }

    pub fn mul_polynomial(&self, polynomial: &Polynomial) -> Function {
        Function(&self.0 * &polynomial.0)
    }

    pub fn content_factor(&self) -> Result<f64> {
        self.0.content_factor().map(|c| c.into_inner())
    }

    pub fn used_decision_variable_ids(&self) -> BTreeSet<u64> {
        self.0
            .required_ids()
            .into_iter()
            .map(|id| id.into_inner())
            .collect()
    }

    pub fn terms(&self) -> BTreeMap<Vec<u64>, f64> {
        self.0
            .iter()
            .map(|(ids, coeff)| {
                let u64_ids: Vec<u64> = ids.into_iter().map(|id| id.into_inner()).collect();
                (u64_ids, coeff.into_inner())
            })
            .collect()
    }

    #[staticmethod]
    #[pyo3(signature = (
        rng,
        num_terms=ommx::PolynomialParameters::default().num_terms(),
        max_degree=ommx::PolynomialParameters::default().max_degree().into_inner(),
        max_id=ommx::PolynomialParameters::default().max_id().into_inner()
    ))]
    pub fn random(rng: &Rng, num_terms: usize, max_degree: u32, max_id: u64) -> Result<Self> {
        let mut rng = rng.lock().map_err(|_| anyhow!("Cannot get lock for RNG"))?;
        let inner: ommx::Function = ommx::random::random(
            &mut rng,
            ommx::PolynomialParameters::new(num_terms, max_degree.into(), max_id.into())?,
        );
        Ok(Self(inner))
    }

    pub fn evaluate(&self, state: &Bound<PyBytes>) -> Result<f64> {
        use ommx::{Evaluate, Message};
        let state = ommx::v1::State::decode(state.as_bytes())?;
        self.0.evaluate(&state, ommx::ATol::default())
    }

    pub fn partial_evaluate(&self, state: &Bound<PyBytes>) -> Result<Function> {
        use ommx::Message;
        let state = ommx::v1::State::decode(state.as_bytes())?;
        let mut inner = self.0.clone();
        inner.partial_evaluate(&state, ommx::ATol::default())?;
        Ok(Function(inner))
    }
}
