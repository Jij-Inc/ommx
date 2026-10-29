use crate::{
    AttachedDecisionVariable, Constraint, DecisionVariable, Linear, Parameter, Polynomial,
    Quadratic, Rng, State, VariableBound,
};

use approx::AbsDiffEq;
use ommx::{ATol, Coefficient, CoefficientError, Evaluate, LinearMonomial};
use pyo3::{
    exceptions::PyTypeError,
    prelude::*,
    types::{PyDict, PyTuple},
    Bound, PyAny,
};
use std::collections::{BTreeMap, BTreeSet};

#[doc = include_str!("docstrings/ommx/Function.md")]
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct Function(pub ommx::Function);

// Manual PyClassInfo submission (instead of #[gen_stub_pyclass])
pyo3_stub_gen::inventory::submit! {
    pyo3_stub_gen::type_info::PyClassInfo {
        pyclass_name: "Function",
        struct_id: || std::any::TypeId::of::<Function>(),
        doc: include_str!("docstrings/ommx/Function.md"),
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
/// alone (e.g. `Linear * Scalar -> Linear` but `Linear * Linear -> Quadratic`). The `Scalar`
/// and `Linear` variants are deliberately separate even though both have degree ≤ 1, because
/// multiplication behaves differently for them.
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
/// The `Function` branch is checked first via `cast` (not `extract`) so an actual `Function`
/// instance short-circuits the dispatch — using `extract` would invoke
/// `Function::FromPyObject` (which itself calls back into this impl) and recurse. After
/// that, more specific arithmetic types (`Polynomial` → `Quadratic` → `Linear`) are tried
/// before degree-≤-1 wrappers (`DecisionVariable`, `AttachedDecisionVariable`, `Parameter`)
/// and finally `f64` for scalars.
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
                LinearMonomial::Variable(dv.0),
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
                // NaN / Inf. The error class is not user-observable: PyO3 catches
                // any parameter-extraction failure on a binop and converts it to
                // `NotImplemented`, after which Python raises its generic
                // `TypeError("unsupported operand types")`. This is a minor
                // behavior change vs. pre-refactor (which raised `ValueError`
                // from the operator body) — accepted as part of the typed-
                // `FunctionInput` parameter design in v3.
                Err(e) => Err(PyTypeError::new_err(e.to_string())),
            };
        }
        Err(PyTypeError::new_err(format!(
            "Cannot convert {} to ToFunction. Accepted: int, float, numpy.integer, numpy.floating, DecisionVariable, AttachedDecisionVariable, Parameter, Linear, Quadratic, Polynomial, Function",
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

// Type alias: ScalarLike = int | float | numpy.integer | numpy.floating
pyo3_stub_gen::type_alias!(
    "ommx._ommx_rust",
    ScalarLike = i64 | f64 | NumpyInteger | NumpyFloating
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

// Type alias: ToFunction = ScalarLike | LinearLike | Parameter | Quadratic | Polynomial | Function
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

// Manual stubs for methods whose Python protocol signatures differ from the
// Rust callbacks understood by pyo3-stub-gen. PyO3's in-place operator wrapper
// returns the mutated self object to Python when the Rust callback succeeds.
pyo3_stub_gen::inventory::submit! {
    pyo3_stub_gen::derive::gen_methods_from_python! {
        r#"
        class Function:
            def __iadd__(self, rhs: ToFunction) -> Function: ...
            def __pow__(self, exponent: int, modulo: None = None) -> Function: ...
        "#
    }
}

fn polynomial_only_error(attribute: &str) -> PyErr {
    PyTypeError::new_err(format!(
        "Function.{attribute} is only available for polynomial functions"
    ))
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Function {
    /// Create a Function from various types.
    ///
    /// Accepts:
    /// - int or float: creates a constant function
    /// - DecisionVariable: creates a linear function with single term
    /// - AttachedDecisionVariable: creates a linear function with single term
    ///   (only the id is used; no host borrow is taken)
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
    pub fn from_scalar(scalar: f64) -> crate::error::OmmxPyResult<Self> {
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

    /// Return the degree of the stored polynomial, or ``None`` for a composed expression.
    ///
    /// Returns the highest degree of any term in a polynomial function.
    /// Zero function has degree 0, constant function has degree 0,
    /// linear function has degree 1, quadratic function has degree 2, etc.
    /// This inspects the representation; it does not expand an expression to
    /// determine its mathematical degree. For the variable expression
    /// ``g = Function(DecisionVariable.continuous(0))``, ``g ** 2`` returns
    /// ``None`` while the explicitly multiplied ``g * g`` has degree 2.
    pub fn degree(&self) -> Option<u32> {
        self.0.degree().map(|degree| degree.into_inner())
    }

    /// Get the number of terms in this function.
    ///
    /// Zero function has 0 terms, constant function has 1 term,
    /// and polynomial functions have the number of non-zero coefficient terms.
    /// Returns ``None`` for composed expressions, even when their mathematical
    /// value could be represented by a polynomial. No expansion is performed.
    pub fn num_terms(&self) -> Option<usize> {
        self.0.num_terms()
    }

    /// Compare polynomial coefficients or composed-expression structure within ``atol``.
    ///
    /// For two stored polynomials, compares the coefficient of each monomial;
    /// an absent monomial has coefficient zero. Different polynomial storage
    /// variants may compare equal. For two composed expressions, operators and
    /// evaluation order must match, and polynomial operands are compared with
    /// the same coefficient tolerance. A polynomial and a composed expression
    /// do not compare equal merely because they evaluate to the same values.
    /// This is not a test of global mathematical equivalence.
    ///
    /// - ``other``: Function to compare with.
    /// - ``atol``: Positive absolute coefficient tolerance.
    ///
    /// >>> from ommx import DecisionVariable, Function
    /// >>> x = Function(DecisionVariable.continuous(0))
    /// >>> assert ((x + 1) * (x + 1)).almost_equal(x * x + 2 * x + 1)
    /// >>> assert (x * x).almost_equal(x * x)
    /// >>> assert not (x ** 2).almost_equal(x * x)
    #[pyo3(signature = (other, atol=ATol::default().into_inner()))]
    pub fn almost_equal(&self, other: &Function, atol: f64) -> crate::error::OmmxPyResult<bool> {
        Ok(self.0.abs_diff_eq(&other.0, ommx::ATol::new(atol)?))
    }

    pub fn __repr__(&self) -> String {
        format!("Function({})", self.0)
    }

    /// Negation operator
    pub fn __neg__(&self) -> Function {
        Function(-self.0.clone())
    }

    /// Absolute value operator
    pub fn __abs__(&self) -> Function {
        Function(self.0.clone().abs())
    }

    /// Construct the pointwise sign of this function.
    ///
    /// Evaluation returns 0 when ``abs(value) <= atol``, -1 below that interval,
    /// and 1 above it. The tolerance is supplied to {meth}`~ommx.Function.evaluate`,
    /// not fixed when this expression is constructed or partially evaluated.
    pub fn signum(&self) -> Function {
        Function(self.0.clone().signum())
    }

    /// Construct the pointwise minimum of two functions.
    ///
    /// Use this method instead of Python's built-in ``min``, which compares
    /// objects rather than constructing an optimization expression.
    pub fn minimum(&self, rhs: Function) -> Function {
        Function(self.0.clone().min(rhs.0))
    }

    /// Construct the pointwise maximum of two functions.
    ///
    /// Use this method instead of Python's built-in ``max``.
    pub fn maximum(&self, rhs: Function) -> Function {
        Function(self.0.clone().max(rhs.0))
    }

    /// Addition
    pub fn __add__(&self, rhs: Function) -> crate::error::OmmxPyResult<Function> {
        Ok(Function((&self.0 + &rhs.0)?))
    }

    /// Reverse addition (lhs + self)
    pub fn __radd__(&self, lhs: Function) -> crate::error::OmmxPyResult<Function> {
        Ok(Function((&lhs.0 + &self.0)?))
    }

    /// Subtraction
    pub fn __sub__(&self, rhs: Function) -> crate::error::OmmxPyResult<Function> {
        Ok(Function((&self.0 - &rhs.0)?))
    }

    /// Reverse subtraction (lhs - self)
    pub fn __rsub__(&self, lhs: Function) -> crate::error::OmmxPyResult<Function> {
        Ok(Function((&lhs.0 - &self.0)?))
    }

    pub fn add_assign(&mut self, rhs: &Function) -> crate::error::OmmxPyResult<()> {
        crate::in_place_add::preflight_function(&self.0, &rhs.0)?;
        self.0.try_add_assign_in_place(rhs.0.clone())?;
        Ok(())
    }

    /// In-place addition for += operator.
    ///
    /// PyO3's in-place operator wrapper returns the mutated self object to Python
    /// when this Rust callback succeeds.
    #[gen_stub(skip)]
    pub fn __iadd__(&mut self, rhs: &Function) -> crate::error::OmmxPyResult<()> {
        self.add_assign(rhs)
    }

    /// Multiplication
    pub fn __mul__(&self, rhs: Function) -> crate::error::OmmxPyResult<Function> {
        Ok(Function((&self.0 * &rhs.0)?))
    }

    /// Reverse multiplication (lhs * self)
    pub fn __rmul__(&self, lhs: Function) -> crate::error::OmmxPyResult<Function> {
        Ok(Function((&lhs.0 * &self.0)?))
    }

    /// Construct division by another function, retaining a composed expression.
    ///
    /// This also applies to a constant divisor. The zero-domain check is made
    /// by {meth}`~ommx.Function.evaluate` with its ``atol``; constructing the
    /// expression does not establish that all inputs lie in its domain.
    pub fn __truediv__(&self, rhs: Function) -> crate::error::OmmxPyResult<Function> {
        Ok(Function((self.0.clone() / rhs.0)?))
    }

    /// Reverse division (lhs / self)
    pub fn __rtruediv__(&self, lhs: Function) -> crate::error::OmmxPyResult<Function> {
        Ok(Function((lhs.0 / self.0.clone())?))
    }

    /// Raise this function to a signed 32-bit integer power.
    ///
    /// ``f.powi(n)`` and ``f ** n`` construct the same operation. Floating-point
    /// or function-valued exponents, reversed exponentiation such as ``2 ** f``,
    /// and modular exponentiation are not supported.
    ///
    /// Except for identity operations and finite constant folding, powers
    /// remain composed expressions, including nonnegative powers. They are not
    /// automatically expanded into polynomial coefficients. Use explicit
    /// multiplication, such as ``g * g``, when an expanded polynomial square is
    /// required by an input format.
    ///
    /// During {meth}`~ommx.Function.evaluate`, a negative power is undefined when
    /// the base has absolute value at most ``atol``. Negative bases are supported because the
    /// exponent is integral; zero to the zero power is 1. The operand is still
    /// evaluated, so raising an undefined expression to zero does not erase its
    /// domain restriction.
    ///
    /// >>> from ommx import DecisionVariable, Function
    /// >>> g = Function(DecisionVariable.continuous(0))
    /// >>> assert (g ** 2).degree() is None
    /// >>> assert (g * g).degree() == 2
    /// >>> assert (g ** 0).evaluate({0: 0}) == 1.0
    pub fn powi(&self, exponent: i32) -> Function {
        Function(self.0.clone().powi(exponent))
    }

    /// Integer exponentiation
    #[gen_stub(skip)]
    pub fn __pow__(&self, exponent: i32, modulo: Option<&Bound<'_, PyAny>>) -> PyResult<Function> {
        if modulo.is_some() {
            return Err(PyTypeError::new_err(
                "modular exponentiation is not supported for Function",
            ));
        }
        Ok(self.powi(exponent))
    }

    pub fn add_scalar(&self, scalar: f64) -> crate::error::OmmxPyResult<Function> {
        match TryInto::<Coefficient>::try_into(scalar) {
            Ok(coeff) => Ok(Function((&self.0 + coeff)?)),
            Err(CoefficientError::Zero) => Ok(Function(self.0.clone())), // Return unchanged if scalar is zero
            Err(e) => Err(e.into()), // Return error for NaN or infinite
        }
    }

    pub fn add_linear(&self, linear: &Linear) -> crate::error::OmmxPyResult<Function> {
        Ok(Function((&self.0 + &linear.0)?))
    }

    pub fn add_quadratic(&self, quadratic: &Quadratic) -> crate::error::OmmxPyResult<Function> {
        Ok(Function((&self.0 + &quadratic.0)?))
    }

    pub fn add_polynomial(&self, polynomial: &Polynomial) -> crate::error::OmmxPyResult<Function> {
        Ok(Function((&self.0 + &polynomial.0)?))
    }

    pub fn mul_scalar(&self, scalar: f64) -> crate::error::OmmxPyResult<Function> {
        match TryInto::<Coefficient>::try_into(scalar) {
            Ok(coeff) => Ok(Function((&self.0 * coeff)?)),
            // Keep the domain of a partial expression: `0 * (1 / x)` is still
            // undefined at x = 0. The Rust Function multiplication fast path
            // reduces polynomial zero-products while retaining composite ones.
            Err(CoefficientError::Zero) => Ok(Function((&self.0 * ommx::Function::default())?)),
            Err(e) => Err(e.into()), // Return error for NaN or infinite
        }
    }

    pub fn mul_linear(&self, linear: &Linear) -> crate::error::OmmxPyResult<Function> {
        Ok(Function((&self.0 * &linear.0)?))
    }

    pub fn mul_quadratic(&self, quadratic: &Quadratic) -> crate::error::OmmxPyResult<Function> {
        Ok(Function((&self.0 * &quadratic.0)?))
    }

    pub fn mul_polynomial(&self, polynomial: &Polynomial) -> crate::error::OmmxPyResult<Function> {
        Ok(Function((&self.0 * &polynomial.0)?))
    }

    /// Return the minimal positive factor that makes all coefficients integers.
    ///
    /// Raises `TypeError` for a composed, non-polynomial Function.
    pub fn content_factor(&self) -> crate::error::OmmxPyResult<f64> {
        if !self.0.is_polynomial() {
            return Err(polynomial_only_error("content_factor").into());
        }
        Ok(self.0.content_factor()?.into_inner())
    }

    pub fn required_ids(&self) -> BTreeSet<u64> {
        self.0
            .required_ids()
            .into_iter()
            .map(|id| id.into_inner())
            .collect()
    }

    /// Get all polynomial terms as a dictionary mapping monomial tuples to coefficients.
    ///
    /// The empty tuple denotes the constant term. Repeated IDs denote powers,
    /// for example ``(0, 0)`` for the square of variable 0. The dictionary is a
    /// snapshot and does not change the Function when edited.
    /// Finite nonzero coefficients are not discarded because of their size or
    /// the evaluation tolerance; exact cancellation or floating-point underflow
    /// may produce zeros during polynomial arithmetic.
    /// Raises ``TypeError`` for a composed expression. This property does not
    /// expand the expression, including a retained integer power.
    #[getter]
    pub fn terms<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let terms = self
            .0
            .iter()
            .ok_or_else(|| polynomial_only_error("terms"))?;
        let result = PyDict::new(py);
        for (monomial, coefficient) in terms {
            let ids = monomial
                .iter()
                .map(|id| id.into_inner())
                .collect::<Vec<_>>();
            result.set_item(PyTuple::new(py, ids)?, coefficient.into_inner())?;
        }
        Ok(result)
    }

    /// Get linear terms as a dictionary mapping variable id to coefficient.
    ///
    /// Returns dictionary mapping variable IDs to their linear coefficients.
    /// Returns empty dict if function has no linear terms.
    /// Works for all polynomial functions by filtering only degree-1 terms.
    /// Raises TypeError if this is a non-polynomial expression function.
    #[getter]
    pub fn linear_terms(&self) -> PyResult<BTreeMap<u64, f64>> {
        Ok(self
            .0
            .linear_terms()
            .ok_or_else(|| polynomial_only_error("linear_terms"))?
            .map(|(id, coeff)| (id.into_inner(), coeff.into_inner()))
            .collect())
    }

    /// Get quadratic terms as a dictionary mapping (row, col) to coefficient.
    ///
    /// Returns dictionary mapping variable ID pairs to their quadratic coefficients.
    /// Returns empty dict if function has no quadratic terms.
    /// Works for all polynomial functions by filtering only degree-2 terms.
    /// Raises TypeError if this is a non-polynomial expression function.
    #[getter]
    pub fn quadratic_terms(&self) -> PyResult<BTreeMap<(u64, u64), f64>> {
        Ok(self
            .0
            .quadratic_terms()
            .ok_or_else(|| polynomial_only_error("quadratic_terms"))?
            .map(|(pair, coeff)| {
                (
                    (pair.lower().into_inner(), pair.upper().into_inner()),
                    coeff.into_inner(),
                )
            })
            .collect())
    }

    /// Get the constant term of the function.
    ///
    /// Returns the constant term. Returns 0.0 if function has no constant term.
    /// Works for all polynomial functions by filtering the degree-0 term.
    /// Raises TypeError if this is a non-polynomial expression function.
    #[getter]
    pub fn constant_term(&self) -> PyResult<f64> {
        self.0
            .constant_term()
            .ok_or_else(|| polynomial_only_error("constant_term"))
    }

    #[staticmethod]
    #[pyo3(signature = (
        rng,
        num_terms=ommx::PolynomialParameters::default().num_terms(),
        max_degree=ommx::PolynomialParameters::default().max_degree().into_inner(),
        max_id=ommx::PolynomialParameters::default().max_id().into_inner()
    ))]
    pub fn random(
        rng: &Rng,
        num_terms: usize,
        max_degree: u32,
        max_id: u64,
    ) -> crate::error::OmmxPyResult<Self> {
        let mut rng = rng.lock()?;
        let inner: ommx::Function = ommx::random::random(
            &mut rng,
            ommx::FunctionParameters::polynomial_only(ommx::PolynomialParameters::new(
                num_terms,
                max_degree.into(),
                max_id.into(),
            )?),
        );
        Ok(Self(inner))
    }

    /// Evaluate this expression at the supplied variable values.
    ///
    /// - ``state``: A {class}`~ommx.State` or mapping from variable IDs to values.
    ///     All IDs required by the expression must be supplied.
    /// - ``atol``: Positive absolute tolerance for zero-sensitive operations.
    ///     If omitted, {func}`~ommx.get_default_atol` supplies it.
    ///
    /// Returns the scalar value of the expression.
    ///
    /// A Function references IDs but does not own variable kinds, bounds, or
    /// constraints. This method does not test candidate feasibility or round
    /// discrete-variable values; use {meth}`~ommx.Instance.evaluate` for that.
    ///
    /// ``abs(v) <= atol`` is treated as zero, including the boundary. This gives
    /// ``signum(v) == 0`` and makes a denominator or a negative-power base in
    /// that interval undefined. ``0 ** 0`` is 1, and integer powers of negative
    /// bases are supported. Polynomial coefficients are not pruned using this
    /// tolerance.
    ///
    /// Composed expressions evaluate operands from left to right and combine
    /// two values at each operator application. Parentheses and operation order
    /// are retained rather than reassociated. Overflow and domain failures can
    /// therefore depend on the represented order. Simplification does not
    /// erase the domain of a partial function: ``0 * (1 / f)`` remains undefined
    /// where ``f`` is treated as zero.
    ///
    /// Raises ``ValueError`` for a missing required variable value, undefined
    /// point operation, or non-finite operand/intermediate result.
    ///
    /// >>> from ommx import Function, Linear
    /// >>> f = Function(Linear(terms={0: 1, 1: 2}, constant=3))
    /// >>> assert f.evaluate({0: 1, 1: 0}) == 4.0
    #[pyo3(signature = (state, *, atol=None))]
    pub fn evaluate(&self, state: State, atol: Option<f64>) -> crate::error::OmmxPyResult<f64> {
        use ommx::Evaluate;
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        Ok(self.0.evaluate(&state.0, atol)?)
    }

    /// Substitute supplied variable values and return the remaining Function.
    ///
    /// The original Function is unchanged. Values may cover only some of the
    /// required IDs; unresolved variables remain in the returned expression.
    /// The concrete {class}`~ommx.Linear`, {class}`~ommx.Quadratic`, and
    /// {class}`~ommx.Polynomial` partial-evaluation methods preserve their
    /// respective Python types; this method returns a Function.
    ///
    /// Closed subexpressions are folded only when the result is independent of
    /// the later evaluation tolerance. ``signum``, division, and negative
    /// integer powers remain composed operations even after all their variables
    /// are replaced. Construction, substitution, and partial evaluation do not
    /// capture a default or supplied ``atol`` for these zero-domain decisions;
    /// the subsequent {meth}`~ommx.Function.evaluate` call determines their
    /// value or domain.
    ///
    /// - ``state``: Variable-ID/value mapping or {class}`~ommx.State` to substitute.
    /// - ``atol``: A positive tolerance; zero-sensitive composed
    ///     operations are retained rather than resolved with this value.
    ///
    /// >>> from ommx import Function, Linear
    /// >>> f = Function(Linear(terms={0: 1, 1: 2}, constant=3))
    /// >>> remaining = f.partial_evaluate({0: 1})
    /// >>> assert remaining.evaluate({1: 0}) == 4.0
    /// >>> x = Function(Linear(terms={0: 1}))
    /// >>> sign = x.signum().partial_evaluate({0: 1e-8}, atol=1e-6)
    /// >>> assert sign.evaluate({}, atol=1e-6) == 0.0
    /// >>> assert sign.evaluate({}, atol=1e-9) == 1.0
    #[pyo3(signature = (state, *, atol=None))]
    pub fn partial_evaluate(
        &self,
        state: State,
        atol: Option<f64>,
    ) -> crate::error::OmmxPyResult<Function> {
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        let mut inner = self.0.clone();
        inner.partial_evaluate(&state.0, atol)?;
        Ok(Function(inner))
    }

    /// Compute an interval bound of this function given variable bounds.
    ///
    /// Missing IDs in `bounds` are treated as unbounded (`Bound.unbounded()`).
    ///
    /// **Args:**
    ///
    /// - `bounds`: Mapping from variable ID to its {class}`~ommx.Bound`.
    /// - `atol`: Absolute tolerance used by operations whose semantics depend on
    ///   whether a value is zero. If omitted, {func}`~ommx.get_default_atol` supplies it.
    ///   Use the same tolerance when point-evaluating this function.
    ///
    /// **Returns:** A {class}`~ommx.Bound` that contains $[\inf f, \sup f]$ over the given variable bounds.
    ///
    /// **Tightness:** Polynomial leaves are bounded **term by term**
    /// (monomial-wise), and composed expression operations combine their operand
    /// bounds with interval arithmetic. The result is a **sound
    /// over-approximation** of the true range $[\inf f, \sup f]$ but is **not
    /// guaranteed to be tight**, because it ignores dependencies between terms
    /// or operands that share variables. For example, $f = x^2 - x$ with
    /// $x \in [0, 1]$ has true range $[-1/4, 0]$ (minimum at $x = 1/2$), but
    /// term-wise evaluation yields $[0, 1] + (-[0, 1]) = [-1, 1]$.
    ///
    /// **Raises:** `RuntimeError` when an interval contains a value treated as
    /// zero by `atol` in a denominator or as the base of a negative integer
    /// power.
    /// Raises `ValueError` if valid bound endpoints cannot be constructed after
    /// numeric overflow.
    ///
    /// # Examples
    ///
    /// >>> from ommx import Function, Linear, Bound
    /// >>> f = Function(Linear(terms={1: 2}, constant=3))  # 2*x1 + 3
    /// >>> b = f.evaluate_bound({1: Bound(0.0, 2.0)})
    /// >>> b.lower <= 3.0 and b.upper >= 7.0
    /// True
    #[pyo3(signature = (bounds, *, atol=None))]
    pub fn evaluate_bound(
        &self,
        bounds: BTreeMap<u64, VariableBound>,
        atol: Option<f64>,
    ) -> crate::error::OmmxPyResult<VariableBound> {
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        let bounds: ommx::Bounds = bounds
            .into_iter()
            .map(|(id, b)| (ommx::VariableID::from(id), b.0))
            .collect();
        Ok(VariableBound(self.0.evaluate_bound(&bounds, atol)?))
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
            ommx::Function::Expression(_) => "Expression",
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
    pub fn reduce_binary_power(
        &mut self,
        binary_ids: BTreeSet<u64>,
    ) -> crate::error::OmmxPyResult<bool> {
        let variable_id_set: ommx::VariableIDSet =
            binary_ids.into_iter().map(ommx::VariableID::from).collect();
        Ok(self.0.reduce_binary_power(&variable_id_set)?)
    }

    /// Create an equality constraint: self == other → Constraint with EqualToZero
    ///
    /// Returns a Constraint where (self - other) == 0.
    /// Note: This does NOT return bool, it creates a Constraint object.
    #[gen_stub(type_ignore = ["override"])]
    #[pyo3(name = "__eq__")]
    pub fn py_eq(&self, other: Function) -> crate::error::OmmxPyResult<Constraint> {
        crate::comparison_constraint(-other.0 + &self.0, ommx::Equality::EqualToZero)
    }

    /// Create a less-than-or-equal constraint: self <= other → Constraint with LessThanOrEqualToZero
    ///
    /// Returns a Constraint where (self - other) <= 0.
    #[pyo3(name = "__le__")]
    pub fn py_le(&self, other: Function) -> crate::error::OmmxPyResult<Constraint> {
        crate::comparison_constraint(-other.0 + &self.0, ommx::Equality::LessThanOrEqualToZero)
    }

    /// Create a greater-than-or-equal constraint: self >= other → Constraint with LessThanOrEqualToZero
    ///
    /// Returns a Constraint where (other - self) <= 0.
    #[pyo3(name = "__ge__")]
    pub fn py_ge(&self, other: Function) -> crate::error::OmmxPyResult<Constraint> {
        crate::comparison_constraint(other.0 - &self.0, ommx::Equality::LessThanOrEqualToZero)
    }
}
