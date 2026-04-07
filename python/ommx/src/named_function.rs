use crate::{next_constraint_id, Constraint, EvaluatedNamedFunction, Function, State};
use ommx::{Evaluate, NamedFunctionID};
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict, PySet},
    Bound, PyAny,
};
use std::collections::HashMap;

/// NamedFunction wrapper for Python
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct NamedFunction(pub ommx::NamedFunction);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl NamedFunction {
    /// Create a new NamedFunction.
    ///
    /// Args:
    ///     id: The unique identifier for this named function
    ///     function: The function (int, float, DecisionVariable, Linear, Quadratic, Polynomial, or Function)
    ///     name: Optional name for the function
    ///     subscripts: Optional subscripts for indexing
    ///     description: Optional description
    ///     parameters: Optional key-value parameters
    #[new]
    #[pyo3(signature = (*, id, function, name=None, subscripts=Vec::new(), description=None, parameters=HashMap::default()))]
    pub fn new(
        id: u64,
        function: &Bound<PyAny>,
        name: Option<String>,
        subscripts: Vec<i64>,
        description: Option<String>,
        parameters: HashMap<String, String>,
    ) -> PyResult<Self> {
        // Extract function from polymorphic input using Function::new
        let rust_function = Function::new(function)?.0;
        let named_function_id = NamedFunctionID::from(id);

        let named_function = ommx::NamedFunction {
            id: named_function_id,
            function: rust_function,
            name,
            subscripts,
            parameters: parameters.into_iter().collect(),
            description,
        };

        Ok(Self(named_function))
    }

    #[getter]
    pub fn id(&self) -> u64 {
        self.0.id.into_inner()
    }

    #[getter]
    pub fn function(&self) -> Function {
        Function(self.0.function.clone())
    }

    #[getter]
    pub fn name(&self) -> Option<String> {
        self.0.name.clone()
    }

    #[getter]
    pub fn subscripts(&self) -> Vec<i64> {
        self.0.subscripts.clone()
    }

    #[getter]
    pub fn parameters(&self) -> HashMap<String, String> {
        self.0.parameters.clone().into_iter().collect()
    }

    #[getter]
    pub fn description(&self) -> Option<String> {
        self.0.description.clone()
    }

    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> PyResult<Self> {
        ommx::NamedFunction::from_bytes(bytes.as_bytes())
            .map(Self)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.0.to_bytes())
    }

    /// Evaluate the named function with the given state.
    ///
    /// Args:
    ///     state: A State object, dict[int, float], or iterable of (int, float) tuples
    ///     atol: Optional absolute tolerance for evaluation
    ///
    /// Returns:
    ///     EvaluatedNamedFunction containing the evaluated value
    #[pyo3(signature = (state, *, atol=None))]
    pub fn evaluate(
        &self,
        state: &Bound<PyAny>,
        atol: Option<f64>,
    ) -> PyResult<EvaluatedNamedFunction> {
        let state = State::new(state)?;
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?,
            None => ommx::ATol::default(),
        };
        let evaluated = self
            .0
            .evaluate(&state.0, atol)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(EvaluatedNamedFunction(evaluated))
    }

    /// Partially evaluate the named function with the given state.
    ///
    /// This modifies self in-place and returns self for method chaining.
    ///
    /// Args:
    ///     state: A State object, dict[int, float], or iterable of (int, float) tuples
    ///     atol: Optional absolute tolerance for evaluation
    ///
    /// Returns:
    ///     Self (modified in-place) for method chaining
    #[pyo3(signature = (state, *, atol=None))]
    pub fn partial_evaluate(&mut self, state: &Bound<PyAny>, atol: Option<f64>) -> PyResult<Self> {
        let state = State::new(state)?;
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?,
            None => ommx::ATol::default(),
        };
        self.0
            .partial_evaluate(&state.0, atol)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(self.clone())
    }

    // Arithmetic operators - delegate to the inner function
    // These return Py<PyAny> to allow NotImplemented to propagate for reflected operations

    /// Addition: returns self.function + other
    #[gen_stub(override_return_type(type_repr = "Function"))]
    pub fn __add__(&self, py: Python<'_>, other: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        self.function().py_add(py, other)
    }

    /// Reverse addition: returns other + self.function
    #[gen_stub(override_return_type(type_repr = "Function"))]
    pub fn __radd__(&self, py: Python<'_>, other: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        self.function().py_add(py, other)
    }

    /// Subtraction: returns self.function - other
    #[gen_stub(override_return_type(type_repr = "Function"))]
    pub fn __sub__(&self, py: Python<'_>, other: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        self.function().py_sub(py, other)
    }

    /// Reverse subtraction: returns other - self.function
    #[gen_stub(override_return_type(type_repr = "Function"))]
    pub fn __rsub__(&self, py: Python<'_>, other: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        // other - self = -self + other
        let neg_self = self.__neg__();
        neg_self.py_add(py, other)
    }

    /// Multiplication: returns self.function * other
    #[gen_stub(override_return_type(type_repr = "Function"))]
    pub fn __mul__(&self, py: Python<'_>, other: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        self.function().py_mul(py, other)
    }

    /// Reverse multiplication: returns other * self.function
    #[gen_stub(override_return_type(type_repr = "Function"))]
    pub fn __rmul__(&self, py: Python<'_>, other: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        self.function().py_mul(py, other)
    }

    /// Negation: returns -self.function
    pub fn __neg__(&self) -> Function {
        Function(-self.0.function.clone())
    }

    // Comparison operators - return Constraint
    // These check for NotImplemented and propagate it

    /// Create an equality constraint: self.function == other → Constraint with EqualToZero
    ///
    /// Returns a Constraint where (self.function - other) == 0.
    /// Note: This does NOT return bool, it creates a Constraint object.
    #[gen_stub(type_ignore = ["override"], override_return_type(type_repr = "Constraint"))]
    #[pyo3(name = "__eq__")]
    pub fn py_eq(&self, py: Python<'_>, other: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        // self.function - other
        let diff = self.function().py_sub(py, other)?;
        // Check if NotImplemented was returned
        if diff.bind(py).is(py.NotImplemented()) {
            return Ok(py.NotImplemented().into_any());
        }
        let diff_func = diff.extract::<Function>(py)?;
        let id = next_constraint_id();
        let constraint = Constraint(ommx::Constraint {
            id: ommx::ConstraintID::from(id),
            function: diff_func.0,
            equality: ommx::Equality::EqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: Default::default(),
            description: None,
        });
        Ok(constraint.into_pyobject(py)?.into_any().unbind())
    }

    /// Create a less-than-or-equal constraint: self.function <= other → Constraint with LessThanOrEqualToZero
    ///
    /// Returns a Constraint where (self.function - other) <= 0.
    #[gen_stub(override_return_type(type_repr = "Constraint"))]
    #[pyo3(name = "__le__")]
    pub fn py_le(&self, py: Python<'_>, other: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        // self.function - other <= 0
        let diff = self.function().py_sub(py, other)?;
        // Check if NotImplemented was returned
        if diff.bind(py).is(py.NotImplemented()) {
            return Ok(py.NotImplemented().into_any());
        }
        let diff_func = diff.extract::<Function>(py)?;
        let id = next_constraint_id();
        let constraint = Constraint(ommx::Constraint {
            id: ommx::ConstraintID::from(id),
            function: diff_func.0,
            equality: ommx::Equality::LessThanOrEqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: Default::default(),
            description: None,
        });
        Ok(constraint.into_pyobject(py)?.into_any().unbind())
    }

    /// Create a greater-than-or-equal constraint: self.function >= other → Constraint with LessThanOrEqualToZero
    ///
    /// Returns a Constraint where (other - self.function) <= 0.
    #[gen_stub(override_return_type(type_repr = "Constraint"))]
    #[pyo3(name = "__ge__")]
    pub fn py_ge(&self, py: Python<'_>, other: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        // self.function >= other is equivalent to other - self.function <= 0
        let neg_self = self.__neg__();
        let diff = neg_self.py_add(py, other)?;
        // Check if NotImplemented was returned
        if diff.bind(py).is(py.NotImplemented()) {
            return Ok(py.NotImplemented().into_any());
        }
        let diff_func = diff.extract::<Function>(py)?;
        let id = next_constraint_id();
        let constraint = Constraint(ommx::Constraint {
            id: ommx::ConstraintID::from(id),
            function: diff_func.0,
            equality: ommx::Equality::LessThanOrEqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: Default::default(),
            description: None,
        });
        Ok(constraint.into_pyobject(py)?.into_any().unbind())
    }

    /// Internal method for pandas DataFrame conversion.
    ///
    /// Returns a dictionary with named function information suitable for pandas DataFrame.
    pub fn _as_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);

        dict.set_item("id", self.0.id.into_inner())?;

        // Get function type name
        let type_name = match &self.0.function {
            ommx::Function::Zero => "Zero",
            ommx::Function::Constant(_) => "Constant",
            ommx::Function::Linear(_) => "Linear",
            ommx::Function::Quadratic(_) => "Quadratic",
            ommx::Function::Polynomial(_) => "Polynomial",
        };
        dict.set_item("type", type_name)?;

        // Store the function itself
        dict.set_item("function", Function(self.0.function.clone()))?;

        // Get used variable IDs as a set
        let used_ids: Vec<u64> = self
            .0
            .function
            .required_ids()
            .iter()
            .map(|id| id.into_inner())
            .collect();
        let used_ids_set = PySet::new(py, &used_ids)?;
        dict.set_item("used_ids", used_ids_set)?;

        // Name - use Python None for missing values (pandas NA equivalent)
        match &self.0.name {
            Some(n) => dict.set_item("name", n)?,
            None => dict.set_item("name", py.None())?,
        };

        dict.set_item("subscripts", self.0.subscripts.clone())?;

        // Description - use Python None for missing values
        match &self.0.description {
            Some(d) => dict.set_item("description", d)?,
            None => dict.set_item("description", py.None())?,
        };

        // Add parameters as separate columns
        for (key, value) in &self.0.parameters {
            dict.set_item(format!("parameters.{}", key), value)?;
        }

        Ok(dict)
    }

    pub fn __repr__(&self) -> String {
        self.0.to_string()
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}
