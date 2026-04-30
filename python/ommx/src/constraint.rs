use crate::{Equality, EvaluatedConstraint, Function, Instance, State};
use fnv::FnvHashMap;
use ommx::Evaluate;
use pyo3::{exceptions::PyKeyError, prelude::*, Bound, PyAny};
use std::collections::HashMap;

/// Constraint wrapper for Python.
///
/// Carries the inner Rust `Constraint<Created>` plus a snapshot of its
/// auxiliary metadata. When this wrapper is read from an [`Instance`], the
/// snapshot is filled from the instance's `ConstraintMetadataStore`. When the
/// wrapper is handed back to an instance (e.g. via `from_components`), the
/// snapshot is drained into that instance's metadata store. Mutations on a
/// wrapper retrieved from an instance therefore do not propagate back; the
/// caller must re-add the constraint to apply changes.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct Constraint(pub ommx::Constraint, pub ommx::ConstraintMetadata);

impl Constraint {
    /// Create a wrapper holding `inner` with empty (default) metadata.
    pub fn standalone(inner: ommx::Constraint) -> Self {
        Self(inner, ommx::ConstraintMetadata::default())
    }

    /// Create a wrapper from explicit `(inner, metadata)` parts.
    pub fn from_parts(inner: ommx::Constraint, metadata: ommx::ConstraintMetadata) -> Self {
        Self(inner, metadata)
    }
}

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
    /// - `name`: Optional name for the constraint
    /// - `subscripts`: Optional subscripts for indexing
    /// - `description`: Optional description
    /// - `parameters`: Optional key-value parameters
    #[new]
    #[pyo3(signature = (*, function, equality, name=None, subscripts=Vec::new(), description=None, parameters=HashMap::default()))]
    pub fn new(
        function: Function,
        equality: Equality,
        name: Option<String>,
        subscripts: Vec<i64>,
        description: Option<String>,
        parameters: HashMap<String, String>,
    ) -> PyResult<Self> {
        let rust_function = function.0;
        let rust_equality = equality.into();

        let constraint = ommx::Constraint {
            equality: rust_equality,
            stage: ommx::CreatedData {
                function: rust_function,
            },
        };
        let metadata = ommx::ConstraintMetadata {
            name,
            subscripts,
            parameters: parameters.into_iter().collect(),
            description,
            provenance: Vec::new(),
        };

        Ok(Self(constraint, metadata))
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
        self.1.name.clone()
    }

    #[getter]
    pub fn subscripts(&self) -> Vec<i64> {
        self.1.subscripts.clone()
    }

    #[getter]
    pub fn description(&self) -> Option<String> {
        self.1.description.clone()
    }

    #[getter]
    pub fn parameters(&self) -> HashMap<String, String> {
        self.1
            .parameters
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// The chain of transformations that produced this constraint.
    ///
    /// Empty for directly-authored constraints. When a special constraint
    /// (one-hot / SOS1 / indicator) is converted into a regular constraint,
    /// a {class}`~ommx.v1.Provenance` entry recording the original constraint
    /// is appended. Older entries come first, newer last; the immediate
    /// parent is therefore the last element.
    #[getter]
    pub fn provenance(&self) -> Vec<crate::Provenance> {
        crate::provenance_list(&self.1)
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
        Ok(EvaluatedConstraint::from_parts(evaluated, self.1.clone()))
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
        self.1.name = Some(name);
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
        self.1.subscripts = subscripts;
        self.clone()
    }

    /// Add subscripts to the constraint
    /// Returns self for method chaining
    pub fn add_subscripts(&mut self, subscripts: Vec<i64>) -> Self {
        self.1.subscripts.extend(subscripts);
        self.clone()
    }

    /// Set the description of the constraint
    /// Returns self for method chaining
    pub fn set_description(&mut self, description: String) -> Self {
        self.1.description = Some(description);
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
        self.1.parameters = parameters.into_iter().collect();
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
        self.1.parameters.insert(key, value);
        self.clone()
    }

    /// Create an indicator constraint from this constraint.
    ///
    /// Returns an IndicatorConstraint where `indicator_variable = 1 → this constraint`.
    pub fn with_indicator(
        &self,
        indicator_variable: &crate::DecisionVariable,
    ) -> crate::IndicatorConstraint {
        let ic = ommx::IndicatorConstraint::new(
            indicator_variable.0.id(),
            self.0.equality,
            self.0.stage.function.clone(),
        );
        crate::IndicatorConstraint(ic, self.1.clone())
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
/// Holds the inner `Constraint`, a snapshot of its metadata, and the removal
/// reason. As with [`Constraint`], the metadata snapshot does not propagate
/// back to the originating instance — it is read-only context for inspection.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct RemovedConstraint {
    pub constraint: ommx::Constraint,
    pub metadata: ommx::ConstraintMetadata,
    pub removed_reason: ommx::RemovedReason,
}

impl RemovedConstraint {
    pub fn from_parts(
        constraint: ommx::Constraint,
        metadata: ommx::ConstraintMetadata,
        removed_reason: ommx::RemovedReason,
    ) -> Self {
        Self {
            constraint,
            metadata,
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
            metadata: constraint.1,
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
        Constraint::from_parts(self.constraint.clone(), self.metadata.clone())
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
    pub fn name(&self) -> Option<String> {
        self.metadata.name.clone()
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
        self.metadata.description.clone()
    }

    /// Get the subscripts from the underlying constraint
    #[getter]
    pub fn subscripts(&self) -> Vec<i64> {
        self.metadata.subscripts.clone()
    }

    /// Get the parameters from the underlying constraint
    #[getter]
    pub fn parameters(&self) -> HashMap<String, String> {
        self.metadata
            .parameters
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Get the provenance chain from the underlying constraint.
    ///
    /// See {attr}`~ommx.v1.Constraint.provenance` for semantics.
    #[getter]
    pub fn provenance(&self) -> Vec<crate::Provenance> {
        crate::provenance_list(&self.metadata)
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

/// Attached constraint — a write-through handle bound to an [`Instance`].
///
/// `AttachedConstraint` is returned by [`Instance.add_constraint`] and by
/// `instance.constraints[id]`. Unlike [`Constraint`], which is a snapshot,
/// reads pull live data from the parent instance and metadata setters write
/// through to its SoA metadata store. Two `AttachedConstraint` instances
/// pointing at the same id observe the same state.
///
/// The handle keeps the parent `Instance` alive through a refcount; drop
/// the wrapper to release the back-reference.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
pub struct AttachedConstraint {
    instance: Py<Instance>,
    id: ommx::ConstraintID,
}

impl AttachedConstraint {
    pub fn new(instance: Py<Instance>, id: ommx::ConstraintID) -> Self {
        Self { instance, id }
    }
}

fn lookup_constraint<'a>(
    inst: &'a ommx::Instance,
    id: ommx::ConstraintID,
) -> PyResult<&'a ommx::Constraint> {
    inst.constraints()
        .get(&id)
        .or_else(|| inst.removed_constraints().get(&id).map(|(c, _)| c))
        .ok_or_else(|| {
            PyKeyError::new_err(format!(
                "constraint id {} not found in instance",
                id.into_inner()
            ))
        })
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl AttachedConstraint {
    /// The id this handle points at.
    #[getter]
    pub fn constraint_id(&self) -> u64 {
        self.id.into_inner()
    }

    /// The parent {class}`~ommx.v1.Instance` this constraint lives in.
    #[getter]
    pub fn instance(&self, py: Python<'_>) -> Py<Instance> {
        self.instance.clone_ref(py)
    }

    /// Return a {class}`~ommx.v1.Constraint` snapshot of the current
    /// state. Mutations on the returned object do not propagate back.
    pub fn detach(&self, py: Python<'_>) -> PyResult<Constraint> {
        let inst = self.instance.borrow(py);
        let c = lookup_constraint(&inst.inner, self.id)?.clone();
        let metadata = inst.inner.constraint_metadata().collect_for(self.id);
        Ok(Constraint(c, metadata))
    }

    #[getter]
    pub fn function(&self, py: Python<'_>) -> PyResult<Function> {
        let inst = self.instance.borrow(py);
        Ok(Function(
            lookup_constraint(&inst.inner, self.id)?
                .stage
                .function
                .clone(),
        ))
    }

    #[getter]
    pub fn equality(&self, py: Python<'_>) -> PyResult<Equality> {
        let inst = self.instance.borrow(py);
        Ok(lookup_constraint(&inst.inner, self.id)?.equality.into())
    }

    #[getter]
    pub fn name(&self, py: Python<'_>) -> Option<String> {
        let inst = self.instance.borrow(py);
        inst.inner
            .constraint_metadata()
            .name(self.id)
            .map(str::to_owned)
    }

    #[getter]
    pub fn subscripts(&self, py: Python<'_>) -> Vec<i64> {
        let inst = self.instance.borrow(py);
        inst.inner
            .constraint_metadata()
            .subscripts(self.id)
            .to_vec()
    }

    #[getter]
    pub fn description(&self, py: Python<'_>) -> Option<String> {
        let inst = self.instance.borrow(py);
        inst.inner
            .constraint_metadata()
            .description(self.id)
            .map(str::to_owned)
    }

    #[getter]
    pub fn parameters(&self, py: Python<'_>) -> HashMap<String, String> {
        let inst = self.instance.borrow(py);
        inst.inner
            .constraint_metadata()
            .parameters(self.id)
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    #[getter]
    pub fn provenance(&self, py: Python<'_>) -> Vec<crate::Provenance> {
        let inst = self.instance.borrow(py);
        inst.inner
            .constraint_metadata()
            .provenance(self.id)
            .iter()
            .map(crate::Provenance::from)
            .collect()
    }

    /// Set the name. Writes through to the parent instance's SoA store.
    pub fn set_name(&self, py: Python<'_>, name: String) {
        self.instance
            .borrow_mut(py)
            .inner
            .constraint_metadata_mut()
            .set_name(self.id, name);
    }

    /// Alias for {meth}`set_name` (backward compatibility).
    pub fn add_name(&self, py: Python<'_>, name: String) {
        self.set_name(py, name);
    }

    /// Set the subscripts. Writes through to the parent instance's SoA store.
    pub fn set_subscripts(&self, py: Python<'_>, subscripts: Vec<i64>) {
        self.instance
            .borrow_mut(py)
            .inner
            .constraint_metadata_mut()
            .set_subscripts(self.id, subscripts);
    }

    /// Append subscripts. Writes through to the parent instance's SoA store.
    pub fn add_subscripts(&self, py: Python<'_>, subscripts: Vec<i64>) {
        self.instance
            .borrow_mut(py)
            .inner
            .constraint_metadata_mut()
            .extend_subscripts(self.id, subscripts);
    }

    /// Set the description. Writes through to the parent instance's SoA store.
    pub fn set_description(&self, py: Python<'_>, description: String) {
        self.instance
            .borrow_mut(py)
            .inner
            .constraint_metadata_mut()
            .set_description(self.id, description);
    }

    /// Alias for {meth}`set_description` (backward compatibility).
    pub fn add_description(&self, py: Python<'_>, description: String) {
        self.set_description(py, description);
    }

    /// Replace all parameters. Writes through to the parent instance's SoA store.
    pub fn set_parameters(&self, py: Python<'_>, parameters: HashMap<String, String>) {
        let params: FnvHashMap<String, String> = parameters.into_iter().collect();
        self.instance
            .borrow_mut(py)
            .inner
            .constraint_metadata_mut()
            .set_parameters(self.id, params);
    }

    /// Alias for {meth}`set_parameters` (backward compatibility).
    pub fn add_parameters(&self, py: Python<'_>, parameters: HashMap<String, String>) {
        self.set_parameters(py, parameters);
    }

    /// Add a single parameter entry. Writes through to the parent instance's SoA store.
    pub fn add_parameter(&self, py: Python<'_>, key: String, value: String) {
        self.instance
            .borrow_mut(py)
            .inner
            .constraint_metadata_mut()
            .set_parameter(self.id, key, value);
    }

    /// Evaluate the constraint with the given state.
    #[pyo3(signature = (state, *, atol=None))]
    pub fn evaluate(
        &self,
        py: Python<'_>,
        state: State,
        atol: Option<f64>,
    ) -> PyResult<EvaluatedConstraint> {
        let snapshot = self.detach(py)?;
        snapshot.evaluate(state, atol)
    }

    pub fn __repr__(&self, py: Python<'_>) -> String {
        let inst = self.instance.borrow(py);
        match lookup_constraint(&inst.inner, self.id) {
            Ok(c) => c.to_string(),
            Err(_) => format!("AttachedConstraint(id={}, dropped)", self.id.into_inner()),
        }
    }

    fn __copy__(&self, py: Python<'_>) -> Self {
        Self {
            instance: self.instance.clone_ref(py),
            id: self.id,
        }
    }

    fn __deepcopy__(&self, py: Python<'_>, _memo: Bound<'_, PyAny>) -> Self {
        // The wrapper is a refcounted handle, not a value type — deepcopy
        // shares the same parent Instance.
        self.__copy__(py)
    }
}
