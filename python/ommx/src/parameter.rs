use crate::{extract_to_function, next_constraint_id, Constraint, Linear, Polynomial, Quadratic};
use anyhow::Result;
use ommx::{LinearMonomial, Message, VariableID};
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict},
    Bound, PyAny,
};
use std::collections::HashMap;

/// Parameter in an optimization problem.
///
/// Parameters are values that are fixed during optimization but may vary between different
/// runs or scenarios. They share the same ID space with decision variables.
///
/// Note that this object overloads `==` for creating a constraint, not for equality comparison.
///
/// Example
/// -------
/// >>> p = Parameter(1, name="penalty")
/// >>> x = DecisionVariable.integer(2)
/// >>> x + p  # Returns Linear expression
/// Linear(...)
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct Parameter(pub ommx::v1::Parameter);

impl Parameter {
    /// Helper to create a Linear term from this parameter with coefficient 1
    fn as_linear(&self) -> ommx::Linear {
        ommx::Linear::single_term(
            LinearMonomial::Variable(VariableID::from(self.0.id)),
            ommx::coeff!(1.0),
        )
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

        dict.set_item("id", self.0.id)?;

        match &self.0.name {
            Some(name) if !name.is_empty() => dict.set_item("name", name)?,
            _ => dict.set_item("name", na)?,
        }
        dict.set_item("subscripts", self.0.subscripts.clone())?;
        match &self.0.description {
            Some(desc) if !desc.is_empty() => dict.set_item("description", desc)?,
            _ => dict.set_item("description", na)?,
        }

        for (key, value) in &self.0.parameters {
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
        class Parameter:
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
impl Parameter {
    /// Create a new Parameter.
    ///
    /// Args:
    ///     id: Unique identifier for the parameter (must be unique within the instance
    ///         including decision variables)
    ///     name: Optional name for the parameter
    ///     subscripts: Optional subscripts for indexing
    ///     parameters: Optional metadata key-value pairs
    ///     description: Optional human-readable description
    #[new]
    #[pyo3(signature = (id, name=None, subscripts=Vec::new(), parameters=HashMap::default(), description=None))]
    pub fn new(
        id: u64,
        name: Option<String>,
        subscripts: Vec<i64>,
        parameters: HashMap<String, String>,
        description: Option<String>,
    ) -> Self {
        let mut param = ommx::v1::Parameter::default();
        param.id = id;
        param.name = name;
        param.subscripts = subscripts;
        param.parameters = parameters;
        param.description = description;
        Self(param)
    }

    #[getter]
    pub fn id(&self) -> u64 {
        self.0.id
    }

    #[getter]
    pub fn name(&self) -> String {
        self.0.name.clone().unwrap_or_default()
    }

    #[getter]
    pub fn subscripts(&self) -> Vec<i64> {
        self.0.subscripts.clone()
    }

    #[getter]
    pub fn parameters(&self) -> HashMap<String, String> {
        self.0.parameters.clone()
    }

    #[getter]
    pub fn description(&self) -> String {
        self.0.description.clone().unwrap_or_default()
    }

    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::v1::Parameter::decode(bytes.as_bytes())?;
        Ok(Self(inner))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.0.encode_to_vec())
    }

    pub fn __repr__(&self) -> String {
        format!("Parameter(id={}, name=\"{}\")", self.id(), self.name(),)
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }

    // =====================
    // Arithmetic Operators
    // =====================

    /// Negation operator: -p → Linear(-1 * p)
    pub fn __neg__(&self) -> Linear {
        Linear(ommx::Linear::single_term(
            LinearMonomial::Variable(VariableID::from(self.0.id)),
            ommx::coeff!(-1.0),
        ))
    }

    /// Polymorphic addition: p + ... → Linear or Quadratic or Polynomial
    #[gen_stub(skip)]
    #[pyo3(name = "__add__")]
    pub fn py_add(&self, py: Python<'_>, rhs: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
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
        // Try to extract as Rust DecisionVariable
        if let Ok(dv) = rhs.extract::<PyRef<crate::DecisionVariable>>() {
            let self_linear = self.as_linear();
            let rhs_linear =
                ommx::Linear::single_term(LinearMonomial::Variable(dv.0.id()), ommx::coeff!(1.0));
            return Ok(Linear(&self_linear + &rhs_linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract from Python wrapper (has .raw attribute)
        if let Ok(raw) = rhs.getattr("raw") {
            if let Ok(dv) = raw.extract::<PyRef<crate::DecisionVariable>>() {
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

    /// Polymorphic subtraction: p - ... → Linear or Quadratic or Polynomial
    #[gen_stub(skip)]
    #[pyo3(name = "__sub__")]
    pub fn py_sub(&self, py: Python<'_>, rhs: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
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
        // Try to extract as Rust DecisionVariable
        if let Ok(dv) = rhs.extract::<PyRef<crate::DecisionVariable>>() {
            let self_linear = self.as_linear();
            let rhs_linear =
                ommx::Linear::single_term(LinearMonomial::Variable(dv.0.id()), ommx::coeff!(1.0));
            return Ok(Linear(&self_linear - &rhs_linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract from Python wrapper (has .raw attribute)
        if let Ok(raw) = rhs.getattr("raw") {
            if let Ok(dv) = raw.extract::<PyRef<crate::DecisionVariable>>() {
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
        let neg = self.__neg__();
        neg.py_add(py, lhs)
    }

    /// Polymorphic multiplication: p * ... → Linear or Quadratic or Polynomial
    #[gen_stub(skip)]
    #[pyo3(name = "__mul__")]
    pub fn py_mul(&self, py: Python<'_>, rhs: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
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
        // Try to extract as Rust DecisionVariable
        if let Ok(dv) = rhs.extract::<PyRef<crate::DecisionVariable>>() {
            let self_linear = self.as_linear();
            let rhs_linear =
                ommx::Linear::single_term(LinearMonomial::Variable(dv.0.id()), ommx::coeff!(1.0));
            return Ok(Quadratic(&self_linear * &rhs_linear)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        // Try to extract from Python wrapper (has .raw attribute)
        if let Ok(raw) = rhs.getattr("raw") {
            if let Ok(dv) = raw.extract::<PyRef<crate::DecisionVariable>>() {
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
                Ok(coeff) => ommx::Linear::single_term(
                    LinearMonomial::Variable(VariableID::from(self.0.id)),
                    coeff,
                ),
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

    /// Create a less-than-or-equal constraint: self <= other → Constraint
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

    /// Create a greater-than-or-equal constraint: self >= other → Constraint
    #[pyo3(name = "__ge__")]
    pub fn py_ge(&self, py: Python<'_>, other: &Bound<PyAny>) -> PyResult<Constraint> {
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
