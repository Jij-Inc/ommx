use crate::{Linear, Quadratic, Rng};

use anyhow::{anyhow, Result};
use approx::AbsDiffEq;
use ommx::MonomialDyn;
use ommx::{v1, ATol, Coefficient, CoefficientError, Evaluate, Message, Parse};
use pyo3::{prelude::*, types::PyBytes, Bound, PyAny};
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
