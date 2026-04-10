use crate::{
    next_constraint_id, Constraint, DecisionVariable, Function, Parameter, Polynomial, Quadratic,
    Rng, State,
};

use anyhow::{anyhow, Result};
use approx::AbsDiffEq;
use ommx::LinearMonomial;
use ommx::{ATol, Coefficient, CoefficientError, Evaluate};
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict},
    Bound, PyAny,
};
use std::collections::BTreeMap;

/// Linear function of decision variables.
///
/// A linear function has the form: $c_0 + \sum_i c_i x_i$ where $x_i$ are decision variables
/// and $c_i$ are coefficients.
///
/// # Examples
///
/// Create a linear function `f(x₁, x₂) = 2x₁ + 3x₂ + 1`:
///
/// ```python
/// >>> f = Linear(terms={1: 2, 2: 3}, constant=1)
/// ```
///
/// Or create via DecisionVariable arithmetic:
///
/// ```python
/// >>> x1 = DecisionVariable.integer(1)
/// >>> x2 = DecisionVariable.integer(2)
/// >>> g = 2*x1 + 3*x2 + 1
/// ```
///
/// Compare two linear functions with tolerance:
///
/// ```python
/// >>> f.almost_equal(g, atol=1e-12)
/// True
/// ```
///
/// Note that `==` creates an equality Constraint, not a boolean:
///
/// ```python
/// >>> constraint = f == g  # Returns Constraint, not bool
/// ```
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct Linear(pub ommx::Linear);

// Overload stubs for arithmetic operators.
// Must appear before #[gen_stub_pymethods] for correct ordering.
pyo3_stub_gen::inventory::submit! {
    pyo3_stub_gen::derive::gen_methods_from_python! {
        r#"
        class Linear:
            @overload
            def __add__(self, rhs: int | float | DecisionVariable | Parameter | Linear) -> Linear: ...
            @overload
            def __add__(self, rhs: Quadratic) -> Quadratic: ...
            @overload
            def __add__(self, rhs: Polynomial) -> Polynomial: ...

            @overload
            def __radd__(self, lhs: int | float | DecisionVariable | Parameter | Linear) -> Linear: ...
            @overload
            def __radd__(self, lhs: Quadratic) -> Quadratic: ...
            @overload
            def __radd__(self, lhs: Polynomial) -> Polynomial: ...

            @overload
            def __sub__(self, rhs: int | float | DecisionVariable | Parameter | Linear) -> Linear: ...
            @overload
            def __sub__(self, rhs: Quadratic) -> Quadratic: ...
            @overload
            def __sub__(self, rhs: Polynomial) -> Polynomial: ...

            @overload
            def __rsub__(self, lhs: int | float | DecisionVariable | Parameter | Linear) -> Linear: ...
            @overload
            def __rsub__(self, lhs: Quadratic) -> Quadratic: ...
            @overload
            def __rsub__(self, lhs: Polynomial) -> Polynomial: ...

            @overload
            def __mul__(self, rhs: int | float) -> Linear: ...
            @overload
            def __mul__(self, rhs: DecisionVariable | Parameter | Linear) -> Quadratic: ...
            @overload
            def __mul__(self, rhs: Quadratic | Polynomial) -> Polynomial: ...

            @overload
            def __rmul__(self, lhs: int | float) -> Linear: ...
            @overload
            def __rmul__(self, lhs: DecisionVariable | Parameter | Linear) -> Quadratic: ...
            @overload
            def __rmul__(self, lhs: Quadratic | Polynomial) -> Polynomial: ...

            def __iadd__(self, rhs: Linear) -> Linear: ...
        "#
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
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
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        Ok(Self(ommx::Linear::from_bytes(bytes.as_bytes())?))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.0.to_bytes())
    }

    #[getter]
    pub fn linear_terms(&self) -> BTreeMap<u64, f64> {
        self.0
            .iter()
            .filter_map(|(id, coeff)| match id {
                LinearMonomial::Variable(id) => Some((id.into_inner(), coeff.into_inner())),
                _ => None,
            })
            .collect()
    }

    #[getter]
    pub fn constant_term(&self) -> f64 {
        self.0
            .get(&LinearMonomial::Constant)
            .map(|coeff| coeff.into_inner())
            .unwrap_or(0.0)
    }

    #[pyo3(signature = (other, atol=ATol::default().into_inner()))]
    pub fn almost_equal(&self, other: &Linear, atol: f64) -> Result<bool> {
        Ok(self.0.abs_diff_eq(&other.0, ommx::ATol::new(atol)?))
    }

    pub fn __repr__(&self) -> String {
        format!("Linear({})", self.0)
    }

    /// Negation operator
    pub fn __neg__(&self) -> Linear {
        Linear(-self.0.clone())
    }

    /// Polymorphic addition: supports int, float, DecisionVariable, Linear
    /// Returns Linear when adding scalars or Linear, Quadratic otherwise
    #[gen_stub(skip)]
    #[pyo3(name = "__add__")]
    pub fn py_add(&self, py: Python<'_>, rhs: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        // Type check order: custom types first, then primitives
        if let Ok(linear) = rhs.extract::<PyRef<Linear>>() {
            return Ok(Linear(&self.0 + &linear.0)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        if let Ok(quad) = rhs.extract::<PyRef<Quadratic>>() {
            return Ok(Quadratic(&quad.0 + &self.0)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        if let Ok(poly) = rhs.extract::<PyRef<Polynomial>>() {
            return Ok(Polynomial(&poly.0 + &self.0)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract as Rust DecisionVariable directly
        if let Ok(dv) = rhs.extract::<PyRef<DecisionVariable>>() {
            let rhs_linear =
                ommx::Linear::single_term(LinearMonomial::Variable(dv.0.id()), ommx::coeff!(1.0));
            return Ok(Linear(&self.0 + &rhs_linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract as Rust Parameter directly
        if let Ok(param) = rhs.extract::<PyRef<Parameter>>() {
            let rhs_linear = ommx::Linear::single_term(
                LinearMonomial::Variable(ommx::VariableID::from(param.0.id)),
                ommx::coeff!(1.0),
            );
            return Ok(Linear(&self.0 + &rhs_linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract from Python wrapper (has .raw attribute)
        if let Ok(raw) = rhs.getattr("raw") {
            if let Ok(dv) = raw.extract::<PyRef<DecisionVariable>>() {
                let rhs_linear = ommx::Linear::single_term(
                    LinearMonomial::Variable(dv.0.id()),
                    ommx::coeff!(1.0),
                );
                return Ok(Linear(&self.0 + &rhs_linear)
                    .into_pyobject(py)?
                    .into_any()
                    .unbind());
            }
        }
        if let Ok(val) = rhs.extract::<f64>() {
            let linear = self
                .add_scalar(val)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
            return Ok(linear.into_pyobject(py)?.into_any().unbind());
        }
        // Return NotImplemented to allow Python to try the reflected operation
        Ok(py.NotImplemented().clone_ref(py).into_any())
    }

    /// Reverse addition (lhs + self)
    #[gen_stub(skip)]
    pub fn __radd__(&self, py: Python<'_>, lhs: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        self.py_add(py, lhs) // Addition is commutative
    }

    /// Polymorphic subtraction
    #[gen_stub(skip)]
    #[pyo3(name = "__sub__")]
    pub fn py_sub(&self, py: Python<'_>, rhs: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        if let Ok(linear) = rhs.extract::<PyRef<Linear>>() {
            return Ok(Linear(&self.0 - &linear.0)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        if let Ok(quad) = rhs.extract::<PyRef<Quadratic>>() {
            // self - quad = -quad + self
            let mut result = -quad.0.clone();
            result += &self.0;
            return Ok(Quadratic(result).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(poly) = rhs.extract::<PyRef<Polynomial>>() {
            let mut result = -poly.0.clone();
            result += &self.0;
            return Ok(Polynomial(result).into_pyobject(py)?.into_any().unbind());
        }
        // Try to extract as Rust DecisionVariable directly
        if let Ok(dv) = rhs.extract::<PyRef<DecisionVariable>>() {
            let rhs_linear =
                ommx::Linear::single_term(LinearMonomial::Variable(dv.0.id()), ommx::coeff!(1.0));
            return Ok(Linear(&self.0 - &rhs_linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract as Rust Parameter directly
        if let Ok(param) = rhs.extract::<PyRef<Parameter>>() {
            let rhs_linear = ommx::Linear::single_term(
                LinearMonomial::Variable(ommx::VariableID::from(param.0.id)),
                ommx::coeff!(1.0),
            );
            return Ok(Linear(&self.0 - &rhs_linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract from Python wrapper (has .raw attribute)
        if let Ok(raw) = rhs.getattr("raw") {
            if let Ok(dv) = raw.extract::<PyRef<DecisionVariable>>() {
                let rhs_linear = ommx::Linear::single_term(
                    LinearMonomial::Variable(dv.0.id()),
                    ommx::coeff!(1.0),
                );
                return Ok(Linear(&self.0 - &rhs_linear)
                    .into_pyobject(py)?
                    .into_any()
                    .unbind());
            }
        }
        if let Ok(val) = rhs.extract::<f64>() {
            let linear = self
                .add_scalar(-val)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
            return Ok(linear.into_pyobject(py)?.into_any().unbind());
        }
        // Return NotImplemented to allow Python to try the reflected operation
        Ok(py.NotImplemented().clone_ref(py).into_any())
    }

    /// Reverse subtraction (lhs - self)
    #[gen_stub(skip)]
    pub fn __rsub__(&self, py: Python<'_>, lhs: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        // lhs - self = -self + lhs
        let neg = self.__neg__();
        neg.py_add(py, lhs)
    }

    /// Polymorphic multiplication
    #[gen_stub(skip)]
    #[pyo3(name = "__mul__")]
    pub fn py_mul(&self, py: Python<'_>, rhs: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        if let Ok(linear) = rhs.extract::<PyRef<Linear>>() {
            return Ok(Quadratic(&self.0 * &linear.0)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        if let Ok(quad) = rhs.extract::<PyRef<Quadratic>>() {
            return Ok(Polynomial(&self.0 * &quad.0)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        if let Ok(poly) = rhs.extract::<PyRef<Polynomial>>() {
            return Ok(Polynomial(&self.0 * &poly.0)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract as Rust DecisionVariable directly
        if let Ok(dv) = rhs.extract::<PyRef<DecisionVariable>>() {
            let rhs_linear =
                ommx::Linear::single_term(LinearMonomial::Variable(dv.0.id()), ommx::coeff!(1.0));
            return Ok(Quadratic(&self.0 * &rhs_linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract as Rust Parameter directly
        if let Ok(param) = rhs.extract::<PyRef<Parameter>>() {
            let rhs_linear = ommx::Linear::single_term(
                LinearMonomial::Variable(ommx::VariableID::from(param.0.id)),
                ommx::coeff!(1.0),
            );
            return Ok(Quadratic(&self.0 * &rhs_linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract from Python wrapper (has .raw attribute)
        if let Ok(raw) = rhs.getattr("raw") {
            if let Ok(dv) = raw.extract::<PyRef<DecisionVariable>>() {
                let rhs_linear = ommx::Linear::single_term(
                    LinearMonomial::Variable(dv.0.id()),
                    ommx::coeff!(1.0),
                );
                return Ok(Quadratic(&self.0 * &rhs_linear)
                    .into_pyobject(py)?
                    .into_any()
                    .unbind());
            }
        }
        if let Ok(val) = rhs.extract::<f64>() {
            let linear = self
                .mul_scalar(val)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
            return Ok(linear.into_pyobject(py)?.into_any().unbind());
        }
        // Return NotImplemented to allow Python to try the reflected operation
        Ok(py.NotImplemented().clone_ref(py).into_any())
    }

    /// Reverse multiplication (lhs * self)
    #[gen_stub(skip)]
    pub fn __rmul__(&self, py: Python<'_>, lhs: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        self.py_mul(py, lhs) // Multiplication is commutative
    }

    pub fn add_assign(&mut self, rhs: &Linear) {
        self.0 += &rhs.0;
    }

    /// In-place addition for += operator
    #[gen_stub(skip)]
    pub fn __iadd__(&mut self, rhs: &Linear) {
        self.0 += &rhs.0;
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

    pub fn terms<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let obj = serde_pyobject::to_pyobject(py, &self.0)?;
        Ok(obj.cast::<PyDict>()?.clone())
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
    pub fn partial_evaluate(&self, state: State, atol: Option<f64>) -> PyResult<Linear> {
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?,
            None => ommx::ATol::default(),
        };
        let mut inner = self.0.clone();
        inner
            .partial_evaluate(&state.0, atol)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
        Ok(Linear(inner))
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
        // self - other == 0
        let mut function = -other.0;
        function += &self.0;
        let id = next_constraint_id();
        Constraint(ommx::Constraint {
            id: ommx::ConstraintID::from(id),
            function,
            equality: ommx::Equality::EqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: Default::default(),
            description: None,
        })
    }

    /// Create a less-than-or-equal constraint: self <= other → Constraint
    #[pyo3(name = "__le__")]
    pub fn py_le(&self, other: Function) -> Constraint {
        // self - other <= 0: compute as -(other) + self
        let mut function = -other.0;
        function += &self.0;
        let id = next_constraint_id();
        Constraint(ommx::Constraint {
            id: ommx::ConstraintID::from(id),
            function,
            equality: ommx::Equality::LessThanOrEqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: Default::default(),
            description: None,
        })
    }

    /// Create a greater-than-or-equal constraint: self >= other → Constraint
    #[pyo3(name = "__ge__")]
    pub fn py_ge(&self, other: Function) -> Constraint {
        // self >= other ⇔ other - self <= 0
        let function = other.0 - &self.0;
        let id = next_constraint_id();
        Constraint(ommx::Constraint {
            id: ommx::ConstraintID::from(id),
            function,
            equality: ommx::Equality::LessThanOrEqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: Default::default(),
            description: None,
        })
    }
}
