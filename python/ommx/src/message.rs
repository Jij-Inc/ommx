use approx::AbsDiffEq;
use ommx::{v1, Message};
use pyo3::{exceptions::PyRuntimeError, prelude::*, types::PyBytes};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct Linear(v1::Linear);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Linear {
    #[staticmethod]
    pub fn single_term(id: u64, coefficient: f64) -> Self {
        Self(v1::Linear::single_term(id, coefficient))
    }

    #[staticmethod]
    pub fn decode(bytes: &Bound<PyBytes>) -> PyResult<Self> {
        let inner = v1::Linear::decode(bytes.as_bytes())
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(Self(inner))
    }

    pub fn encode<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let bytes = self.0.encode_to_vec();
        Ok(PyBytes::new_bound(py, &bytes))
    }

    pub fn almost_equal(&self, other: &Linear, atol: f64) -> bool {
        self.0.abs_diff_eq(&other.0, atol)
    }

    pub fn __expr__(&self) -> String {
        self.0.to_string()
    }

    pub fn __add__(&self, rhs: &Linear) -> Linear {
        Linear(self.0.clone() + rhs.0.clone())
    }

    pub fn __sub__(&self, rhs: &Linear) -> Linear {
        Linear(self.0.clone() - rhs.0.clone())
    }

    pub fn __mul__(&self, rhs: &Linear) -> Quadratic {
        Quadratic(self.0.clone() * rhs.0.clone())
    }

    pub fn add_scalar(&self, scalar: f64) -> Linear {
        Linear(self.0.clone() + scalar)
    }

    pub fn mul_scalar(&self, scalar: f64) -> Linear {
        Linear(self.0.clone() * scalar)
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct Quadratic(v1::Quadratic);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Quadratic {
    #[staticmethod]
    pub fn decode(bytes: &Bound<PyBytes>) -> PyResult<Self> {
        let inner = v1::Quadratic::decode(bytes.as_bytes())
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(Self(inner))
    }

    pub fn encode<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let bytes = self.0.encode_to_vec();
        Ok(PyBytes::new_bound(py, &bytes))
    }

    pub fn almost_equal(&self, other: &Quadratic, atol: f64) -> bool {
        self.0.abs_diff_eq(&other.0, atol)
    }

    pub fn __expr__(&self) -> String {
        self.0.to_string()
    }

    pub fn __add__(&self, rhs: &Quadratic) -> Quadratic {
        Quadratic(self.0.clone() + rhs.0.clone())
    }

    pub fn __sub__(&self, rhs: &Quadratic) -> Quadratic {
        Quadratic(self.0.clone() - rhs.0.clone())
    }

    pub fn __mul__(&self, rhs: &Quadratic) -> Polynomial {
        Polynomial(self.0.clone() * rhs.0.clone())
    }

    pub fn add_scalar(&self, scalar: f64) -> Quadratic {
        Quadratic(self.0.clone() + scalar)
    }

    pub fn add_linear(&self, linear: &Linear) -> Quadratic {
        Quadratic(self.0.clone() + linear.0.clone())
    }

    pub fn mul_scalar(&self, scalar: f64) -> Quadratic {
        Quadratic(self.0.clone() * scalar)
    }

    pub fn mul_linear(&self, linear: &Linear) -> Polynomial {
        Polynomial(self.0.clone() * linear.0.clone())
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct Polynomial(v1::Polynomial);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Polynomial {
    #[staticmethod]
    pub fn decode(bytes: &Bound<PyBytes>) -> PyResult<Self> {
        let inner = v1::Polynomial::decode(bytes.as_bytes())
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(Self(inner))
    }

    pub fn encode<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let bytes = self.0.encode_to_vec();
        Ok(PyBytes::new_bound(py, &bytes))
    }

    pub fn almost_equal(&self, other: &Polynomial, atol: f64) -> bool {
        self.0.abs_diff_eq(&other.0, atol)
    }

    pub fn __expr__(&self) -> String {
        self.0.to_string()
    }

    pub fn __add__(&self, rhs: &Polynomial) -> Polynomial {
        Polynomial(self.0.clone() + rhs.0.clone())
    }

    pub fn __sub__(&self, rhs: &Polynomial) -> Polynomial {
        Polynomial(self.0.clone() - rhs.0.clone())
    }

    pub fn __mul__(&self, rhs: &Polynomial) -> Polynomial {
        Polynomial(self.0.clone() * rhs.0.clone())
    }

    pub fn add_scalar(&self, scalar: f64) -> Polynomial {
        Polynomial(self.0.clone() + scalar)
    }

    pub fn add_linear(&self, linear: &Linear) -> Polynomial {
        Polynomial(self.0.clone() + linear.0.clone())
    }

    pub fn add_quadratic(&self, quadratic: &Quadratic) -> Polynomial {
        Polynomial(self.0.clone() + quadratic.0.clone())
    }

    pub fn mul_scalar(&self, scalar: f64) -> Polynomial {
        Polynomial(self.0.clone() * scalar)
    }

    pub fn mul_linear(&self, linear: &Linear) -> Polynomial {
        Polynomial(self.0.clone() * linear.0.clone())
    }

    pub fn mul_quadratic(&self, quadratic: &Quadratic) -> Polynomial {
        Polynomial(self.0.clone() * quadratic.0.clone())
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct Function(v1::Function);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Function {
    #[staticmethod]
    pub fn from_scalar(scalar: f64) -> Self {
        Self(v1::Function::from(scalar))
    }

    #[staticmethod]
    pub fn from_linear(linear: &Linear) -> Self {
        Self(v1::Function::from(linear.0.clone()))
    }

    #[staticmethod]
    pub fn from_quadratic(quadratic: &Quadratic) -> Self {
        Self(v1::Function::from(quadratic.0.clone()))
    }

    #[staticmethod]
    pub fn from_polynomial(polynomial: &Polynomial) -> Self {
        Self(v1::Function::from(polynomial.0.clone()))
    }

    #[staticmethod]
    pub fn decode(bytes: &Bound<PyBytes>) -> PyResult<Self> {
        let inner = v1::Function::decode(bytes.as_bytes())
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(Self(inner))
    }

    pub fn encode<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let bytes = self.0.encode_to_vec();
        Ok(PyBytes::new_bound(py, &bytes))
    }

    pub fn almost_equal(&self, other: &Function, atol: f64) -> bool {
        self.0.abs_diff_eq(&other.0, atol)
    }

    pub fn __expr__(&self) -> String {
        self.0.to_string()
    }

    pub fn __add__(&self, rhs: &Function) -> Function {
        Function(self.0.clone() + rhs.0.clone())
    }

    pub fn __sub__(&self, rhs: &Function) -> Function {
        Function(self.0.clone() - rhs.0.clone())
    }

    pub fn __mul__(&self, rhs: &Function) -> Function {
        Function(self.0.clone() * rhs.0.clone())
    }

    pub fn add_scalar(&self, scalar: f64) -> Function {
        Function(self.0.clone() + scalar)
    }

    pub fn add_linear(&self, linear: &Linear) -> Function {
        Function(self.0.clone() + linear.0.clone())
    }

    pub fn add_quadratic(&self, quadratic: &Quadratic) -> Function {
        Function(self.0.clone() + quadratic.0.clone())
    }

    pub fn add_polynomial(&self, polynomial: &Polynomial) -> Function {
        Function(self.0.clone() + polynomial.0.clone())
    }

    pub fn mul_scalar(&self, scalar: f64) -> Function {
        Function(self.0.clone() * scalar)
    }

    pub fn mul_linear(&self, linear: &Linear) -> Function {
        Function(self.0.clone() * linear.0.clone())
    }

    pub fn mul_quadratic(&self, quadratic: &Quadratic) -> Function {
        Function(self.0.clone() * quadratic.0.clone())
    }

    pub fn mul_polynomial(&self, polynomial: &Polynomial) -> Function {
        Function(self.0.clone() * polynomial.0.clone())
    }
}
