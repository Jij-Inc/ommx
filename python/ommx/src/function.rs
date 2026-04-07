use crate::{
    next_constraint_id, Constraint, DecisionVariable, Linear, Parameter, Polynomial, Quadratic,
    Rng, State,
};

use anyhow::{anyhow, Result};
use approx::AbsDiffEq;
use ommx::{ATol, Coefficient, CoefficientError, Evaluate, LinearMonomial};
use pyo3::{
    exceptions::PyTypeError,
    prelude::*,
    types::{PyBytes, PyDict},
    Bound, PyAny,
};
use std::collections::{BTreeMap, BTreeSet};

/// General mathematical function of decision variables.
///
/// Function is a unified type that can represent constant, linear, quadratic,
/// or polynomial functions. It is used as the objective function and constraint
/// functions in optimization problems.
///
/// Example
/// -------
/// Create from various types:
///
/// >>> f = Function(1.0)  # Constant
/// >>> f = Function(Linear(terms={1: 2}, constant=1))  # Linear
/// >>> f = Function(x * y)  # From Quadratic expression
///
/// Access the terms:
///
/// >>> f = Function(Linear(terms={1: 2.5}, constant=1.0))
/// >>> f.terms
/// {(1,): 2.5, (): 1.0}
///
/// Check the degree:
///
/// >>> f.degree()
/// 1
///
/// .
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct Function(pub ommx::Function);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Function {
    /// Create a Function from various types.
    ///
    /// Accepts:
    /// - int or float: creates a constant function
    /// - DecisionVariable: creates a linear function with single term
    /// - Linear: creates a linear function
    /// - Quadratic: creates a quadratic function
    /// - Polynomial: creates a polynomial function
    /// - Function: returns a copy
    #[new]
    pub fn new(inner: &Bound<PyAny>) -> PyResult<Self> {
        // Try to extract as Function first (check if it's already our type)
        if inner.is_instance_of::<Self>() {
            return Ok(inner.extract::<Self>()?);
        }
        // Also try direct extraction in case the type check fails
        if let Ok(f) = inner.extract::<Self>() {
            return Ok(f);
        }
        // Try to extract as Polynomial
        if let Ok(p) = inner.extract::<Polynomial>() {
            return Ok(Self(ommx::Function::from(p.0)));
        }
        // Try to extract as Quadratic
        if let Ok(q) = inner.extract::<Quadratic>() {
            return Ok(Self(ommx::Function::from(q.0)));
        }
        // Try to extract as Linear
        if let Ok(l) = inner.extract::<Linear>() {
            return Ok(Self(ommx::Function::from(l.0)));
        }
        // Try to extract as Rust DecisionVariable directly
        if let Ok(dv) = inner.extract::<DecisionVariable>() {
            let linear =
                ommx::Linear::single_term(LinearMonomial::Variable(dv.0.id()), ommx::coeff!(1.0));
            return Ok(Self(ommx::Function::from(linear)));
        }
        // Try to extract as Rust Parameter directly
        if let Ok(param) = inner.extract::<Parameter>() {
            let linear = ommx::Linear::single_term(
                LinearMonomial::Variable(ommx::VariableID::from(param.0.id)),
                ommx::coeff!(1.0),
            );
            return Ok(Self(ommx::Function::from(linear)));
        }
        // Try to extract from Python wrapper (has .raw attribute)
        if let Ok(raw) = inner.getattr("raw") {
            if let Ok(dv) = raw.extract::<DecisionVariable>() {
                let linear = ommx::Linear::single_term(
                    LinearMonomial::Variable(dv.0.id()),
                    ommx::coeff!(1.0),
                );
                return Ok(Self(ommx::Function::from(linear)));
            }
        }
        // Try to extract as float
        if let Ok(scalar) = inner.extract::<f64>() {
            return match TryInto::<Coefficient>::try_into(scalar) {
                Ok(coeff) => Ok(Self(ommx::Function::from(coeff))),
                Err(CoefficientError::Zero) => Ok(Self(ommx::Function::default())),
                Err(e) => Err(PyTypeError::new_err(e.to_string())),
            };
        }
        // Try to extract as protobuf message (has SerializeToString method)
        if let Ok(serialize_method) = inner.getattr("SerializeToString") {
            if let Ok(bytes) = serialize_method.call0() {
                if let Ok(bytes_data) = bytes.extract::<Vec<u8>>() {
                    return ommx::Function::from_bytes(&bytes_data)
                        .map(Self)
                        .map_err(|e| PyTypeError::new_err(e.to_string()));
                }
            }
        }
        Err(PyTypeError::new_err(format!(
            "Cannot create Function from {}",
            inner.get_type().name()?
        )))
    }

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

    /// Negation operator
    pub fn __neg__(&self) -> Function {
        Function(-self.0.clone())
    }

    /// Polymorphic addition: supports int, float, DecisionVariable, Linear, Quadratic, Polynomial, Function
    #[pyo3(name = "__add__")]
    pub fn py_add(&self, py: Python<'_>, rhs: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        // Type check order: custom types first, then primitives
        if let Ok(func) = rhs.extract::<PyRef<Function>>() {
            return Ok(Function(&self.0 + &func.0)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        if let Ok(poly) = rhs.extract::<PyRef<Polynomial>>() {
            return Ok(self
                .add_polynomial(&poly)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        if let Ok(quad) = rhs.extract::<PyRef<Quadratic>>() {
            return Ok(self
                .add_quadratic(&quad)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        if let Ok(linear) = rhs.extract::<PyRef<Linear>>() {
            return Ok(self
                .add_linear(&linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract as Rust DecisionVariable directly
        if let Ok(dv) = rhs.extract::<PyRef<DecisionVariable>>() {
            // DecisionVariable → Linear(id, 1) conversion
            let linear =
                ommx::Linear::single_term(LinearMonomial::Variable(dv.0.id()), ommx::coeff!(1.0));
            return Ok(Function(&self.0 + &linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract as Rust Parameter directly
        if let Ok(param) = rhs.extract::<PyRef<Parameter>>() {
            let linear = ommx::Linear::single_term(
                LinearMonomial::Variable(ommx::VariableID::from(param.0.id)),
                ommx::coeff!(1.0),
            );
            return Ok(Function(&self.0 + &linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract from Python wrapper (has .raw attribute)
        if let Ok(raw) = rhs.getattr("raw") {
            if let Ok(dv) = raw.extract::<PyRef<DecisionVariable>>() {
                let linear = ommx::Linear::single_term(
                    LinearMonomial::Variable(dv.0.id()),
                    ommx::coeff!(1.0),
                );
                return Ok(Function(&self.0 + &linear)
                    .into_pyobject(py)?
                    .into_any()
                    .unbind());
            }
        }
        if let Ok(val) = rhs.extract::<f64>() {
            let func = self
                .add_scalar(val)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
            return Ok(func.into_pyobject(py)?.into_any().unbind());
        }
        // Return NotImplemented to allow Python to try the reflected operation
        Ok(py.NotImplemented().clone_ref(py).into_any())
    }

    /// Reverse addition (lhs + self)
    pub fn __radd__(&self, py: Python<'_>, lhs: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        self.py_add(py, lhs) // Addition is commutative
    }

    /// Polymorphic subtraction: supports int, float, DecisionVariable, Linear, Quadratic, Polynomial, Function
    #[pyo3(name = "__sub__")]
    pub fn py_sub(&self, py: Python<'_>, rhs: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        // self - rhs: clone self and use owned subtraction
        // Type check order: custom types first, then primitives
        if let Ok(func) = rhs.extract::<PyRef<Function>>() {
            return Ok(Function(&self.0 - &func.0)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        if let Ok(poly) = rhs.extract::<PyRef<Polynomial>>() {
            return Ok(Function(self.0.clone() - poly.0.clone())
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        if let Ok(quad) = rhs.extract::<PyRef<Quadratic>>() {
            return Ok(Function(self.0.clone() - quad.0.clone())
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        if let Ok(linear) = rhs.extract::<PyRef<Linear>>() {
            return Ok(Function(self.0.clone() - linear.0.clone())
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract as Rust DecisionVariable directly
        if let Ok(dv) = rhs.extract::<PyRef<DecisionVariable>>() {
            let linear =
                ommx::Linear::single_term(LinearMonomial::Variable(dv.0.id()), ommx::coeff!(1.0));
            return Ok(Function(self.0.clone() - linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract as Rust Parameter directly
        if let Ok(param) = rhs.extract::<PyRef<Parameter>>() {
            let linear = ommx::Linear::single_term(
                LinearMonomial::Variable(ommx::VariableID::from(param.0.id)),
                ommx::coeff!(1.0),
            );
            return Ok(Function(self.0.clone() - linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract from Python wrapper (has .raw attribute)
        if let Ok(raw) = rhs.getattr("raw") {
            if let Ok(dv) = raw.extract::<PyRef<DecisionVariable>>() {
                let linear = ommx::Linear::single_term(
                    LinearMonomial::Variable(dv.0.id()),
                    ommx::coeff!(1.0),
                );
                return Ok(Function(self.0.clone() - linear)
                    .into_pyobject(py)?
                    .into_any()
                    .unbind());
            }
        }
        if let Ok(val) = rhs.extract::<f64>() {
            let func = self
                .add_scalar(-val)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
            return Ok(func.into_pyobject(py)?.into_any().unbind());
        }
        // Return NotImplemented to allow Python to try the reflected operation
        Ok(py.NotImplemented().clone_ref(py).into_any())
    }

    /// Reverse subtraction (lhs - self)
    pub fn __rsub__(&self, py: Python<'_>, lhs: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        // lhs - self = -self + lhs
        self.__neg__().py_add(py, lhs)
    }

    pub fn add_assign(&mut self, rhs: &Function) {
        self.0 += &rhs.0;
    }

    /// In-place addition for += operator
    ///
    /// Note: This returns `()` in Rust, but PyO3 automatically returns `self` to Python.
    /// See https://github.com/PyO3/pyo3/issues/4605 for details.
    pub fn __iadd__(&mut self, rhs: &Function) {
        self.0 += &rhs.0;
    }

    /// Polymorphic multiplication: supports int, float, DecisionVariable, Linear, Quadratic, Polynomial, Function
    #[pyo3(name = "__mul__")]
    pub fn py_mul(&self, py: Python<'_>, rhs: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        // Type check order: custom types first, then primitives
        if let Ok(func) = rhs.extract::<PyRef<Function>>() {
            return Ok(Function(&self.0 * &func.0)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        if let Ok(poly) = rhs.extract::<PyRef<Polynomial>>() {
            return Ok(self
                .mul_polynomial(&poly)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        if let Ok(quad) = rhs.extract::<PyRef<Quadratic>>() {
            return Ok(self
                .mul_quadratic(&quad)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        if let Ok(linear) = rhs.extract::<PyRef<Linear>>() {
            return Ok(self
                .mul_linear(&linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract as Rust DecisionVariable directly
        if let Ok(dv) = rhs.extract::<PyRef<DecisionVariable>>() {
            let linear =
                ommx::Linear::single_term(LinearMonomial::Variable(dv.0.id()), ommx::coeff!(1.0));
            return Ok(Function(&self.0 * &linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract as Rust Parameter directly
        if let Ok(param) = rhs.extract::<PyRef<Parameter>>() {
            let linear = ommx::Linear::single_term(
                LinearMonomial::Variable(ommx::VariableID::from(param.0.id)),
                ommx::coeff!(1.0),
            );
            return Ok(Function(&self.0 * &linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract from Python wrapper (has .raw attribute)
        if let Ok(raw) = rhs.getattr("raw") {
            if let Ok(dv) = raw.extract::<PyRef<DecisionVariable>>() {
                let linear = ommx::Linear::single_term(
                    LinearMonomial::Variable(dv.0.id()),
                    ommx::coeff!(1.0),
                );
                return Ok(Function(&self.0 * &linear)
                    .into_pyobject(py)?
                    .into_any()
                    .unbind());
            }
        }
        if let Ok(val) = rhs.extract::<f64>() {
            let func = self
                .mul_scalar(val)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
            return Ok(func.into_pyobject(py)?.into_any().unbind());
        }
        // Return NotImplemented to allow Python to try the reflected operation
        Ok(py.NotImplemented().clone_ref(py).into_any())
    }

    /// Reverse multiplication (lhs * self)
    pub fn __rmul__(&self, py: Python<'_>, lhs: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        self.py_mul(py, lhs) // Multiplication is commutative
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

    #[getter]
    pub fn terms<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let obj = serde_pyobject::to_pyobject(py, &self.0)?;
        Ok(obj.cast::<PyDict>()?.clone())
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

    #[pyo3(signature = (state, *, atol=None))]
    pub fn evaluate(&self, state: &Bound<PyAny>, atol: Option<f64>) -> PyResult<f64> {
        use ommx::Evaluate;
        let state = State::new(state)?;
        let atol = match atol {
            Some(value) => {
                ommx::ATol::new(value).map_err(|e| PyTypeError::new_err(e.to_string()))?
            }
            None => ommx::ATol::default(),
        };
        self.0
            .evaluate(&state.0, atol)
            .map_err(|e| PyTypeError::new_err(e.to_string()))
    }

    #[pyo3(signature = (state, *, atol=None))]
    pub fn partial_evaluate(&self, state: &Bound<PyAny>, atol: Option<f64>) -> PyResult<Function> {
        let state = State::new(state)?;
        let atol = match atol {
            Some(value) => {
                ommx::ATol::new(value).map_err(|e| PyTypeError::new_err(e.to_string()))?
            }
            None => ommx::ATol::default(),
        };
        let mut inner = self.0.clone();
        inner
            .partial_evaluate(&state.0, atol)
            .map_err(|e| PyTypeError::new_err(e.to_string()))?;
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

    /// Create an equality constraint: self == other → Constraint with EqualToZero
    ///
    /// Returns a Constraint where (self - other) == 0.
    /// Note: This does NOT return bool, it creates a Constraint object.
    #[gen_stub(type_ignore = ["override"])]
    #[pyo3(name = "__eq__")]
    pub fn py_eq(&self, py: Python<'_>, other: &Bound<PyAny>) -> PyResult<Constraint> {
        let diff = self.py_sub(py, other)?;
        let function = extract_to_function(py, diff)?;
        let id = next_constraint_id();
        Ok(Constraint(ommx::Constraint {
            id: ommx::ConstraintID::from(id),
            function,
            equality: ommx::Equality::EqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: Default::default(),
            description: None,
        }))
    }

    /// Create a less-than-or-equal constraint: self <= other → Constraint with LessThanOrEqualToZero
    ///
    /// Returns a Constraint where (self - other) <= 0.
    #[pyo3(name = "__le__")]
    pub fn py_le(&self, py: Python<'_>, other: &Bound<PyAny>) -> PyResult<Constraint> {
        let diff = self.py_sub(py, other)?;
        let function = extract_to_function(py, diff)?;
        let id = next_constraint_id();
        Ok(Constraint(ommx::Constraint {
            id: ommx::ConstraintID::from(id),
            function,
            equality: ommx::Equality::LessThanOrEqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: Default::default(),
            description: None,
        }))
    }

    /// Create a greater-than-or-equal constraint: self >= other → Constraint with LessThanOrEqualToZero
    ///
    /// Returns a Constraint where (other - self) <= 0.
    #[pyo3(name = "__ge__")]
    pub fn py_ge(&self, py: Python<'_>, other: &Bound<PyAny>) -> PyResult<Constraint> {
        // self >= other is equivalent to other - self <= 0
        // But we need to express other as a Function first
        let neg_self = self.__neg__();
        let diff = neg_self.py_add(py, other)?;
        let function = extract_to_function(py, diff)?;
        let id = next_constraint_id();
        Ok(Constraint(ommx::Constraint {
            id: ommx::ConstraintID::from(id),
            function,
            equality: ommx::Equality::LessThanOrEqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: Default::default(),
            description: None,
        }))
    }
}

/// Helper function to extract a PyAny result into ommx::Function
fn extract_to_function(py: Python<'_>, obj: Py<PyAny>) -> PyResult<ommx::Function> {
    if let Ok(func) = obj.extract::<Function>(py) {
        return Ok(func.0);
    }
    if let Ok(poly) = obj.extract::<Polynomial>(py) {
        return Ok(ommx::Function::from(poly.0));
    }
    if let Ok(quad) = obj.extract::<Quadratic>(py) {
        return Ok(ommx::Function::from(quad.0));
    }
    if let Ok(linear) = obj.extract::<Linear>(py) {
        return Ok(ommx::Function::from(linear.0));
    }
    Err(PyTypeError::new_err(
        "Cannot convert to Function: expected Linear, Quadratic, Polynomial, or Function",
    ))
}
