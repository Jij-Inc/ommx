use crate::{
    next_constraint_id, Constraint, Function, Linear, Parameter, Polynomial, Quadratic,
    VariableBound,
};
use anyhow::Result;
use ommx::{v1, ATol, LinearMonomial, VariableID};
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict},
    Bound, PyAny,
};
use std::collections::HashMap;

/// Decision variable in an optimization problem.
///
/// This class represents a variable that will be optimized in a mathematical programming problem.
/// It supports various types (binary, integer, continuous, semi-integer, semi-continuous) and
/// can be used in arithmetic expressions to build objective functions and constraints.
///
/// Note that this object overloads `==` for creating a constraint, not for equality comparison.
///
/// Example:
///     >>> x = DecisionVariable.integer(1)
///     >>> x == 1  # Returns Constraint, not bool
///     Constraint(...)
///
/// For object equality comparison, use the ``equals_to()`` method or compare IDs:
///
/// Example:
///     >>> y = DecisionVariable.integer(2)
///     >>> x.id == y.id
///     False
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct DecisionVariable(pub ommx::DecisionVariable);

impl DecisionVariable {
    /// Helper to create a Linear term from this decision variable with coefficient 1
    fn as_linear(&self) -> ommx::Linear {
        ommx::Linear::single_term(LinearMonomial::Variable(self.0.id()), ommx::coeff!(1.0))
    }

    /// Convert to a dict for pandas DataFrame. Not exposed to Python.
    ///
    /// `na` should be `pandas.NA`, pre-fetched by the caller.
    pub(crate) fn as_pandas_entry<'py>(
        &self,
        py: Python<'py>,
        na: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);

        dict.set_item("id", self.id())?;

        let kind: v1::decision_variable::Kind = self.0.kind().into();
        let kind_str = match kind {
            v1::decision_variable::Kind::Unspecified => "Unspecified",
            v1::decision_variable::Kind::Binary => "Binary",
            v1::decision_variable::Kind::Integer => "Integer",
            v1::decision_variable::Kind::Continuous => "Continuous",
            v1::decision_variable::Kind::SemiInteger => "SemiInteger",
            v1::decision_variable::Kind::SemiContinuous => "SemiContinuous",
            _ => "Unknown",
        };
        dict.set_item("kind", kind_str)?;
        dict.set_item("lower", self.0.bound().lower())?;
        dict.set_item("upper", self.0.bound().upper())?;

        match &self.0.metadata.name {
            Some(name) if !name.is_empty() => dict.set_item("name", name)?,
            _ => dict.set_item("name", na)?,
        }
        dict.set_item("subscripts", self.0.metadata.subscripts.clone())?;
        match &self.0.metadata.description {
            Some(desc) if !desc.is_empty() => dict.set_item("description", desc)?,
            _ => dict.set_item("description", na)?,
        }
        match self.0.substituted_value() {
            Some(v) => dict.set_item("substituted_value", v)?,
            None => dict.set_item("substituted_value", na)?,
        }

        for (key, value) in &self.0.metadata.parameters {
            dict.set_item(format!("parameters.{key}"), value)?;
        }

        Ok(dict)
    }
}

// Overload stubs for arithmetic operators.
// Must appear before #[gen_stub_pymethods] for correct ordering.
pyo3_stub_gen::inventory::submit! {
    pyo3_stub_gen::derive::gen_methods_from_python! {
        r#"
        class DecisionVariable:
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
        "#
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl DecisionVariable {
    #[new]
    #[pyo3(signature = (id, kind, bound, name=None, subscripts=Vec::new(), parameters=HashMap::default(), description=None))]
    pub fn new(
        id: u64,
        kind: i32,
        bound: VariableBound,
        name: Option<String>,
        subscripts: Vec<i64>,
        parameters: HashMap<String, String>,
        description: Option<String>,
    ) -> Result<Self> {
        let variable_id = VariableID::from(id);
        let kind = v1::decision_variable::Kind::try_from(kind)?.try_into()?;

        let mut decision_variable = ommx::DecisionVariable::new(
            variable_id,
            kind,
            bound.0,
            None, // substituted_value
            ATol::default(),
        )?;

        decision_variable.metadata.name = name;
        decision_variable.metadata.subscripts = subscripts;
        decision_variable.metadata.parameters = parameters.into_iter().collect();
        decision_variable.metadata.description = description;

        Ok(Self(decision_variable))
    }

    #[getter]
    pub fn id(&self) -> u64 {
        self.0.id().into_inner()
    }

    #[getter]
    pub fn kind(&self) -> i32 {
        let kind: v1::decision_variable::Kind = self.0.kind().into();
        kind as i32
    }

    #[getter]
    pub fn bound(&self) -> VariableBound {
        VariableBound(self.0.bound())
    }

    #[getter]
    pub fn name(&self) -> String {
        self.0.metadata.name.clone().unwrap_or_default()
    }

    #[getter]
    pub fn subscripts(&self) -> Vec<i64> {
        self.0.metadata.subscripts.clone()
    }

    #[getter]
    pub fn parameters(&self) -> HashMap<String, String> {
        self.0
            .metadata
            .parameters
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    #[getter]
    pub fn description(&self) -> String {
        self.0.metadata.description.clone().unwrap_or_default()
    }

    #[getter]
    pub fn substituted_value(&self) -> Option<f64> {
        self.0.substituted_value()
    }

    #[staticmethod]
    #[pyo3(signature = (id, name=None, subscripts=Vec::new(), parameters=HashMap::default(), description=None))]
    pub fn binary(
        id: u64,
        name: Option<String>,
        subscripts: Vec<i64>,
        parameters: HashMap<String, String>,
        description: Option<String>,
    ) -> Result<Self> {
        Self::new(
            id,
            1, // KIND_BINARY
            VariableBound(ommx::Bound::of_binary()),
            name,
            subscripts,
            parameters,
            description,
        )
    }

    #[staticmethod]
    #[pyo3(signature = (id, lower=f64::NEG_INFINITY, upper=f64::INFINITY, name=None, subscripts=Vec::new(), parameters=HashMap::default(), description=None))]
    pub fn integer(
        id: u64,
        lower: f64,
        upper: f64,
        name: Option<String>,
        subscripts: Vec<i64>,
        parameters: HashMap<String, String>,
        description: Option<String>,
    ) -> Result<Self> {
        Self::new(
            id,
            2, // KIND_INTEGER
            VariableBound(ommx::Bound::new(lower, upper)?),
            name,
            subscripts,
            parameters,
            description,
        )
    }

    #[staticmethod]
    #[pyo3(signature = (id, lower=f64::NEG_INFINITY, upper=f64::INFINITY, name=None, subscripts=Vec::new(), parameters=HashMap::default(), description=None))]
    pub fn continuous(
        id: u64,
        lower: f64,
        upper: f64,
        name: Option<String>,
        subscripts: Vec<i64>,
        parameters: HashMap<String, String>,
        description: Option<String>,
    ) -> Result<Self> {
        Self::new(
            id,
            3, // KIND_CONTINUOUS
            VariableBound(ommx::Bound::new(lower, upper)?),
            name,
            subscripts,
            parameters,
            description,
        )
    }

    #[staticmethod]
    #[pyo3(signature = (id, lower=f64::NEG_INFINITY, upper=f64::INFINITY, name=None, subscripts=Vec::new(), parameters=HashMap::default(), description=None))]
    pub fn semi_integer(
        id: u64,
        lower: f64,
        upper: f64,
        name: Option<String>,
        subscripts: Vec<i64>,
        parameters: HashMap<String, String>,
        description: Option<String>,
    ) -> Result<Self> {
        Self::new(
            id,
            4, // KIND_SEMI_INTEGER
            VariableBound(ommx::Bound::new(lower, upper)?),
            name,
            subscripts,
            parameters,
            description,
        )
    }

    #[staticmethod]
    #[pyo3(signature = (id, lower=f64::NEG_INFINITY, upper=f64::INFINITY, name=None, subscripts=Vec::new(), parameters=HashMap::default(), description=None))]
    pub fn semi_continuous(
        id: u64,
        lower: f64,
        upper: f64,
        name: Option<String>,
        subscripts: Vec<i64>,
        parameters: HashMap<String, String>,
        description: Option<String>,
    ) -> Result<Self> {
        Self::new(
            id,
            5, // KIND_SEMI_CONTINUOUS
            VariableBound(ommx::Bound::new(lower, upper)?),
            name,
            subscripts,
            parameters,
            description,
        )
    }

    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        Ok(Self(ommx::DecisionVariable::from_bytes(bytes.as_bytes())?))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.0.to_bytes())
    }

    pub fn __repr__(&self) -> String {
        format!(
            "DecisionVariable(id={}, kind={}, name=\"{}\", bound=[{}, {}])",
            self.id(),
            self.kind(),
            self.name(),
            self.0.bound().lower(),
            self.0.bound().upper()
        )
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

    // =====================
    // Class-level constants for variable kinds
    // =====================

    #[classattr]
    const BINARY: i32 = 1;

    #[classattr]
    const INTEGER: i32 = 2;

    #[classattr]
    const CONTINUOUS: i32 = 3;

    #[classattr]
    const SEMI_INTEGER: i32 = 4;

    #[classattr]
    const SEMI_CONTINUOUS: i32 = 5;

    // =====================
    // Comparison for equality (not constraint creation)
    // =====================

    /// Compare two DecisionVariable objects for equality.
    ///
    /// This is different from `__eq__` which creates a Constraint.
    /// Use this method when you want to check if two variables represent the same variable.
    pub fn equals_to(&self, other: &DecisionVariable) -> bool {
        self.0.id() == other.0.id()
            && self.0.kind() == other.0.kind()
            && self.0.bound() == other.0.bound()
    }

    // =====================
    // Arithmetic Operators
    // =====================

    /// Negation operator: -x → Linear(-1 * x)
    pub fn __neg__(&self) -> Linear {
        Linear(ommx::Linear::single_term(
            LinearMonomial::Variable(self.0.id()),
            ommx::coeff!(-1.0),
        ))
    }

    /// Polymorphic addition: x + ... → Linear or Quadratic or Polynomial
    #[gen_stub(skip)]
    #[pyo3(name = "__add__")]
    pub fn py_add(&self, py: Python<'_>, rhs: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        // Try to extract as Rust DecisionVariable directly
        if let Ok(dv) = rhs.extract::<PyRef<DecisionVariable>>() {
            let self_linear = self.as_linear();
            let rhs_linear =
                ommx::Linear::single_term(LinearMonomial::Variable(dv.0.id()), ommx::coeff!(1.0));
            return Ok(Linear(&self_linear + &rhs_linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract as Rust Parameter directly
        if let Ok(param) = rhs.extract::<PyRef<Parameter>>() {
            let self_linear = self.as_linear();
            let rhs_linear = ommx::Linear::single_term(
                LinearMonomial::Variable(VariableID::from(param.0.id)),
                ommx::coeff!(1.0),
            );
            return Ok(Linear(&self_linear + &rhs_linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract from Python wrapper (has .raw attribute)
        if let Ok(raw) = rhs.getattr("raw") {
            if let Ok(dv) = raw.extract::<PyRef<DecisionVariable>>() {
                let self_linear = self.as_linear();
                let rhs_linear = ommx::Linear::single_term(
                    LinearMonomial::Variable(dv.0.id()),
                    ommx::coeff!(1.0),
                );
                return Ok(Linear(&self_linear + &rhs_linear)
                    .into_pyobject(py)?
                    .into_any()
                    .unbind());
            }
        }
        if let Ok(linear) = rhs.extract::<PyRef<Linear>>() {
            let self_linear = self.as_linear();
            return Ok(Linear(&self_linear + &linear.0)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        if let Ok(quad) = rhs.extract::<PyRef<Quadratic>>() {
            let self_linear = self.as_linear();
            return Ok(Quadratic(&quad.0 + &self_linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        if let Ok(poly) = rhs.extract::<PyRef<Polynomial>>() {
            let self_linear = self.as_linear();
            return Ok(Polynomial(&poly.0 + &self_linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        if let Ok(val) = rhs.extract::<f64>() {
            let self_linear = self.as_linear();
            let result = match TryInto::<ommx::Coefficient>::try_into(val) {
                Ok(coeff) => &self_linear + coeff,
                Err(ommx::CoefficientError::Zero) => self_linear,
                Err(e) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        e.to_string(),
                    ))
                }
            };
            return Ok(Linear(result).into_pyobject(py)?.into_any().unbind());
        }
        // Return NotImplemented to allow Python to try the reflected operation
        Ok(py.NotImplemented().clone_ref(py).into_any())
    }

    /// Reverse addition (lhs + self)
    #[gen_stub(skip)]
    pub fn __radd__(&self, py: Python<'_>, lhs: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        self.py_add(py, lhs) // Addition is commutative
    }

    /// Polymorphic subtraction: x - ... → Linear or Quadratic or Polynomial
    #[gen_stub(skip)]
    #[pyo3(name = "__sub__")]
    pub fn py_sub(&self, py: Python<'_>, rhs: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        // Try to extract as Rust DecisionVariable directly
        if let Ok(dv) = rhs.extract::<PyRef<DecisionVariable>>() {
            let self_linear = self.as_linear();
            let rhs_linear =
                ommx::Linear::single_term(LinearMonomial::Variable(dv.0.id()), ommx::coeff!(1.0));
            return Ok(Linear(&self_linear - &rhs_linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract as Rust Parameter directly
        if let Ok(param) = rhs.extract::<PyRef<Parameter>>() {
            let self_linear = self.as_linear();
            let rhs_linear = ommx::Linear::single_term(
                LinearMonomial::Variable(VariableID::from(param.0.id)),
                ommx::coeff!(1.0),
            );
            return Ok(Linear(&self_linear - &rhs_linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract from Python wrapper (has .raw attribute)
        if let Ok(raw) = rhs.getattr("raw") {
            if let Ok(dv) = raw.extract::<PyRef<DecisionVariable>>() {
                let self_linear = self.as_linear();
                let rhs_linear = ommx::Linear::single_term(
                    LinearMonomial::Variable(dv.0.id()),
                    ommx::coeff!(1.0),
                );
                return Ok(Linear(&self_linear - &rhs_linear)
                    .into_pyobject(py)?
                    .into_any()
                    .unbind());
            }
        }
        if let Ok(linear) = rhs.extract::<PyRef<Linear>>() {
            let self_linear = self.as_linear();
            return Ok(Linear(&self_linear - &linear.0)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        if let Ok(quad) = rhs.extract::<PyRef<Quadratic>>() {
            // self - quad = -quad + self
            let self_linear = self.as_linear();
            let mut result = -quad.0.clone();
            result += &self_linear;
            return Ok(Quadratic(result).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(poly) = rhs.extract::<PyRef<Polynomial>>() {
            let self_linear = self.as_linear();
            let mut result = -poly.0.clone();
            result += &self_linear;
            return Ok(Polynomial(result).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(val) = rhs.extract::<f64>() {
            let self_linear = self.as_linear();
            let result = match TryInto::<ommx::Coefficient>::try_into(-val) {
                Ok(coeff) => &self_linear + coeff,
                Err(ommx::CoefficientError::Zero) => self_linear,
                Err(e) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        e.to_string(),
                    ))
                }
            };
            return Ok(Linear(result).into_pyobject(py)?.into_any().unbind());
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

    /// Polymorphic multiplication: x * ... → Linear or Quadratic or Polynomial
    #[gen_stub(skip)]
    #[pyo3(name = "__mul__")]
    pub fn py_mul(&self, py: Python<'_>, rhs: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        // Try to extract as Rust DecisionVariable directly
        if let Ok(dv) = rhs.extract::<PyRef<DecisionVariable>>() {
            let self_linear = self.as_linear();
            let rhs_linear =
                ommx::Linear::single_term(LinearMonomial::Variable(dv.0.id()), ommx::coeff!(1.0));
            return Ok(Quadratic(&self_linear * &rhs_linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract as Rust Parameter directly
        if let Ok(param) = rhs.extract::<PyRef<Parameter>>() {
            let self_linear = self.as_linear();
            let rhs_linear = ommx::Linear::single_term(
                LinearMonomial::Variable(VariableID::from(param.0.id)),
                ommx::coeff!(1.0),
            );
            return Ok(Quadratic(&self_linear * &rhs_linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract from Python wrapper (has .raw attribute)
        if let Ok(raw) = rhs.getattr("raw") {
            if let Ok(dv) = raw.extract::<PyRef<DecisionVariable>>() {
                let self_linear = self.as_linear();
                let rhs_linear = ommx::Linear::single_term(
                    LinearMonomial::Variable(dv.0.id()),
                    ommx::coeff!(1.0),
                );
                return Ok(Quadratic(&self_linear * &rhs_linear)
                    .into_pyobject(py)?
                    .into_any()
                    .unbind());
            }
        }
        if let Ok(linear) = rhs.extract::<PyRef<Linear>>() {
            let self_linear = self.as_linear();
            return Ok(Quadratic(&self_linear * &linear.0)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        if let Ok(quad) = rhs.extract::<PyRef<Quadratic>>() {
            let self_linear = self.as_linear();
            return Ok(Polynomial(&self_linear * &quad.0)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        if let Ok(poly) = rhs.extract::<PyRef<Polynomial>>() {
            let self_linear = self.as_linear();
            return Ok(Polynomial(&self_linear * &poly.0)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        if let Ok(val) = rhs.extract::<f64>() {
            let result = match TryInto::<ommx::Coefficient>::try_into(val) {
                Ok(coeff) => {
                    ommx::Linear::single_term(LinearMonomial::Variable(self.0.id()), coeff)
                }
                Err(ommx::CoefficientError::Zero) => ommx::Linear::default(),
                Err(e) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        e.to_string(),
                    ))
                }
            };
            return Ok(Linear(result).into_pyobject(py)?.into_any().unbind());
        }
        // Return NotImplemented to allow Python to try the reflected operation
        Ok(py.NotImplemented().clone_ref(py).into_any())
    }

    /// Reverse multiplication (lhs * self)
    #[gen_stub(skip)]
    pub fn __rmul__(&self, py: Python<'_>, lhs: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        self.py_mul(py, lhs) // Multiplication is commutative
    }

    // =====================
    // Comparison Operators (return Constraint)
    // =====================

    /// Create an equality constraint: self == other → Constraint with EqualToZero
    #[gen_stub(type_ignore = ["override"])]
    #[pyo3(name = "__eq__")]
    pub fn py_eq(&self, other: Function) -> Constraint {
        let mut function = -other.0;
        function += &self.as_linear();
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
        let mut function = -other.0;
        function += &self.as_linear();
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
        let function = other.0 - &self.as_linear();
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
