use crate::{Linear, Quadratic, Rng};

use anyhow::{anyhow, Result};
use approx::AbsDiffEq;
use ommx::MonomialDyn;
use ommx::{ATol, Coefficient, CoefficientError, Evaluate};
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict, PyTuple},
    Bound, PyAny,
};
use std::collections::BTreeMap;

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[derive(Clone)]
pub struct Polynomial(pub ommx::Polynomial);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Polynomial {
    #[new]
    pub fn new(terms: BTreeMap<Vec<u64>, f64>) -> Result<Self> {
        let mut out = ommx::Polynomial::default();
        for (ids, coeff) in terms {
            match TryInto::<Coefficient>::try_into(coeff) {
                Ok(coeff) => {
                    let key = MonomialDyn::from_iter(ids.into_iter().map(|id| id.into()));
                    out.add_term(key, coeff);
                }
                Err(CoefficientError::Zero) => {
                    // Skip zero coefficients
                }
                Err(e) => return Err(e.into()),
            }
        }
        Ok(Self(out))
    }

    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        Ok(Self(ommx::Polynomial::from_bytes(bytes.as_bytes())?))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.0.to_bytes())
    }

    #[pyo3(signature = (other, atol=ATol::default().into_inner()))]
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

    pub fn add_assign(&mut self, rhs: &Polynomial) {
        self.0 += &rhs.0;
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

    pub fn terms<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let obj = serde_pyobject::to_pyobject(py, &self.0)?;
        Ok(obj.downcast::<PyDict>()?.clone())
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

    #[pyo3(signature = (state, *, atol=None))]
    pub fn evaluate(&self, state: &Bound<PyBytes>, atol: Option<f64>) -> Result<f64> {
        use ommx::{Evaluate, Message};
        let state = ommx::v1::State::decode(state.as_bytes())?;
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        self.0.evaluate(&state, atol)
    }

    #[pyo3(signature = (state, *, atol=None))]
    pub fn partial_evaluate(
        &self,
        state: &Bound<PyBytes>,
        atol: Option<f64>,
    ) -> Result<Polynomial> {
        use ommx::Message;
        let state = ommx::v1::State::decode(state.as_bytes())?;
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        let mut inner = self.0.clone();
        inner.partial_evaluate(&state, atol)?;
        Ok(Polynomial(inner))
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
