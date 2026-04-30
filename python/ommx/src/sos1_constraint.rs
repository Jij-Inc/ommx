use pyo3::{exceptions::PyKeyError, prelude::*};
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use std::collections::{BTreeSet, HashMap};

use crate::ConstraintHost;

/// A SOS1 (Special Ordered Set type 1) constraint: at most one variable can be non-zero.
///
/// This is a structural constraint — no explicit function is stored.
/// Unlike OneHotConstraint, SOS1 allows all variables to be zero.
#[gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct Sos1Constraint(pub ommx::Sos1Constraint, pub ommx::ConstraintMetadata);

impl Sos1Constraint {
    pub fn standalone(inner: ommx::Sos1Constraint) -> Self {
        Self(inner, ommx::ConstraintMetadata::default())
    }

    pub fn from_parts(inner: ommx::Sos1Constraint, metadata: ommx::ConstraintMetadata) -> Self {
        Self(inner, metadata)
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl Sos1Constraint {
    /// Create a new SOS1 constraint.
    ///
    /// **Args:**
    ///
    /// - `variables`: List of decision variable IDs (at most one can be non-zero)
    /// - `name` / `subscripts` / `description` / `parameters`: Optional
    ///   metadata. Drained into the host's SoA store on insertion.
    #[new]
    #[pyo3(signature = (*, variables, name=None, subscripts=Vec::new(), description=None, parameters=HashMap::default()))]
    pub fn new(
        variables: Vec<u64>,
        name: Option<String>,
        subscripts: Vec<i64>,
        description: Option<String>,
        parameters: HashMap<String, String>,
    ) -> Self {
        let vars: BTreeSet<ommx::VariableID> =
            variables.into_iter().map(ommx::VariableID::from).collect();
        let metadata = ommx::ConstraintMetadata {
            name,
            subscripts,
            parameters: parameters.into_iter().collect(),
            description,
            provenance: Vec::new(),
        };
        Self(ommx::Sos1Constraint::new(vars), metadata)
    }

    #[getter]
    pub fn variables(&self) -> Vec<u64> {
        self.0.variables.iter().map(|v| v.into_inner()).collect()
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

    /// Set the name. Returns self for method chaining (snapshot mutation).
    pub fn set_name(&mut self, name: String) -> Self {
        self.1.name = Some(name);
        self.clone()
    }

    /// Set the subscripts. Returns self for method chaining (snapshot mutation).
    pub fn set_subscripts(&mut self, subscripts: Vec<i64>) -> Self {
        self.1.subscripts = subscripts;
        self.clone()
    }

    /// Set the description. Returns self for method chaining (snapshot mutation).
    pub fn set_description(&mut self, description: String) -> Self {
        self.1.description = Some(description);
        self.clone()
    }

    /// Replace all parameters. Returns self for method chaining (snapshot mutation).
    pub fn set_parameters(&mut self, parameters: HashMap<String, String>) -> Self {
        self.1.parameters = parameters.into_iter().collect();
        self.clone()
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

/// A removed SOS1 constraint together with the reason it was removed.
#[gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct RemovedSos1Constraint {
    pub constraint: ommx::Sos1Constraint,
    pub metadata: ommx::ConstraintMetadata,
    pub removed_reason: ommx::RemovedReason,
}

impl RemovedSos1Constraint {
    pub fn from_parts(
        constraint: ommx::Sos1Constraint,
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
impl RemovedSos1Constraint {
    #[getter]
    pub fn constraint(&self) -> Sos1Constraint {
        Sos1Constraint(self.constraint.clone(), self.metadata.clone())
    }

    #[getter]
    pub fn variables(&self) -> Vec<u64> {
        self.constraint
            .variables
            .iter()
            .map(|v| v.into_inner())
            .collect()
    }

    #[getter]
    pub fn name(&self) -> Option<String> {
        self.metadata.name.clone()
    }

    #[getter]
    pub fn subscripts(&self) -> Vec<i64> {
        self.metadata.subscripts.clone()
    }

    #[getter]
    pub fn description(&self) -> Option<String> {
        self.metadata.description.clone()
    }

    #[getter]
    pub fn parameters(&self) -> HashMap<String, String> {
        self.metadata
            .parameters
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
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
        format!("RemovedSos1Constraint({head})")
    }
}

/// Attached SOS1 constraint — a write-through handle bound to a host
/// ([`crate::Instance`] or [`crate::ParametricInstance`]).
///
/// Returned by `Instance.add_sos1_constraint` /
/// `ParametricInstance.add_sos1_constraint` and by their
/// `sos1_constraints[id]` getters.
#[gen_stub_pyclass]
#[pyclass]
pub struct AttachedSos1Constraint {
    pub(crate) host: ConstraintHost,
    pub(crate) id: ommx::Sos1ConstraintID,
}

impl AttachedSos1Constraint {
    pub fn new(host: ConstraintHost, id: ommx::Sos1ConstraintID) -> Self {
        Self { host, id }
    }

    pub fn from_instance(instance: Py<crate::Instance>, id: ommx::Sos1ConstraintID) -> Self {
        Self::new(ConstraintHost::Instance(instance), id)
    }

    pub fn from_parametric(
        parametric: Py<crate::ParametricInstance>,
        id: ommx::Sos1ConstraintID,
    ) -> Self {
        Self::new(ConstraintHost::Parametric(parametric), id)
    }
}

fn lookup_sos1<'a>(
    inst: &'a ommx::Instance,
    id: ommx::Sos1ConstraintID,
) -> PyResult<&'a ommx::Sos1Constraint> {
    inst.sos1_constraints()
        .get(&id)
        .or_else(|| inst.removed_sos1_constraints().get(&id).map(|(c, _)| c))
        .ok_or_else(|| {
            PyKeyError::new_err(format!(
                "SOS1 constraint id {} not found in instance",
                id.into_inner()
            ))
        })
}

fn lookup_sos1_parametric<'a>(
    inst: &'a ommx::ParametricInstance,
    id: ommx::Sos1ConstraintID,
) -> PyResult<&'a ommx::Sos1Constraint> {
    inst.sos1_constraints()
        .get(&id)
        .or_else(|| inst.removed_sos1_constraints().get(&id).map(|(c, _)| c))
        .ok_or_else(|| {
            PyKeyError::new_err(format!(
                "SOS1 constraint id {} not found in parametric instance",
                id.into_inner()
            ))
        })
}

#[gen_stub_pymethods]
#[pymethods]
impl AttachedSos1Constraint {
    #[getter]
    pub fn constraint_id(&self) -> u64 {
        self.id.into_inner()
    }

    #[getter]
    pub fn instance(&self, py: Python<'_>) -> Py<pyo3::PyAny> {
        match &self.host {
            ConstraintHost::Instance(p) => p.clone_ref(py).into_any(),
            ConstraintHost::Parametric(p) => p.clone_ref(py).into_any(),
        }
    }

    /// Return a {class}`~ommx.v1.Sos1Constraint` snapshot of the current
    /// state. Mutations on the returned object do not propagate back.
    pub fn detach(&self, py: Python<'_>) -> PyResult<Sos1Constraint> {
        match &self.host {
            ConstraintHost::Instance(p) => {
                let inst = p.borrow(py);
                let c = lookup_sos1(&inst.inner, self.id)?.clone();
                let metadata = inst.inner.sos1_constraint_metadata().collect_for(self.id);
                Ok(Sos1Constraint::from_parts(c, metadata))
            }
            ConstraintHost::Parametric(p) => {
                let inst = p.borrow(py);
                let c = lookup_sos1_parametric(&inst.inner, self.id)?.clone();
                let metadata = inst.inner.sos1_constraint_metadata().collect_for(self.id);
                Ok(Sos1Constraint::from_parts(c, metadata))
            }
        }
    }

    #[getter]
    pub fn variables(&self, py: Python<'_>) -> PyResult<Vec<u64>> {
        match &self.host {
            ConstraintHost::Instance(p) => {
                let inst = p.borrow(py);
                Ok(lookup_sos1(&inst.inner, self.id)?
                    .variables
                    .iter()
                    .map(|v| v.into_inner())
                    .collect())
            }
            ConstraintHost::Parametric(p) => {
                let inst = p.borrow(py);
                Ok(lookup_sos1_parametric(&inst.inner, self.id)?
                    .variables
                    .iter()
                    .map(|v| v.into_inner())
                    .collect())
            }
        }
    }

    pub fn __repr__(&self, py: Python<'_>) -> String {
        match &self.host {
            ConstraintHost::Instance(p) => {
                let inst = p.borrow(py);
                match lookup_sos1(&inst.inner, self.id) {
                    Ok(c) => c.to_string(),
                    Err(_) => format!(
                        "AttachedSos1Constraint(id={}, dropped)",
                        self.id.into_inner()
                    ),
                }
            }
            ConstraintHost::Parametric(p) => {
                let inst = p.borrow(py);
                match lookup_sos1_parametric(&inst.inner, self.id) {
                    Ok(c) => c.to_string(),
                    Err(_) => format!(
                        "AttachedSos1Constraint(id={}, dropped)",
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
    AttachedSos1Constraint,
    ommx::Sos1ConstraintID,
    sos1_constraint_metadata,
    sos1_constraint_metadata_mut
);
