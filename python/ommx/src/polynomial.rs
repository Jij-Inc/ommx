use crate::{Constraint, Function, Linear, Quadratic, Rng, State};

use anyhow::{anyhow, Result};
use approx::AbsDiffEq;
use ommx::MonomialDyn;
use ommx::{ATol, Coefficient, CoefficientError, Evaluate};
use pyo3::{prelude::*, types::PyDict, Bound, PyAny};
use std::collections::BTreeMap;

/// Polynomial function of decision variables.
///
/// A polynomial function of arbitrary degree with terms of the form $c \cdot x_1^{a_1} \cdot x_2^{a_2} \cdots$
/// where $x_i$ are decision variables and $c$ is a coefficient.
///
/// # Examples
///
/// Create via DecisionVariable operations:
///
/// ```python
/// >>> x = DecisionVariable.integer(1)
/// >>> y = DecisionVariable.integer(2)
/// >>> p = x * x * y + x * y * y + 1  # Cubic polynomial
/// ```
///
/// Note that `==`, `<=`, `>=` create Constraint objects:
///
/// ```python
/// >>> constraint = p == 0  # Returns Constraint
/// ```
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct Polynomial(pub ommx::Polynomial);

// Overload stubs for arithmetic operators.
// Must appear before #[gen_stub_pymethods] for correct ordering.
pyo3_stub_gen::inventory::submit! {
    pyo3_stub_gen::derive::gen_methods_from_python! {
        r#"
        class Polynomial:
            @overload
            def __add__(self, rhs: ScalarLike | LinearLike | Parameter | Quadratic | Polynomial) -> Polynomial: ...
            @overload
            def __add__(self, rhs: Function) -> Function: ...

            @overload
            def __radd__(self, lhs: ScalarLike | LinearLike | Parameter | Quadratic | Polynomial) -> Polynomial: ...
            @overload
            def __radd__(self, lhs: Function) -> Function: ...

            @overload
            def __sub__(self, rhs: ScalarLike | LinearLike | Parameter | Quadratic | Polynomial) -> Polynomial: ...
            @overload
            def __sub__(self, rhs: Function) -> Function: ...

            @overload
            def __rsub__(self, lhs: ScalarLike | LinearLike | Parameter | Quadratic | Polynomial) -> Polynomial: ...
            @overload
            def __rsub__(self, lhs: Function) -> Function: ...

            @overload
            def __mul__(self, rhs: ScalarLike | LinearLike | Parameter | Quadratic | Polynomial) -> Polynomial: ...
            @overload
            def __mul__(self, rhs: Function) -> Function: ...

            @overload
            def __rmul__(self, lhs: ScalarLike | LinearLike | Parameter | Quadratic | Polynomial) -> Polynomial: ...
            @overload
            def __rmul__(self, lhs: Function) -> Function: ...

            def __iadd__(self, rhs: Polynomial) -> Polynomial: ...
        "#
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
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

    #[pyo3(signature = (other, atol=ATol::default().into_inner()))]
    pub fn almost_equal(&self, other: &Polynomial, atol: f64) -> bool {
        self.0.abs_diff_eq(&other.0, ommx::ATol::new(atol).unwrap())
    }

    pub fn __repr__(&self) -> String {
        format!("Polynomial({})", self.0)
    }

    /// Negation operator
    pub fn __neg__(&self) -> Polynomial {
        Polynomial(-self.0.clone())
    }

    /// Polymorphic addition. Dispatches on the operand class of `rhs`
    /// (see `crate::FunctionInput`). All non-Function inputs promote to Polynomial.
    #[gen_stub(skip)]
    #[pyo3(name = "__add__")]
    pub fn py_add(&self, py: Python<'_>, rhs: crate::FunctionInput) -> PyResult<Py<PyAny>> {
        Ok(match rhs {
            crate::FunctionInput::Scalar(None) => Polynomial(self.0.clone())
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Scalar(Some(c)) => Polynomial(&self.0 + c)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Linear(l) => Polynomial(&self.0 + &l)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Quadratic(q) => Polynomial(&self.0 + &q)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Polynomial(p) => Polynomial(&self.0 + &p)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Function(f) => Function(ommx::Function::from(self.0.clone()) + f)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
        })
    }

    /// Reverse addition (lhs + self)
    #[gen_stub(skip)]
    pub fn __radd__(&self, py: Python<'_>, lhs: crate::FunctionInput) -> PyResult<Py<PyAny>> {
        self.py_add(py, lhs) // Addition is commutative
    }

    /// Polymorphic subtraction. See `py_add`.
    #[gen_stub(skip)]
    #[pyo3(name = "__sub__")]
    pub fn py_sub(&self, py: Python<'_>, rhs: crate::FunctionInput) -> PyResult<Py<PyAny>> {
        Ok(match rhs {
            crate::FunctionInput::Scalar(None) => Polynomial(self.0.clone())
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Scalar(Some(c)) => Polynomial(&self.0 - c)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Linear(l) => Polynomial(self.0.clone() - &l)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Quadratic(q) => Polynomial(self.0.clone() - &q)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Polynomial(p) => Polynomial(&self.0 - &p)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Function(f) => Function(ommx::Function::from(self.0.clone()) - f)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
        })
    }

    /// Reverse subtraction (lhs - self)
    #[gen_stub(skip)]
    pub fn __rsub__(&self, py: Python<'_>, lhs: crate::FunctionInput) -> PyResult<Py<PyAny>> {
        // lhs - self = -self + lhs
        let neg = self.__neg__();
        neg.py_add(py, lhs)
    }

    pub fn add_assign(&mut self, rhs: &Polynomial) {
        self.0 += &rhs.0;
    }

    /// In-place addition for += operator
    #[gen_stub(skip)]
    pub fn __iadd__(&mut self, rhs: &Polynomial) {
        self.0 += &rhs.0;
    }

    /// Polymorphic multiplication. See `py_add`.
    #[gen_stub(skip)]
    #[pyo3(name = "__mul__")]
    pub fn py_mul(&self, py: Python<'_>, rhs: crate::FunctionInput) -> PyResult<Py<PyAny>> {
        Ok(match rhs {
            crate::FunctionInput::Scalar(None) => Polynomial(ommx::Polynomial::default())
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Scalar(Some(c)) => Polynomial(self.0.clone() * c)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Linear(l) => Polynomial(&self.0 * &l)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Quadratic(q) => Polynomial(&self.0 * &q)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Polynomial(p) => Polynomial(&self.0 * &p)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Function(f) => Function(ommx::Function::from(self.0.clone()) * f)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
        })
    }

    /// Reverse multiplication (lhs * self)
    #[gen_stub(skip)]
    pub fn __rmul__(&self, py: Python<'_>, lhs: crate::FunctionInput) -> PyResult<Py<PyAny>> {
        self.py_mul(py, lhs) // Multiplication is commutative
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
        Ok(obj.cast::<PyDict>()?.clone())
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
    pub fn evaluate(&self, state: State, atol: Option<f64>) -> PyResult<f64> {
        use ommx::Evaluate;
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?,
            None => ommx::ATol::default(),
        };
        self.0
            .evaluate(&state.0, atol)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))
    }

    #[pyo3(signature = (state, *, atol=None))]
    pub fn partial_evaluate(&self, state: State, atol: Option<f64>) -> PyResult<Polynomial> {
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?,
            None => ommx::ATol::default(),
        };
        let mut inner = self.0.clone();
        inner
            .partial_evaluate(&state.0, atol)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
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

    /// Create an equality constraint: self == other → Constraint with EqualToZero
    #[gen_stub(type_ignore = ["override"])]
    #[pyo3(name = "__eq__")]
    pub fn py_eq(&self, other: Function) -> Constraint {
        let mut function = -other.0;
        function += &self.0;
        Constraint(
            ommx::Constraint {
                equality: ommx::Equality::EqualToZero,
                stage: ommx::CreatedData { function },
            },
            ommx::ConstraintMetadata::default(),
        )
    }

    /// Create a less-than-or-equal constraint: self <= other → Constraint
    #[pyo3(name = "__le__")]
    pub fn py_le(&self, other: Function) -> Constraint {
        let mut function = -other.0;
        function += &self.0;
        Constraint(
            ommx::Constraint {
                equality: ommx::Equality::LessThanOrEqualToZero,
                stage: ommx::CreatedData { function },
            },
            ommx::ConstraintMetadata::default(),
        )
    }

    /// Create a greater-than-or-equal constraint: self >= other → Constraint
    #[pyo3(name = "__ge__")]
    pub fn py_ge(&self, other: Function) -> Constraint {
        let function = other.0 - &self.0;
        Constraint(
            ommx::Constraint {
                equality: ommx::Equality::LessThanOrEqualToZero,
                stage: ommx::CreatedData { function },
            },
            ommx::ConstraintMetadata::default(),
        )
    }
}
