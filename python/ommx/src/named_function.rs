use crate::{next_constraint_id, Constraint, Function};
use anyhow::Result;
use ommx::{Evaluate, Message, NamedFunctionID};
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict, PyList},
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
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        Ok(Self(ommx::NamedFunction::from_bytes(bytes.as_bytes())?))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.0.to_bytes())
    }

    #[pyo3(signature = (state, *, atol=None))]
    pub fn evaluate<'py>(
        &self,
        py: Python<'py>,
        state: &Bound<PyBytes>,
        atol: Option<f64>,
    ) -> Result<Bound<'py, PyBytes>> {
        let state = ommx::v1::State::decode(state.as_bytes())?;
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        let evaluated = self.0.evaluate(&state, atol)?;
        let v1_evaluated: ommx::v1::EvaluatedNamedFunction = evaluated.into();
        Ok(PyBytes::new(py, &v1_evaluated.encode_to_vec()))
    }

    #[pyo3(signature = (state, *, atol=None))]
    pub fn partial_evaluate<'py>(
        &mut self,
        py: Python<'py>,
        state: &Bound<PyBytes>,
        atol: Option<f64>,
    ) -> Result<Bound<'py, PyBytes>> {
        let state = ommx::v1::State::decode(state.as_bytes())?;
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        self.0.partial_evaluate(&state, atol)?;
        let inner: ommx::v1::NamedFunction = self.0.clone().into();
        Ok(PyBytes::new(py, &inner.encode_to_vec()))
    }

    // Arithmetic operators - delegate to the inner function

    /// Addition: returns self.function + other
    pub fn __add__(&self, py: Python<'_>, other: &Bound<PyAny>) -> PyResult<Function> {
        self.function()
            .py_add(py, other)
            .and_then(|obj| Ok(obj.extract::<Function>(py)?))
    }

    /// Reverse addition: returns other + self.function
    pub fn __radd__(&self, py: Python<'_>, other: &Bound<PyAny>) -> PyResult<Function> {
        self.function()
            .py_add(py, other)
            .and_then(|obj| Ok(obj.extract::<Function>(py)?))
    }

    /// Subtraction: returns self.function - other
    pub fn __sub__(&self, py: Python<'_>, other: &Bound<PyAny>) -> PyResult<Function> {
        self.function()
            .py_sub(py, other)
            .and_then(|obj| Ok(obj.extract::<Function>(py)?))
    }

    /// Reverse subtraction: returns other - self.function
    pub fn __rsub__(&self, py: Python<'_>, other: &Bound<PyAny>) -> PyResult<Function> {
        // other - self = -self + other
        let neg_self = self.__neg__();
        neg_self
            .py_add(py, other)
            .and_then(|obj| Ok(obj.extract::<Function>(py)?))
    }

    /// Multiplication: returns self.function * other
    pub fn __mul__(&self, py: Python<'_>, other: &Bound<PyAny>) -> PyResult<Function> {
        self.function()
            .py_mul(py, other)
            .and_then(|obj| Ok(obj.extract::<Function>(py)?))
    }

    /// Reverse multiplication: returns other * self.function
    pub fn __rmul__(&self, py: Python<'_>, other: &Bound<PyAny>) -> PyResult<Function> {
        self.function()
            .py_mul(py, other)
            .and_then(|obj| Ok(obj.extract::<Function>(py)?))
    }

    /// Negation: returns -self.function
    pub fn __neg__(&self) -> Function {
        Function(-self.0.function.clone())
    }

    // Comparison operators - return Constraint

    /// Create an equality constraint: self.function == other → Constraint with EqualToZero
    ///
    /// Returns a Constraint where (self.function - other) == 0.
    /// Note: This does NOT return bool, it creates a Constraint object.
    #[gen_stub(type_ignore = ["override"])]
    #[pyo3(name = "__eq__")]
    pub fn py_eq(&self, py: Python<'_>, other: &Bound<PyAny>) -> PyResult<Constraint> {
        // self.function - other
        let diff = self.function().py_sub(py, other)?;
        let diff_func = diff.extract::<Function>(py)?;
        let id = next_constraint_id();
        Ok(Constraint(ommx::Constraint {
            id: ommx::ConstraintID::from(id),
            function: diff_func.0,
            equality: ommx::Equality::EqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: Default::default(),
            description: None,
        }))
    }

    /// Create a less-than-or-equal constraint: self.function <= other → Constraint with LessThanOrEqualToZero
    ///
    /// Returns a Constraint where (self.function - other) <= 0.
    #[pyo3(name = "__le__")]
    pub fn py_le(&self, py: Python<'_>, other: &Bound<PyAny>) -> PyResult<Constraint> {
        // self.function - other <= 0
        let diff = self.function().py_sub(py, other)?;
        let diff_func = diff.extract::<Function>(py)?;
        let id = next_constraint_id();
        Ok(Constraint(ommx::Constraint {
            id: ommx::ConstraintID::from(id),
            function: diff_func.0,
            equality: ommx::Equality::LessThanOrEqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: Default::default(),
            description: None,
        }))
    }

    /// Create a greater-than-or-equal constraint: self.function >= other → Constraint with LessThanOrEqualToZero
    ///
    /// Returns a Constraint where (other - self.function) <= 0.
    #[pyo3(name = "__ge__")]
    pub fn py_ge(&self, py: Python<'_>, other: &Bound<PyAny>) -> PyResult<Constraint> {
        // self.function >= other is equivalent to other - self.function <= 0
        let neg_self = self.__neg__();
        let diff = neg_self.py_add(py, other)?;
        let diff_func = diff.extract::<Function>(py)?;
        let id = next_constraint_id();
        Ok(Constraint(ommx::Constraint {
            id: ommx::ConstraintID::from(id),
            function: diff_func.0,
            equality: ommx::Equality::LessThanOrEqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: Default::default(),
            description: None,
        }))
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

        // Get used variable IDs
        let used_ids: Vec<u64> = self
            .0
            .function
            .required_ids()
            .iter()
            .map(|id| id.into_inner())
            .collect();
        let used_ids_list = PyList::new(py, used_ids)?;
        dict.set_item("used_ids", used_ids_list)?;

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
