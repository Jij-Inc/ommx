use pyo3::{exceptions::PyKeyError, prelude::*};
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use std::collections::HashMap;

use crate::{ConstraintHost, DecisionVariable, Equality, Function};

#[gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct IndicatorConstraint(pub ommx::IndicatorConstraint, pub ommx::ConstraintMetadata);

impl IndicatorConstraint {
    pub fn standalone(inner: ommx::IndicatorConstraint) -> Self {
        Self(inner, ommx::ConstraintMetadata::default())
    }

    pub fn from_parts(
        inner: ommx::IndicatorConstraint,
        metadata: ommx::ConstraintMetadata,
    ) -> Self {
        Self(inner, metadata)
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl IndicatorConstraint {
    /// Create a new indicator constraint.
    ///
    /// An indicator constraint is: `indicator_variable = 1 → f(x) <= 0` (or `f(x) = 0`).
    ///
    /// **Args:**
    ///
    /// - `indicator_variable`: A binary decision variable that activates this constraint
    /// - `function`: The constraint function
    /// - `equality`: The equality type (EqualToZero or LessThanOrEqualToZero)
    /// - `name`: Optional name for the constraint
    /// - `subscripts`: Optional subscripts for indexing
    /// - `description`: Optional description
    /// - `parameters`: Optional key-value parameters
    #[new]
    #[pyo3(signature = (*, indicator_variable, function, equality, name=None, subscripts=Vec::new(), description=None, parameters=HashMap::default()))]
    pub fn new(
        indicator_variable: &DecisionVariable,
        function: Function,
        equality: Equality,
        name: Option<String>,
        subscripts: Vec<i64>,
        description: Option<String>,
        parameters: HashMap<String, String>,
    ) -> PyResult<Self> {
        let ic =
            ommx::IndicatorConstraint::new(indicator_variable.0.id(), equality.into(), function.0);
        let metadata = ommx::ConstraintMetadata {
            name,
            subscripts,
            parameters: parameters.into_iter().collect(),
            description,
            provenance: Vec::new(),
        };
        Ok(Self(ic, metadata))
    }

    #[getter]
    pub fn indicator_variable_id(&self) -> u64 {
        self.0.indicator_variable.into_inner()
    }

    #[getter]
    pub fn function(&self) -> Function {
        Function(self.0.function().clone())
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
        self.1.parameters.clone().into_iter().collect()
    }

    /// Set the constraint name. Returns a new IndicatorConstraint.
    pub fn set_name(&self, name: String) -> Self {
        let mut ic = self.clone();
        ic.1.name = Some(name);
        ic
    }

    fn __repr__(&self) -> String {
        format!("{}", self.0)
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: pyo3::Bound<pyo3::types::PyAny>) -> Self {
        self.clone()
    }
}

/// A removed indicator constraint together with the reason it was removed.
#[gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct RemovedIndicatorConstraint {
    pub constraint: ommx::IndicatorConstraint,
    pub metadata: ommx::ConstraintMetadata,
    pub removed_reason: ommx::RemovedReason,
}

impl RemovedIndicatorConstraint {
    pub fn from_parts(
        constraint: ommx::IndicatorConstraint,
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

#[gen_stub_pymethods]
#[pymethods]
impl RemovedIndicatorConstraint {
    #[getter]
    pub fn constraint(&self) -> IndicatorConstraint {
        IndicatorConstraint(self.constraint.clone(), self.metadata.clone())
    }

    #[getter]
    pub fn indicator_variable_id(&self) -> u64 {
        self.constraint.indicator_variable.into_inner()
    }

    #[getter]
    pub fn equality(&self) -> Equality {
        self.constraint.equality.into()
    }

    #[getter]
    pub fn function(&self) -> Function {
        Function(self.constraint.function().clone())
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

    fn __repr__(&self) -> String {
        let mut extras: Vec<String> = self
            .removed_reason
            .parameters
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        extras.sort();
        let mut head = format!("{}, reason={}", self.constraint, self.removed_reason.reason);
        if !extras.is_empty() {
            head.push_str(", ");
            head.push_str(&extras.join(", "));
        }
        format!("RemovedIndicatorConstraint({head})")
    }
}

/// Attached indicator constraint — a write-through handle bound to a host
/// ([`crate::Instance`] or [`crate::ParametricInstance`]).
///
/// `AttachedIndicatorConstraint` is returned by
/// `Instance.add_indicator_constraint` /
/// `ParametricInstance.add_indicator_constraint` and by their
/// `indicator_constraints[id]` getters. Reads pull live data from the parent
/// host and metadata setters write through to its SoA metadata store.
#[gen_stub_pyclass]
#[pyclass]
pub struct AttachedIndicatorConstraint {
    pub(crate) host: ConstraintHost,
    pub(crate) id: ommx::IndicatorConstraintID,
}

impl AttachedIndicatorConstraint {
    pub fn new(host: ConstraintHost, id: ommx::IndicatorConstraintID) -> Self {
        Self { host, id }
    }

    pub fn from_instance(instance: Py<crate::Instance>, id: ommx::IndicatorConstraintID) -> Self {
        Self::new(ConstraintHost::Instance(instance), id)
    }

    pub fn from_parametric(
        parametric: Py<crate::ParametricInstance>,
        id: ommx::IndicatorConstraintID,
    ) -> Self {
        Self::new(ConstraintHost::Parametric(parametric), id)
    }
}

fn lookup_indicator<'a>(
    inst: &'a ommx::Instance,
    id: ommx::IndicatorConstraintID,
) -> PyResult<&'a ommx::IndicatorConstraint> {
    inst.indicator_constraints()
        .get(&id)
        .or_else(|| {
            inst.removed_indicator_constraints()
                .get(&id)
                .map(|(c, _)| c)
        })
        .ok_or_else(|| {
            PyKeyError::new_err(format!(
                "indicator constraint id {} not found in instance",
                id.into_inner()
            ))
        })
}

fn lookup_indicator_parametric<'a>(
    inst: &'a ommx::ParametricInstance,
    id: ommx::IndicatorConstraintID,
) -> PyResult<&'a ommx::IndicatorConstraint> {
    inst.indicator_constraints()
        .get(&id)
        .or_else(|| {
            inst.removed_indicator_constraints()
                .get(&id)
                .map(|(c, _)| c)
        })
        .ok_or_else(|| {
            PyKeyError::new_err(format!(
                "indicator constraint id {} not found in parametric instance",
                id.into_inner()
            ))
        })
}

#[gen_stub_pymethods]
#[pymethods]
impl AttachedIndicatorConstraint {
    /// The id this handle points at.
    #[getter]
    pub fn constraint_id(&self) -> u64 {
        self.id.into_inner()
    }

    /// The parent host this constraint lives in.
    #[getter]
    pub fn instance(&self, py: Python<'_>) -> Py<pyo3::PyAny> {
        match &self.host {
            ConstraintHost::Instance(p) => p.clone_ref(py).into_any(),
            ConstraintHost::Parametric(p) => p.clone_ref(py).into_any(),
        }
    }

    /// Return an {class}`~ommx.v1.IndicatorConstraint` snapshot of the
    /// current state. Mutations on the returned object do not propagate back.
    pub fn detach(&self, py: Python<'_>) -> PyResult<IndicatorConstraint> {
        match &self.host {
            ConstraintHost::Instance(p) => {
                let inst = p.borrow(py);
                let c = lookup_indicator(&inst.inner, self.id)?.clone();
                let metadata = inst
                    .inner
                    .indicator_constraint_metadata()
                    .collect_for(self.id);
                Ok(IndicatorConstraint::from_parts(c, metadata))
            }
            ConstraintHost::Parametric(p) => {
                let inst = p.borrow(py);
                let c = lookup_indicator_parametric(&inst.inner, self.id)?.clone();
                let metadata = inst
                    .inner
                    .indicator_constraint_metadata()
                    .collect_for(self.id);
                Ok(IndicatorConstraint::from_parts(c, metadata))
            }
        }
    }

    #[getter]
    pub fn indicator_variable_id(&self, py: Python<'_>) -> PyResult<u64> {
        match &self.host {
            ConstraintHost::Instance(p) => {
                let inst = p.borrow(py);
                Ok(lookup_indicator(&inst.inner, self.id)?
                    .indicator_variable
                    .into_inner())
            }
            ConstraintHost::Parametric(p) => {
                let inst = p.borrow(py);
                Ok(lookup_indicator_parametric(&inst.inner, self.id)?
                    .indicator_variable
                    .into_inner())
            }
        }
    }

    #[getter]
    pub fn function(&self, py: Python<'_>) -> PyResult<Function> {
        match &self.host {
            ConstraintHost::Instance(p) => {
                let inst = p.borrow(py);
                Ok(Function(
                    lookup_indicator(&inst.inner, self.id)?.function().clone(),
                ))
            }
            ConstraintHost::Parametric(p) => {
                let inst = p.borrow(py);
                Ok(Function(
                    lookup_indicator_parametric(&inst.inner, self.id)?
                        .function()
                        .clone(),
                ))
            }
        }
    }

    #[getter]
    pub fn equality(&self, py: Python<'_>) -> PyResult<Equality> {
        match &self.host {
            ConstraintHost::Instance(p) => {
                let inst = p.borrow(py);
                Ok(lookup_indicator(&inst.inner, self.id)?.equality.into())
            }
            ConstraintHost::Parametric(p) => {
                let inst = p.borrow(py);
                Ok(lookup_indicator_parametric(&inst.inner, self.id)?
                    .equality
                    .into())
            }
        }
    }

    pub fn __repr__(&self, py: Python<'_>) -> String {
        match &self.host {
            ConstraintHost::Instance(p) => {
                let inst = p.borrow(py);
                match lookup_indicator(&inst.inner, self.id) {
                    Ok(c) => c.to_string(),
                    Err(_) => format!(
                        "AttachedIndicatorConstraint(id={}, dropped)",
                        self.id.into_inner()
                    ),
                }
            }
            ConstraintHost::Parametric(p) => {
                let inst = p.borrow(py);
                match lookup_indicator_parametric(&inst.inner, self.id) {
                    Ok(c) => c.to_string(),
                    Err(_) => format!(
                        "AttachedIndicatorConstraint(id={}, dropped)",
                        self.id.into_inner()
                    ),
                }
            }
        }
    }

    fn __copy__(&self, py: Python<'_>) -> Self {
        Self {
            host: self.host.clone_ref(py),
            id: self.id,
        }
    }

    fn __deepcopy__(&self, py: Python<'_>, _memo: pyo3::Bound<'_, pyo3::PyAny>) -> Self {
        self.__copy__(py)
    }
}

crate::attached_metadata_methods!(
    AttachedIndicatorConstraint,
    ommx::IndicatorConstraintID,
    indicator_constraint_metadata,
    indicator_constraint_metadata_mut
);
