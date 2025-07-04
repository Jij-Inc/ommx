use crate::{Linear, Polynomial, Quadratic, Rng};

use anyhow::{anyhow, Result};
use approx::AbsDiffEq;
use ommx::{ATol, Coefficient, CoefficientError, Evaluate};
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict, PyTuple},
    Bound, PyAny,
};
use std::collections::{BTreeMap, BTreeSet};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[derive(Clone)]
pub struct Function(pub ommx::Function);

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
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        Ok(Self(ommx::Function::from_bytes(bytes.as_bytes())?))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.0.to_bytes())
    }

    /// Try to convert this function to a linear function.
    ///
    /// Returns Some(Linear) if the function can be represented as linear,
    /// None otherwise. This is useful for checking if a function is suitable
    /// for linear programming solvers.
    pub fn as_linear(&self) -> Option<Linear> {
        self.0
            .as_linear()
            .map(|cow_linear| Linear(cow_linear.into_owned()))
    }

    /// Try to convert this function to a quadratic function.
    ///
    /// Returns Some(Quadratic) if the function can be represented as quadratic,
    /// None otherwise.
    pub fn as_quadratic(&self) -> Option<Quadratic> {
        self.0
            .as_quadratic()
            .map(|cow_quadratic| Quadratic(cow_quadratic.into_owned()))
    }

    /// Get the degree of this function.
    ///
    /// Returns the highest degree of any term in the function.
    /// Zero function has degree 0, constant function has degree 0,
    /// linear function has degree 1, quadratic function has degree 2, etc.
    pub fn degree(&self) -> u32 {
        self.0.degree().into_inner()
    }

    /// Get the number of terms in this function.
    ///
    /// Zero function has 0 terms, constant function has 1 term,
    /// and polynomial functions have the number of non-zero coefficient terms.
    pub fn num_terms(&self) -> usize {
        self.0.num_terms()
    }

    #[pyo3(signature = (other, atol=ATol::default().into_inner()))]
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

    pub fn add_assign(&mut self, rhs: &Function) {
        self.0 += &rhs.0;
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

    pub fn required_ids(&self) -> BTreeSet<u64> {
        self.0
            .required_ids()
            .into_iter()
            .map(|id| id.into_inner())
            .collect()
    }

    pub fn terms<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let result = PyDict::new(py);
        for (ids, coeff) in self.0.iter() {
            let u64_ids: Vec<u64> = ids.into_iter().map(|id| id.into_inner()).collect();
            let py_tuple = PyTuple::new(py, &u64_ids)?;
            result.set_item(py_tuple, coeff.into_inner())?;
        }
        Ok(result)
    }

    /// Get linear terms as a dictionary mapping variable id to coefficient.
    ///
    /// Returns dictionary mapping variable IDs to their linear coefficients.
    /// Returns empty dict if function has no linear terms.
    /// Works for all polynomial functions by filtering only degree-1 terms.
    #[getter]
    pub fn linear_terms(&self) -> BTreeMap<u64, f64> {
        self.0
            .linear_terms()
            .map(|(id, coeff)| (id.into_inner(), coeff.into_inner()))
            .collect()
    }

    /// Get quadratic terms as a dictionary mapping (row, col) to coefficient.
    ///
    /// Returns dictionary mapping variable ID pairs to their quadratic coefficients.
    /// Returns empty dict if function has no quadratic terms.
    /// Works for all polynomial functions by filtering only degree-2 terms.
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

    /// Get the constant term of the function.
    ///
    /// Returns the constant term. Returns 0.0 if function has no constant term.
    /// Works for all polynomial functions by filtering the degree-0 term.
    #[getter]
    pub fn constant_term(&self) -> f64 {
        self.0.constant_term()
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

    fn __copy__(&self) -> Self {
        self.clone()
    }

    // __deepcopy__ can also be implemented with self.clone()
    // memo argument is required to match Python protocol but not used in this implementation
    // Since this implementation contains no PyObject references, simple clone is sufficient
    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }

    #[getter]
    pub fn type_name(&self) -> &str {
        match self.0 {
            ommx::Function::Zero => "Zero",
            ommx::Function::Constant(_) => "Constant",
            ommx::Function::Linear(_) => "Linear",
            ommx::Function::Quadratic(_) => "Quadratic",
            ommx::Function::Polynomial(_) => "Polynomial",
        }
    }

    /// Reduce binary powers in the function.
    ///
    /// For binary variables, x^n = x for any n >= 1, so we can reduce higher powers to linear terms.
    ///
    /// Args:
    ///     binary_ids: Set of binary variable IDs to reduce powers for
    ///
    /// Returns:
    ///     True if any reduction was performed, False otherwise
    pub fn reduce_binary_power(&mut self, binary_ids: BTreeSet<u64>) -> bool {
        let variable_id_set: ommx::VariableIDSet =
            binary_ids.into_iter().map(ommx::VariableID::from).collect();
        self.0.reduce_binary_power(&variable_id_set)
    }
}
