use crate::{
    AttachedDecisionVariable, Constraint, DecisionVariable, Linear, Parameter, Polynomial,
    Quadratic, Rng, State, VariableBound,
};

use anyhow::{anyhow, Result};
use approx::AbsDiffEq;
use ommx::{ATol, Coefficient, CoefficientError, Evaluate, LinearMonomial};
use pyo3::{exceptions::PyTypeError, prelude::*, types::PyDict, Bound, PyAny};
use std::collections::{BTreeMap, BTreeSet};

/// General mathematical function of decision variables.
///
/// Function is a unified type that can represent constant, linear, quadratic,
/// or polynomial functions. It is used as the objective function and constraint
/// functions in optimization problems.
///
/// # Examples
///
/// Create from various types:
///
/// ```python
/// >>> f = Function(1.0)  # Constant
/// >>> f = Function(Linear(terms={1: 2}, constant=1))  # Linear
/// >>> f = Function(x * y)  # From Quadratic expression
/// ```
///
/// Access the terms:
///
/// ```python
/// >>> f = Function(Linear(terms={1: 2.5}, constant=1.0))
/// >>> f.terms
/// {(1,): 2.5, (): 1.0}
/// ```
///
/// Check the degree:
///
/// ```python
/// >>> f.degree()
/// 1
/// ```
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct Function(pub ommx::Function);

// Manual PyClassInfo submission (instead of #[gen_stub_pyclass])
pyo3_stub_gen::inventory::submit! {
    pyo3_stub_gen::type_info::PyClassInfo {
        pyclass_name: "Function",
        struct_id: || std::any::TypeId::of::<Function>(),
        doc: "General mathematical function of decision variables.",
        module: Some("ommx._ommx_rust"),
        bases: &[],
        getters: &[],
        setters: &[],
        has_eq: false,
        has_hash: false,
        has_ord: false,
        has_str: false,
        subclass: false,
    }
}

// PyStubType: input uses ToFunction, output uses Function
impl pyo3_stub_gen::PyStubType for Function {
    fn type_input() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo::locally_defined("ToFunction", "ommx._ommx_rust".into())
    }
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo::locally_defined("Function", "ommx._ommx_rust".into())
    }
}

/// Internal enum used by polymorphic arithmetic operators on
/// `DecisionVariable` / `AttachedDecisionVariable` / `Parameter` / `Linear` / `Quadratic` /
/// `Polynomial` to dispatch on the rhs's *operand class* without duplicating the type-extraction
/// logic across files.
///
/// Variants are kept distinct so each operator can pick its return type from the rhs class
/// alone (e.g. `Linear * Scalar -> Linear` but `Linear * Linear -> Quadratic`). `Scalar` and
/// `Linear` are deliberately separate even though both have degree ≤ 1, because multiplication
/// behaves differently for them.
///
/// `Function` is the opaque catch-all: when the rhs is an explicit `Function` instance, the
/// result is also `Function` (matching today's `__radd__`-fallback behavior).
///
/// Adding a new Function-extractable type only requires:
///   1. one new branch in `FromPyObject for FunctionInput` (or fitting it under an existing
///      variant via `Linear::single_term`), and
///   2. a match arm — usually just reusing the existing `Linear` arm — in each polymorphic op.
pub enum FunctionInput {
    /// `int` / `float` / `numpy.integer` / `numpy.floating`. `None` represents zero
    /// (which `ommx::Coefficient` cannot hold).
    Scalar(Option<Coefficient>),
    /// Anything of degree ≤ 1: `DecisionVariable`, `AttachedDecisionVariable`, `Parameter`, `Linear`.
    Linear(ommx::Linear),
    Quadratic(ommx::Quadratic),
    Polynomial(ommx::Polynomial),
    /// Opaque `Function`. Polymorphic operators preserve the `Function` shape on output.
    Function(ommx::Function),
}

impl FunctionInput {
    pub(crate) fn into_function(self) -> ommx::Function {
        match self {
            Self::Scalar(None) => ommx::Function::default(),
            Self::Scalar(Some(c)) => ommx::Function::from(c),
            Self::Linear(l) => ommx::Function::from(l),
            Self::Quadratic(q) => ommx::Function::from(q),
            Self::Polynomial(p) => ommx::Function::from(p),
            Self::Function(f) => f,
        }
    }
}

/// Extract a Python value as one of the `ToFunction`-supported types.
///
/// Order is significant: more specific types are tried first so that, e.g., a `Polynomial` is
/// not accidentally extracted as a `Function` via `Function::FromPyObject` recursion. The
/// `Function` branch uses `cast` (not `extract`) for the same reason.
impl<'py> FromPyObject<'_, 'py> for FunctionInput {
    type Error = PyErr;
    fn extract(ob: Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        if let Ok(f) = ob.cast::<Function>() {
            return Ok(Self::Function(f.borrow().0.clone()));
        }
        if let Ok(p) = ob.extract::<PyRef<Polynomial>>() {
            return Ok(Self::Polynomial(p.0.clone()));
        }
        if let Ok(q) = ob.extract::<PyRef<Quadratic>>() {
            return Ok(Self::Quadratic(q.0.clone()));
        }
        if let Ok(l) = ob.extract::<PyRef<Linear>>() {
            return Ok(Self::Linear(l.0.clone()));
        }
        if let Ok(dv) = ob.extract::<PyRef<DecisionVariable>>() {
            return Ok(Self::Linear(ommx::Linear::single_term(
                LinearMonomial::Variable(dv.0.id()),
                ommx::coeff!(1.0),
            )));
        }
        if let Ok(att) = ob.extract::<PyRef<AttachedDecisionVariable>>() {
            return Ok(Self::Linear(ommx::Linear::single_term(
                LinearMonomial::Variable(att.id),
                ommx::coeff!(1.0),
            )));
        }
        if let Ok(param) = ob.extract::<PyRef<Parameter>>() {
            return Ok(Self::Linear(ommx::Linear::single_term(
                LinearMonomial::Variable(ommx::VariableID::from(param.0.id)),
                ommx::coeff!(1.0),
            )));
        }
        if let Ok(scalar) = ob.extract::<f64>() {
            return match TryInto::<Coefficient>::try_into(scalar) {
                Ok(c) => Ok(Self::Scalar(Some(c))),
                Err(CoefficientError::Zero) => Ok(Self::Scalar(None)),
                Err(e) => Err(PyTypeError::new_err(e.to_string())),
            };
        }
        Err(PyTypeError::new_err(format!(
            "Cannot convert {} to ToFunction. Accepted: int, float, DecisionVariable, AttachedDecisionVariable, Parameter, Linear, Quadratic, Polynomial, Function",
            ob.get_type().name()?
        )))
    }
}

/// `Function::FromPyObject` is the user-facing `ToFunction` extraction. It collapses the
/// per-class detail of `FunctionInput` into a single `ommx::Function`, which is what
/// `Function`-typed parameters (e.g. `Function::__add__(rhs: Function)`) want.
impl<'py> FromPyObject<'_, 'py> for Function {
    type Error = PyErr;
    fn extract(ob: Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        let input = FunctionInput::extract(ob)?;
        Ok(Self(input.into_function()))
    }
}

pyo3_stub_gen::impl_py_runtime_type!(Function);

// Marker types for numpy scalar types in stubs
macro_rules! numpy_stub_marker {
    ($marker:ident, $numpy_name:expr, $numpy_attr:expr) => {
        pub struct $marker;
        impl pyo3_stub_gen::PyStubType for $marker {
            fn type_output() -> pyo3_stub_gen::TypeInfo {
                pyo3_stub_gen::TypeInfo {
                    name: format!("numpy.{}", $numpy_name),
                    source_module: None,
                    import: std::collections::HashSet::from(["numpy".into()]),
                    type_refs: std::collections::HashMap::new(),
                }
            }
        }
        impl pyo3_stub_gen::runtime::PyRuntimeType for $marker {
            fn runtime_type_object(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
                let numpy = py.import("numpy")?;
                numpy.getattr($numpy_attr)
            }
        }
    };
}
numpy_stub_marker!(NumpyInteger, "integer", "integer");
numpy_stub_marker!(NumpyFloating, "floating", "floating");

// Type alias: Scalar = int | float | numpy.integer | numpy.floating
pyo3_stub_gen::type_alias!(
    "ommx._ommx_rust",
    Scalar = i64 | f64 | NumpyInteger | NumpyFloating
);

// Type alias: LinearLike = Linear | DecisionVariable | AttachedDecisionVariable
//
// Note: `Parameter` is intentionally NOT a member of `LinearLike`. Parameters are bound only
// in `ParametricInstance` and represent values fixed before optimization, not optimization
// variables — so they appear separately in `ToFunction` and in arithmetic overloads where a
// caller might pass them.
pyo3_stub_gen::type_alias!(
    "ommx._ommx_rust",
    LinearLike = Linear | DecisionVariable | AttachedDecisionVariable
);

// Type alias: ToFunction = Scalar | LinearLike | Parameter | Quadratic | Polynomial | Function
pyo3_stub_gen::type_alias!(
    "ommx._ommx_rust",
    ToFunction = i64
        | f64
        | NumpyInteger
        | NumpyFloating
        | DecisionVariable
        | AttachedDecisionVariable
        | Parameter
        | Linear
        | Quadratic
        | Polynomial
        | Function
);

// Manual stub for __iadd__ (PyO3 returns () but Python returns self)
pyo3_stub_gen::inventory::submit! {
    pyo3_stub_gen::derive::gen_methods_from_python! {
        r#"
        class Function:
            def __iadd__(self, rhs: ToFunction) -> Function: ...
        "#
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Function {
    /// Create a Function from various types.
    ///
    /// Accepts:
    /// - int or float: creates a constant function
    /// - DecisionVariable: creates a linear function with single term
    /// - Parameter: creates a linear function with single term
    /// - Linear: creates a linear function
    /// - Quadratic: creates a quadratic function
    /// - Polynomial: creates a polynomial function
    /// - Function: returns a copy
    #[new]
    pub fn new(inner: Function) -> Self {
        inner
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

    /// Addition
    pub fn __add__(&self, rhs: Function) -> Function {
        Function(&self.0 + &rhs.0)
    }

    /// Reverse addition (lhs + self)
    pub fn __radd__(&self, lhs: Function) -> Function {
        Function(&self.0 + &lhs.0)
    }

    /// Subtraction
    pub fn __sub__(&self, rhs: Function) -> Function {
        Function(&self.0 - &rhs.0)
    }

    /// Reverse subtraction (lhs - self)
    pub fn __rsub__(&self, lhs: Function) -> Function {
        Function(&lhs.0 - &self.0)
    }

    pub fn add_assign(&mut self, rhs: &Function) {
        self.0 += &rhs.0;
    }

    /// In-place addition for += operator
    #[gen_stub(skip)]
    pub fn __iadd__(&mut self, rhs: &Function) {
        self.0 += &rhs.0;
    }

    /// Multiplication
    pub fn __mul__(&self, rhs: Function) -> Function {
        Function(&self.0 * &rhs.0)
    }

    /// Reverse multiplication (lhs * self)
    pub fn __rmul__(&self, lhs: Function) -> Function {
        Function(&self.0 * &lhs.0)
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
    pub fn partial_evaluate(&self, state: State, atol: Option<f64>) -> PyResult<Function> {
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?,
            None => ommx::ATol::default(),
        };
        let mut inner = self.0.clone();
        inner
            .partial_evaluate(&state.0, atol)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
        Ok(Function(inner))
    }

    /// Compute an interval bound of this function given variable bounds.
    ///
    /// Missing IDs in `bounds` are treated as unbounded (`Bound.unbounded()`).
    ///
    /// **Args:**
    ///
    /// - `bounds`: Mapping from variable ID to its {class}`~ommx.v1.Bound`.
    ///
    /// **Returns:** A {class}`~ommx.v1.Bound` that contains $[\inf f, \sup f]$ over the given variable bounds.
    ///
    /// **Tightness:** This evaluates the bound **term by term** (monomial-wise)
    /// and sums the per-term intervals. The result is a **sound
    /// over-approximation** of the true range $[\inf f, \sup f]$ but is **not
    /// guaranteed to be tight**, because it ignores dependencies between terms
    /// that share variables. For example, $f = x^2 - x$ with $x \in [0, 1]$
    /// has true range $[-1/4, 0]$ (minimum at $x = 1/2$), but term-wise
    /// evaluation yields $[0, 1] + (-[0, 1]) = [-1, 1]$.
    ///
    /// # Examples
    ///
    /// ```python
    /// >>> from ommx.v1 import Function, Linear, Bound
    /// >>> f = Function(Linear(terms={1: 2}, constant=3))  # 2*x1 + 3
    /// >>> b = f.evaluate_bound({1: Bound(0.0, 2.0)})
    /// >>> (b.lower, b.upper)
    /// (3.0, 7.0)
    /// ```
    pub fn evaluate_bound(&self, bounds: BTreeMap<u64, VariableBound>) -> VariableBound {
        let bounds: ommx::Bounds = bounds
            .into_iter()
            .map(|(id, b)| (ommx::VariableID::from(id), b.0))
            .collect();
        VariableBound(self.0.evaluate_bound(&bounds))
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
    /// For binary variables, $x^n = x$ for any $n \geq 1$, so we can reduce higher powers to linear terms.
    ///
    /// **Args:**
    ///
    /// - `binary_ids`: Set of binary variable IDs to reduce powers for
    ///
    /// **Returns:** `True` if any reduction was performed, `False` otherwise
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

    /// Create a less-than-or-equal constraint: self <= other → Constraint with LessThanOrEqualToZero
    ///
    /// Returns a Constraint where (self - other) <= 0.
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

    /// Create a greater-than-or-equal constraint: self >= other → Constraint with LessThanOrEqualToZero
    ///
    /// Returns a Constraint where (other - self) <= 0.
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
