use crate::{
    next_constraint_id, Constraint, DecisionVariable, Function, Linear, Parameter, Polynomial, Rng,
    State,
};

use anyhow::{anyhow, Result};
use approx::AbsDiffEq;
use ommx::{ATol, Coefficient, CoefficientError, Evaluate, LinearMonomial, VariableIDPair};
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict},
    Bound, PyAny,
};
use std::collections::BTreeMap;

/// Quadratic function of decision variables.
///
/// A quadratic function has the form: $c_0 + \sum_i c_i x_i + \sum_{ij} q_{ij} x_i x_j$
/// where $x_i$ are decision variables and $c_i$, $q_{ij}$ are coefficients.
///
/// # Examples
///
/// Create via DecisionVariable multiplication:
///
/// ```python
/// >>> x = DecisionVariable.integer(1)
/// >>> y = DecisionVariable.integer(2)
/// >>> q = x * y + 2*x + 3*y + 1
/// ```
///
/// Note that `==`, `<=`, `>=` create Constraint objects:
///
/// ```python
/// >>> constraint = q <= 10  # Returns Constraint
/// ```
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct Quadratic(pub ommx::Quadratic);

// Overload stubs for arithmetic operators.
// Must appear before #[gen_stub_pymethods] for correct ordering.
pyo3_stub_gen::inventory::submit! {
    pyo3_stub_gen::derive::gen_methods_from_python! {
        r#"
        class Quadratic:
            @overload
            def __add__(self, rhs: int | float | DecisionVariable | Parameter | Linear | Quadratic) -> Quadratic: ...
            @overload
            def __add__(self, rhs: Polynomial) -> Polynomial: ...

            @overload
            def __radd__(self, lhs: int | float | DecisionVariable | Parameter | Linear | Quadratic) -> Quadratic: ...
            @overload
            def __radd__(self, lhs: Polynomial) -> Polynomial: ...

            @overload
            def __sub__(self, rhs: int | float | DecisionVariable | Parameter | Linear | Quadratic) -> Quadratic: ...
            @overload
            def __sub__(self, rhs: Polynomial) -> Polynomial: ...

            @overload
            def __rsub__(self, lhs: int | float | DecisionVariable | Parameter | Linear | Quadratic) -> Quadratic: ...
            @overload
            def __rsub__(self, lhs: Polynomial) -> Polynomial: ...

            @overload
            def __mul__(self, rhs: int | float) -> Quadratic: ...
            @overload
            def __mul__(self, rhs: DecisionVariable | Parameter | Linear | Quadratic | Polynomial) -> Polynomial: ...

            @overload
            def __rmul__(self, lhs: int | float) -> Quadratic: ...
            @overload
            def __rmul__(self, lhs: DecisionVariable | Parameter | Linear | Quadratic | Polynomial) -> Polynomial: ...

            def __iadd__(self, rhs: Quadratic) -> Quadratic: ...
        "#
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Quadratic {
    #[new]
    #[pyo3(signature = (columns, rows, values, linear=None))]
    pub fn new(
        columns: Vec<u64>,
        rows: Vec<u64>,
        values: Vec<f64>,
        linear: Option<Linear>,
    ) -> Result<Self> {
        // Validate that all input vectors have the same length
        if columns.len() != rows.len() || columns.len() != values.len() {
            return Err(anyhow!(
                "Input vectors must have the same length: columns={}, rows={}, values={}",
                columns.len(),
                rows.len(),
                values.len()
            ));
        }

        let mut out = ommx::Quadratic::default();
        for ((col_id, row_id), value) in columns
            .into_iter()
            .zip(rows.into_iter())
            .zip(values.into_iter())
        {
            match TryInto::<Coefficient>::try_into(value) {
                Ok(coeff) => {
                    out.add_term(
                        ommx::QuadraticMonomial::Pair(VariableIDPair::new(
                            col_id.into(),
                            row_id.into(),
                        )),
                        coeff,
                    );
                }
                Err(CoefficientError::Zero) => {
                    // Skip zero coefficients
                    continue;
                }
                Err(e) => {
                    return Err(e.into()); // Return error for NaN or infinite
                }
            }
        }
        // Add linear part if provided
        if let Some(linear) = linear {
            out += &linear.0;
        }
        Ok(Self(out))
    }

    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        Ok(Self(ommx::Quadratic::from_bytes(bytes.as_bytes())?))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.0.to_bytes())
    }

    #[pyo3(signature = (other, atol=ATol::default().into_inner()))]
    pub fn almost_equal(&self, other: &Quadratic, atol: f64) -> bool {
        self.0.abs_diff_eq(&other.0, ommx::ATol::new(atol).unwrap())
    }

    pub fn __repr__(&self) -> String {
        format!("Quadratic({})", self.0)
    }

    /// Negation operator
    pub fn __neg__(&self) -> Quadratic {
        Quadratic(-self.0.clone())
    }

    /// Polymorphic addition
    #[gen_stub(skip)]
    #[pyo3(name = "__add__")]
    pub fn py_add(&self, py: Python<'_>, rhs: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        if let Ok(quad) = rhs.extract::<PyRef<Quadratic>>() {
            return Ok(Quadratic(&self.0 + &quad.0)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        if let Ok(linear) = rhs.extract::<PyRef<Linear>>() {
            return Ok(Quadratic(&self.0 + &linear.0)
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
            return Ok(Quadratic(&self.0 + &rhs_linear)
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
            return Ok(Quadratic(&self.0 + &rhs_linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract from Python wrapper (has .raw attribute pointing to DecisionVariable)
        if let Ok(raw) = rhs.getattr("raw") {
            if let Ok(dv) = raw.extract::<PyRef<DecisionVariable>>() {
                let rhs_linear = ommx::Linear::single_term(
                    LinearMonomial::Variable(dv.0.id()),
                    ommx::coeff!(1.0),
                );
                return Ok(Quadratic(&self.0 + &rhs_linear)
                    .into_pyobject(py)?
                    .into_any()
                    .unbind());
            }
        }
        if let Ok(val) = rhs.extract::<f64>() {
            let quad = self
                .add_scalar(val)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
            return Ok(quad.into_pyobject(py)?.into_any().unbind());
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
        if let Ok(quad) = rhs.extract::<PyRef<Quadratic>>() {
            return Ok(Quadratic(&self.0 - &quad.0)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        if let Ok(linear) = rhs.extract::<PyRef<Linear>>() {
            // self - linear
            return Ok(Quadratic(self.0.clone() - &linear.0)
                .into_pyobject(py)?
                .into_any()
                .unbind());
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
            return Ok(Quadratic(self.0.clone() - &rhs_linear)
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
            return Ok(Quadratic(self.0.clone() - &rhs_linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract from Python wrapper (has .raw attribute pointing to DecisionVariable)
        if let Ok(raw) = rhs.getattr("raw") {
            if let Ok(dv) = raw.extract::<PyRef<DecisionVariable>>() {
                let rhs_linear = ommx::Linear::single_term(
                    LinearMonomial::Variable(dv.0.id()),
                    ommx::coeff!(1.0),
                );
                return Ok(Quadratic(self.0.clone() - &rhs_linear)
                    .into_pyobject(py)?
                    .into_any()
                    .unbind());
            }
        }
        if let Ok(val) = rhs.extract::<f64>() {
            let quad = self
                .add_scalar(-val)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
            return Ok(quad.into_pyobject(py)?.into_any().unbind());
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

    pub fn add_assign(&mut self, rhs: &Quadratic) {
        self.0 += &rhs.0;
    }

    /// In-place addition for += operator
    #[gen_stub(skip)]
    pub fn __iadd__(&mut self, rhs: &Quadratic) {
        self.0 += &rhs.0;
    }

    /// Polymorphic multiplication
    #[gen_stub(skip)]
    #[pyo3(name = "__mul__")]
    pub fn py_mul(&self, py: Python<'_>, rhs: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        if let Ok(quad) = rhs.extract::<PyRef<Quadratic>>() {
            return Ok(Polynomial(&self.0 * &quad.0)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        if let Ok(linear) = rhs.extract::<PyRef<Linear>>() {
            return Ok(Polynomial(&self.0 * &linear.0)
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
            return Ok(Polynomial(&self.0 * &rhs_linear)
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
            return Ok(Polynomial(&self.0 * &rhs_linear)
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
                return Ok(Polynomial(&self.0 * &rhs_linear)
                    .into_pyobject(py)?
                    .into_any()
                    .unbind());
            }
        }
        if let Ok(val) = rhs.extract::<f64>() {
            let quad = self
                .mul_scalar(val)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
            return Ok(quad.into_pyobject(py)?.into_any().unbind());
        }
        // Return NotImplemented to allow Python to try the reflected operation
        Ok(py.NotImplemented().clone_ref(py).into_any())
    }

    /// Reverse multiplication (lhs * self)
    #[gen_stub(skip)]
    pub fn __rmul__(&self, py: Python<'_>, lhs: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        self.py_mul(py, lhs) // Multiplication is commutative
    }

    pub fn add_scalar(&self, scalar: f64) -> Result<Quadratic> {
        match TryInto::<Coefficient>::try_into(scalar) {
            Ok(coeff) => Ok(Quadratic(&self.0 + coeff)),
            Err(CoefficientError::Zero) => Ok(Quadratic(self.0.clone())), // Return unchanged if scalar is zero
            Err(e) => Err(e.into()), // Return error for NaN or infinite
        }
    }

    pub fn add_linear(&self, linear: &Linear) -> Quadratic {
        Quadratic(&self.0 + &linear.0)
    }

    pub fn mul_scalar(&self, scalar: f64) -> Result<Quadratic> {
        match TryInto::<Coefficient>::try_into(scalar) {
            Ok(coeff) => Ok(Quadratic(self.0.clone() * coeff)),
            Err(CoefficientError::Zero) => Ok(Quadratic(ommx::Quadratic::default())), // Return zero if scalar is zero
            Err(e) => Err(e.into()), // Return error for NaN or infinite
        }
    }

    pub fn mul_linear(&self, linear: &Linear) -> Polynomial {
        Polynomial(&self.0 * &linear.0)
    }

    #[getter]
    pub fn linear_terms(&self) -> BTreeMap<u64, f64> {
        self.0
            .linear_terms()
            .map(|(id, coeff)| (id.into_inner(), coeff.into_inner()))
            .collect()
    }

    #[getter]
    pub fn constant_term(&self) -> f64 {
        self.0.constant_term()
    }

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

    pub fn terms<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let obj = serde_pyobject::to_pyobject(py, &self.0)?;
        Ok(obj.cast::<PyDict>()?.clone())
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
    pub fn partial_evaluate(&self, state: State, atol: Option<f64>) -> PyResult<Quadratic> {
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?,
            None => ommx::ATol::default(),
        };
        let mut inner = self.0.clone();
        inner
            .partial_evaluate(&state.0, atol)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
        Ok(Quadratic(inner))
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
        let id = next_constraint_id();
        Constraint(ommx::Constraint {
            id: ommx::ConstraintID::from(id),
            equality: ommx::Equality::EqualToZero,
            metadata: ommx::ConstraintMetadata::default(),
            stage: ommx::CreatedData { function },
        })
    }

    /// Create a less-than-or-equal constraint: self <= other → Constraint
    #[pyo3(name = "__le__")]
    pub fn py_le(&self, other: Function) -> Constraint {
        let mut function = -other.0;
        function += &self.0;
        let id = next_constraint_id();
        Constraint(ommx::Constraint {
            id: ommx::ConstraintID::from(id),
            equality: ommx::Equality::LessThanOrEqualToZero,
            metadata: ommx::ConstraintMetadata::default(),
            stage: ommx::CreatedData { function },
        })
    }

    /// Create a greater-than-or-equal constraint: self >= other → Constraint
    #[pyo3(name = "__ge__")]
    pub fn py_ge(&self, other: Function) -> Constraint {
        let function = other.0 - &self.0;
        let id = next_constraint_id();
        Constraint(ommx::Constraint {
            id: ommx::ConstraintID::from(id),
            equality: ommx::Equality::LessThanOrEqualToZero,
            metadata: ommx::ConstraintMetadata::default(),
            stage: ommx::CreatedData { function },
        })
    }
}
