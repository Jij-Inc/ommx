use crate::{Equality, EvaluatedConstraint, Function, State};
use fnv::FnvHashMap;
use ommx::{ConstraintID, Evaluate};
use pyo3::{prelude::*, types::PyBytes, Bound, PyAny};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Global counter for auto-generating constraint IDs
static CONSTRAINT_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Get next constraint ID (thread-safe)
#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
pub fn next_constraint_id() -> u64 {
    CONSTRAINT_ID_COUNTER.fetch_add(1, Ordering::SeqCst)
}

/// Set constraint ID counter (for deserialization compatibility)
#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
pub fn set_constraint_id_counter(value: u64) {
    CONSTRAINT_ID_COUNTER.store(value, Ordering::SeqCst);
}

/// Update counter to ensure it's at least the given value + 1
/// Returns the new counter value after update
#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
pub fn update_constraint_id_counter(value: u64) -> u64 {
    let new_value = value + 1;
    let previous = CONSTRAINT_ID_COUNTER.fetch_max(new_value, Ordering::SeqCst);
    previous.max(new_value)
}

/// Get current constraint ID counter value
#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
pub fn get_constraint_id_counter() -> u64 {
    CONSTRAINT_ID_COUNTER.load(Ordering::SeqCst)
}

/// Constraint wrapper for Python
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct Constraint(pub ommx::Constraint);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Constraint {
    /// Class constant for equality type: equal to zero (==)
    #[classattr]
    #[pyo3(name = "EQUAL_TO_ZERO")]
    fn class_equal_to_zero() -> Equality {
        Equality::EqualToZero
    }

    /// Class constant for equality type: less than or equal to zero (<=)
    #[classattr]
    #[pyo3(name = "LESS_THAN_OR_EQUAL_TO_ZERO")]
    fn class_less_than_or_equal_to_zero() -> Equality {
        Equality::LessThanOrEqualToZero
    }

    /// Create a new Constraint.
    ///
    /// **Args:**
    ///
    /// - `function`: The constraint function (int, float, DecisionVariable, Linear, Quadratic, Polynomial, or Function)
    /// - `equality`: The equality type (EqualToZero or LessThanOrEqualToZero)
    /// - `id`: Optional constraint ID (auto-generated if not provided)
    /// - `name`: Optional name for the constraint
    /// - `subscripts`: Optional subscripts for indexing
    /// - `description`: Optional description
    /// - `parameters`: Optional key-value parameters
    #[new]
    #[pyo3(signature = (*, function, equality, id=None, name=None, subscripts=Vec::new(), description=None, parameters=HashMap::default()))]
    pub fn new(
        function: Function,
        equality: Equality,
        id: Option<u64>,
        name: Option<String>,
        subscripts: Vec<i64>,
        description: Option<String>,
        parameters: HashMap<String, String>,
    ) -> PyResult<Self> {
        let rust_function = function.0;

        // Auto-generate ID if not provided
        let constraint_id = match id {
            Some(id_val) => {
                // Update counter to ensure it's at least the given value
                update_constraint_id_counter(id_val);
                ConstraintID::from(id_val)
            }
            None => ConstraintID::from(next_constraint_id()),
        };

        let rust_equality = equality.into();

        let constraint = ommx::Constraint {
            id: constraint_id,
            equality: rust_equality,
            metadata: ommx::ConstraintMetadata {
                name,
                subscripts,
                parameters: parameters.into_iter().collect(),
                description,
                provenance: Vec::new(),
            },
            stage: ommx::CreatedData {
                function: rust_function,
            },
        };

        Ok(Self(constraint))
    }

    #[getter]
    pub fn id(&self) -> u64 {
        self.0.id.into_inner()
    }

    #[getter]
    pub fn function(&self) -> Function {
        Function(self.0.stage.function.clone())
    }

    #[getter]
    pub fn equality(&self) -> Equality {
        self.0.equality.into()
    }

    #[getter]
    pub fn name(&self) -> Option<String> {
        self.0.metadata.name.clone()
    }

    #[getter]
    pub fn subscripts(&self) -> Vec<i64> {
        self.0.metadata.subscripts.clone()
    }

    #[getter]
    pub fn description(&self) -> Option<String> {
        self.0.metadata.description.clone()
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

    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> PyResult<Self> {
        let constraint = <ommx::Constraint>::from_bytes(bytes.as_bytes())
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        // Update the ID counter to ensure new IDs don't conflict
        update_constraint_id_counter(constraint.id.into_inner());
        Ok(Self(constraint))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.0.to_bytes())
    }

    /// Evaluate the constraint with the given state.
    ///
    /// **Args:**
    ///
    /// - `state`: A State object, dict[int, float], or iterable of (int, float) tuples
    /// - `atol`: Optional absolute tolerance for evaluation
    ///
    /// **Returns:** {class}`~ommx.v1.EvaluatedConstraint` containing the evaluated value and feasibility
    #[pyo3(signature = (state, *, atol=None))]
    pub fn evaluate(&self, state: State, atol: Option<f64>) -> PyResult<EvaluatedConstraint> {
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?,
            None => ommx::ATol::default(),
        };
        let evaluated = self
            .0
            .evaluate(&state.0, atol)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(EvaluatedConstraint(evaluated))
    }

    /// Partially evaluate the constraint with the given state.
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

    /// Set the name of the constraint
    /// Returns self for method chaining
    pub fn set_name(&mut self, name: String) -> Self {
        self.0.metadata.name = Some(name);
        self.clone()
    }

    /// Alias for set_name (backward compatibility)
    /// Returns self for method chaining
    pub fn add_name(&mut self, name: String) -> Self {
        self.set_name(name)
    }

    /// Set the subscripts of the constraint
    /// Returns self for method chaining
    pub fn set_subscripts(&mut self, subscripts: Vec<i64>) -> Self {
        self.0.metadata.subscripts = subscripts;
        self.clone()
    }

    /// Add subscripts to the constraint
    /// Returns self for method chaining
    pub fn add_subscripts(&mut self, subscripts: Vec<i64>) -> Self {
        self.0.metadata.subscripts.extend(subscripts);
        self.clone()
    }

    /// Set the ID of the constraint
    /// Returns self for method chaining
    pub fn set_id(&mut self, id: u64) -> Self {
        self.0.id = ConstraintID::from(id);
        self.clone()
    }

    /// Set the description of the constraint
    /// Returns self for method chaining
    pub fn set_description(&mut self, description: String) -> Self {
        self.0.metadata.description = Some(description);
        self.clone()
    }

    /// Alias for set_description (backward compatibility)
    /// Returns self for method chaining
    pub fn add_description(&mut self, description: String) -> Self {
        self.set_description(description)
    }

    /// Set the parameters of the constraint
    /// Returns self for method chaining
    pub fn set_parameters(&mut self, parameters: HashMap<String, String>) -> Self {
        self.0.metadata.parameters = parameters.into_iter().collect();
        self.clone()
    }

    /// Alias for set_parameters (backward compatibility)
    /// Returns self for method chaining
    pub fn add_parameters(&mut self, parameters: HashMap<String, String>) -> Self {
        self.set_parameters(parameters)
    }

    /// Add a parameter to the constraint
    /// Returns self for method chaining
    pub fn add_parameter(&mut self, key: String, value: String) -> Self {
        self.0.metadata.parameters.insert(key, value);
        self.clone()
    }

    /// Create an indicator constraint from this constraint.
    ///
    /// Returns an IndicatorConstraint where `indicator_variable = 1 → this constraint`.
    pub fn with_indicator(
        &self,
        indicator_variable: &crate::DecisionVariable,
    ) -> crate::IndicatorConstraint {
        let mut ic = ommx::IndicatorConstraint::new(
            ommx::IndicatorConstraintID::from(self.0.id.into_inner()),
            indicator_variable.0.id(),
            self.0.equality,
            self.0.stage.function.clone(),
        );
        ic.metadata = self.0.metadata.clone();
        crate::IndicatorConstraint(ic)
    }

    pub fn __repr__(&self) -> String {
        self.0.to_string()
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
}

/// RemovedConstraint wrapper for Python.
///
/// Internally holds `(ommx::Constraint, ommx::RemovedReason)`.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct RemovedConstraint {
    pub constraint: ommx::Constraint,
    pub removed_reason: ommx::RemovedReason,
}

impl RemovedConstraint {
    pub fn from_pair(constraint: ommx::Constraint, removed_reason: ommx::RemovedReason) -> Self {
        Self {
            constraint,
            removed_reason,
        }
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl RemovedConstraint {
    #[new]
    #[pyo3(signature = (constraint, removed_reason, removed_reason_parameters=None))]
    pub fn new(
        constraint: Constraint,
        removed_reason: String,
        removed_reason_parameters: Option<HashMap<String, String>>,
    ) -> Self {
        Self {
            constraint: constraint.0,
            removed_reason: ommx::RemovedReason {
                reason: removed_reason,
                parameters: removed_reason_parameters
                    .map(|params| params.into_iter().collect::<FnvHashMap<_, _>>())
                    .unwrap_or_default(),
            },
        }
    }

    #[getter]
    pub fn constraint(&self) -> Constraint {
        Constraint(self.constraint.clone())
    }

    #[getter]
    pub fn removed_reason(&self) -> String {
        self.removed_reason.reason.clone()
    }

    #[getter]
    pub fn removed_reason_parameters(&self) -> HashMap<String, String> {
        self.removed_reason
            .parameters
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    #[getter]
    pub fn id(&self) -> u64 {
        self.constraint.id.into_inner()
    }

    #[getter]
    pub fn name(&self) -> Option<String> {
        self.constraint.metadata.name.clone()
    }

    /// Get the equality type from the underlying constraint
    #[getter]
    pub fn equality(&self) -> Equality {
        self.constraint.equality.into()
    }

    /// Get the function from the underlying constraint
    #[getter]
    pub fn function(&self) -> Function {
        Function(self.constraint.stage.function.clone())
    }

    /// Get the description from the underlying constraint
    #[getter]
    pub fn description(&self) -> Option<String> {
        self.constraint.metadata.description.clone()
    }

    /// Get the subscripts from the underlying constraint
    #[getter]
    pub fn subscripts(&self) -> Vec<i64> {
        self.constraint.metadata.subscripts.clone()
    }

    /// Get the parameters from the underlying constraint
    #[getter]
    pub fn parameters(&self) -> HashMap<String, String> {
        self.constraint
            .metadata
            .parameters
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> PyResult<Self> {
        use ommx::{parse::Parse, Message};
        let v1_removed = ommx::v1::RemovedConstraint::decode(bytes.as_bytes())
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        let (constraint, reason): (ommx::Constraint, ommx::RemovedReason) =
            v1_removed
                .parse(&())
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(Self::from_pair(constraint, reason))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        use ommx::Message;
        let v1: ommx::v1::RemovedConstraint =
            (self.constraint.clone(), self.removed_reason.clone()).into();
        PyBytes::new(py, &v1.encode_to_vec())
    }

    pub fn __repr__(&self) -> String {
        let equality_symbol = match self.constraint.equality {
            ommx::Equality::EqualToZero => "==",
            ommx::Equality::LessThanOrEqualToZero => "<=",
        };

        let mut reason_str = format!("reason={}", self.removed_reason.reason);
        if !self.removed_reason.parameters.is_empty() {
            let params: Vec<String> = self
                .removed_reason
                .parameters
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            reason_str = format!("{}, {}", reason_str, params.join(", "));
        }

        format!(
            "RemovedConstraint({} {} 0, {})",
            self.constraint.stage.function, equality_symbol, reason_str
        )
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}
