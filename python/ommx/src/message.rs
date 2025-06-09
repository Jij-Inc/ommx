use crate::Rng;

use anyhow::{anyhow, Result};
use approx::AbsDiffEq;
use ommx::{v1, Coefficient, Evaluate, Message, Parse};
use pyo3::{prelude::*, types::PyBytes};
use std::collections::BTreeSet;

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct Linear(ommx::Linear);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Linear {
    #[staticmethod]
    pub fn single_term(id: u64, coefficient: f64) -> Result<Self> {
        Ok(Self(ommx::Linear::single_term(
            id.into(),
            coefficient.try_into()?,
        )))
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

    pub fn almost_equal(&self, other: &Linear, atol: f64) -> Result<bool> {
        Ok(self.0.abs_diff_eq(&other.0, ommx::ATol::new(atol)?))
    }

    pub fn __repr__(&self) -> String {
        self.0.to_string()
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
        let coeff: Coefficient = scalar.try_into()?;
        Ok(Linear(&self.0 + coeff))
    }

    pub fn mul_scalar(&self, scalar: f64) -> Result<Linear> {
        let scalar: Coefficient = scalar.try_into()?;
        Ok(Linear(self.0.clone() * scalar))
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct Quadratic(ommx::Quadratic);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Quadratic {
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
        self.0.to_string()
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
        let coeff: Coefficient = scalar.try_into()?;
        Ok(Quadratic(&self.0 + coeff))
    }

    pub fn add_linear(&self, linear: &Linear) -> Quadratic {
        Quadratic(&self.0 + &linear.0)
    }

    pub fn mul_scalar(&self, scalar: f64) -> Result<Quadratic> {
        let coeff: Coefficient = scalar.try_into()?;
        Ok(Quadratic(self.0.clone() * coeff))
    }

    pub fn mul_linear(&self, linear: &Linear) -> Polynomial {
        Polynomial(&self.0 * &linear.0)
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
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct Polynomial(ommx::Polynomial);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Polynomial {
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
        self.0.to_string()
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
        let coeff: Coefficient = scalar.try_into()?;
        Ok(Polynomial(&self.0 + coeff))
    }

    pub fn add_linear(&self, linear: &Linear) -> Polynomial {
        Polynomial(&self.0 + &linear.0)
    }

    pub fn add_quadratic(&self, quadratic: &Quadratic) -> Polynomial {
        Polynomial(&self.0 + &quadratic.0)
    }

    pub fn mul_scalar(&self, scalar: f64) -> Result<Polynomial> {
        let coeff: Coefficient = scalar.try_into()?;
        Ok(Polynomial(self.0.clone() * coeff))
    }

    pub fn mul_linear(&self, linear: &Linear) -> Polynomial {
        Polynomial(&self.0 * &linear.0)
    }

    pub fn mul_quadratic(&self, quadratic: &Quadratic) -> Polynomial {
        Polynomial(&self.0 * &quadratic.0)
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
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct Function(ommx::Function);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Function {
    #[staticmethod]
    pub fn from_scalar(scalar: f64) -> Result<Self> {
        let coeff: Coefficient = scalar.try_into()?;
        Ok(Self(ommx::Function::from(coeff)))
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
        self.0.to_string()
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
        let coeff: Coefficient = scalar.try_into()?;
        Ok(Function(&self.0 + coeff))
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
        let coeff: Coefficient = scalar.try_into()?;
        Ok(Function(&self.0 * coeff))
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
}
