use crate::{next_constraint_id, Constraint, EvaluatedNamedFunction, Function, State};
use ommx::{Evaluate, NamedFunctionID};
use pyo3::{prelude::*, types::PyBytes, Bound, PyAny};
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
    /// **Args:**
    ///
    /// - `id`: The unique identifier for this named function
    /// - `function`: The function (int, float, DecisionVariable, Linear, Quadratic, Polynomial, or Function)
    /// - `name`: Optional name for the function
    /// - `subscripts`: Optional subscripts for indexing
    /// - `description`: Optional description
    /// - `parameters`: Optional key-value parameters
    #[new]
    #[pyo3(signature = (*, id, function, name=None, subscripts=Vec::new(), description=None, parameters=HashMap::default()))]
    pub fn new(
        id: u64,
        function: Function,
        name: Option<String>,
        subscripts: Vec<i64>,
        description: Option<String>,
        parameters: HashMap<String, String>,
    ) -> PyResult<Self> {
        let rust_function = function.0;
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
    /// **Args:**
    ///
    /// - `state`: A State object, dict[int, float], or iterable of (int, float) tuples
    /// - `atol`: Optional absolute tolerance for evaluation
    ///
    /// **Returns:** {class}`~ommx.v1.EvaluatedNamedFunction` containing the evaluated value
    #[pyo3(signature = (state, *, atol=None))]
    pub fn evaluate(&self, state: State, atol: Option<f64>) -> PyResult<EvaluatedNamedFunction> {
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
    /// **Args:**
    ///
    /// - `state`: A State object, dict[int, float], or iterable of (int, float) tuples
    /// - `atol`: Optional absolute tolerance for evaluation
    ///
    /// **Returns:** Self (modified in-place) for method chaining
    #[pyo3(signature = (state, *, atol=None))]
    pub fn partial_evaluate(&mut self, state: State, atol: Option<f64>) -> PyResult<Self> {
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

    /// Addition: returns self.function + other
    pub fn __add__(&self, other: Function) -> Function {
        self.function().__add__(other)
    }

    /// Reverse addition: returns other + self.function
    pub fn __radd__(&self, other: Function) -> Function {
        self.function().__add__(other)
    }

    /// Subtraction: returns self.function - other
    pub fn __sub__(&self, other: Function) -> Function {
        self.function().__sub__(other)
    }

    /// Reverse subtraction: returns other - self.function
    pub fn __rsub__(&self, other: Function) -> Function {
        Function(&other.0 - &self.0.function)
    }

    /// Multiplication: returns self.function * other
    pub fn __mul__(&self, other: Function) -> Function {
        self.function().__mul__(other)
    }

    /// Reverse multiplication: returns other * self.function
    pub fn __rmul__(&self, other: Function) -> Function {
        self.function().__mul__(other)
    }

    /// Negation: returns -self.function
    pub fn __neg__(&self) -> Function {
        Function(-self.0.function.clone())
    }

    // Comparison operators - return Constraint
    // These accept Function via FromPyObject and return Constraint directly

    /// Create an equality constraint: self.function == other → Constraint with EqualToZero
    ///
    /// Returns a Constraint where (self.function - other) == 0.
    /// Note: This does NOT return bool, it creates a Constraint object.
    #[gen_stub(type_ignore = ["override"])]
    #[pyo3(name = "__eq__")]
    pub fn py_eq(&self, other: Function) -> Constraint {
        let mut function = -other.0;
        function += &self.0.function;
        let id = next_constraint_id();
        Constraint(ommx::Constraint {
            id: ommx::ConstraintID::from(id),
            equality: ommx::Equality::EqualToZero,
            metadata: ommx::ConstraintMetadata::default(),
            stage: ommx::CreatedData { function },
        })
    }

    /// Create a less-than-or-equal constraint: self.function <= other → Constraint with LessThanOrEqualToZero
    ///
    /// Returns a Constraint where (self.function - other) <= 0.
    #[pyo3(name = "__le__")]
    pub fn py_le(&self, other: Function) -> Constraint {
        let mut function = -other.0;
        function += &self.0.function;
        let id = next_constraint_id();
        Constraint(ommx::Constraint {
            id: ommx::ConstraintID::from(id),
            equality: ommx::Equality::LessThanOrEqualToZero,
            metadata: ommx::ConstraintMetadata::default(),
            stage: ommx::CreatedData { function },
        })
    }

    /// Create a greater-than-or-equal constraint: self.function >= other → Constraint with LessThanOrEqualToZero
    ///
    /// Returns a Constraint where (other - self.function) <= 0.
    #[pyo3(name = "__ge__")]
    pub fn py_ge(&self, other: Function) -> Constraint {
        let function = other.0 - &self.0.function;
        let id = next_constraint_id();
        Constraint(ommx::Constraint {
            id: ommx::ConstraintID::from(id),
            equality: ommx::Equality::LessThanOrEqualToZero,
            metadata: ommx::ConstraintMetadata::default(),
            stage: ommx::CreatedData { function },
        })
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
