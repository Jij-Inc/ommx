use crate::{
    pandas::{entries_to_dataframe, PyDataFrame},
    Constraint, DecisionVariable, Function, Instance, NamedFunction, Parameter, RemovedConstraint,
    Sense,
};
use anyhow::Result;
use ommx::{ConstraintID, NamedFunctionID, VariableID};
use pyo3::{exceptions::PyKeyError, prelude::*, types::PyBytes, Bound, PyAny};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct ParametricInstance {
    pub(crate) inner: ommx::ParametricInstance,
    pub(crate) annotations: HashMap<String, String>,
}

crate::annotations::impl_instance_annotations!(
    ParametricInstance,
    "org.ommx.v1.parametric-instance"
);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl ParametricInstance {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let _guard = crate::TRACING.attach_parent_context(bytes.py());
        let inner = ommx::ParametricInstance::from_bytes(bytes.as_bytes())?;
        Ok(Self {
            inner,
            annotations: HashMap::new(),
        })
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        let _guard = crate::TRACING.attach_parent_context(py);
        PyBytes::new(py, &self.inner.to_bytes())
    }

    #[staticmethod]
    #[pyo3(signature = (*, sense, objective, decision_variables, constraints, parameters, named_functions=None, description=None))]
    pub fn from_components(
        sense: Sense,
        objective: Function,
        decision_variables: Vec<DecisionVariable>,
        constraints: BTreeMap<u64, Constraint>,
        parameters: Vec<Parameter>,
        named_functions: Option<Vec<NamedFunction>>,
        description: Option<crate::InstanceDescription>,
    ) -> Result<Self> {
        let mut rust_decision_variables = BTreeMap::new();
        let mut variable_metadata_pairs: Vec<(VariableID, ommx::DecisionVariableMetadata)> =
            Vec::new();
        for var in decision_variables {
            let id = var.0.id();
            variable_metadata_pairs.push((id, var.1.clone()));
            if rust_decision_variables.insert(id, var.0).is_some() {
                anyhow::bail!("Duplicate decision variable ID: {}", id.into_inner());
            }
        }

        let mut constraint_metadata_pairs: Vec<(ConstraintID, ommx::ConstraintMetadata)> =
            Vec::new();
        let rust_constraints: BTreeMap<ConstraintID, ommx::Constraint> = constraints
            .into_iter()
            .map(|(id, c)| {
                let cid = ConstraintID::from(id);
                constraint_metadata_pairs.push((cid, c.1));
                (cid, c.0)
            })
            .collect();

        let mut rust_parameters = BTreeMap::new();
        for p in parameters {
            let id = VariableID::from(p.0.id);
            if rust_parameters.insert(id, p.0).is_some() {
                anyhow::bail!("Duplicate parameter ID: {}", id.into_inner());
            }
        }

        let mut builder = ommx::ParametricInstance::builder()
            .sense(sense.into())
            .objective(objective.0)
            .decision_variables(rust_decision_variables)
            .constraints(rust_constraints)
            .parameters(rust_parameters);

        if let Some(nfs) = named_functions {
            let mut rust_named_functions = BTreeMap::new();
            for nf in nfs {
                let id = nf.0.id;
                if rust_named_functions.insert(id, nf.0).is_some() {
                    anyhow::bail!("Duplicate named function ID: {}", id.into_inner());
                }
            }
            builder = builder.named_functions(rust_named_functions);
        }

        if let Some(desc) = description {
            builder = builder.description(desc.0);
        }

        let mut inner = builder.build()?;
        let var_meta = inner.variable_metadata_mut();
        for (id, m) in variable_metadata_pairs {
            var_meta.insert(id, m);
        }
        let constraint_meta = inner.constraint_metadata_mut();
        for (id, m) in constraint_metadata_pairs {
            constraint_meta.insert(id, m);
        }

        Ok(Self {
            inner,
            annotations: HashMap::new(),
        })
    }

    /// Create trivial empty instance of minimization with zero objective, no constraints, and no decision variables and parameters.
    #[staticmethod]
    pub fn empty() -> Result<Self> {
        Self::from_components(
            Sense::Minimize,
            Function(ommx::Function::Zero),
            Vec::new(),
            BTreeMap::new(),
            Vec::new(),
            None,
            None,
        )
    }

    /// Substitute parameters to yield an instance.
    ///
    /// Parameters can be provided as a dict mapping parameter IDs to their values.
    pub fn with_parameters(&self, parameters: HashMap<u64, f64>) -> Result<Instance> {
        let mut v1_params = ommx::v1::Parameters::default();
        v1_params.entries = parameters;
        let instance = self.inner.clone().with_parameters(v1_params)?;
        Ok(Instance {
            inner: instance,
            annotations: HashMap::new(),
        })
    }

    #[getter]
    pub fn sense(&self) -> Sense {
        (*self.inner.sense()).into()
    }

    #[getter]
    pub fn objective(&self) -> Function {
        Function(self.inner.objective().clone())
    }

    #[getter]
    pub fn decision_variables(&self) -> Vec<DecisionVariable> {
        let metadata = self.inner.variable_metadata();
        self.inner
            .decision_variables()
            .iter()
            .map(|(id, var)| DecisionVariable::from_parts(var.clone(), metadata.collect_for(*id)))
            .collect()
    }

    #[getter]
    pub fn constraints(&self) -> BTreeMap<u64, Constraint> {
        let metadata = self.inner.constraint_collection().metadata();
        self.inner
            .constraints()
            .iter()
            .map(|(id, constraint)| {
                (
                    id.into_inner(),
                    Constraint::from_parts(constraint.clone(), metadata.collect_for(*id)),
                )
            })
            .collect()
    }

    #[getter]
    pub fn removed_constraints(&self) -> BTreeMap<u64, RemovedConstraint> {
        let metadata = self.inner.constraint_collection().metadata();
        self.inner
            .removed_constraints()
            .iter()
            .map(|(id, (c, r))| {
                (
                    id.into_inner(),
                    RemovedConstraint::from_parts(c.clone(), metadata.collect_for(*id), r.clone()),
                )
            })
            .collect()
    }

    #[getter]
    pub fn named_functions(&self) -> Vec<NamedFunction> {
        self.inner
            .named_functions()
            .values()
            .map(|nf| NamedFunction(nf.clone()))
            .collect()
    }

    #[getter]
    pub fn parameters(&self) -> Vec<Parameter> {
        self.inner
            .parameters()
            .values()
            .map(|p| Parameter(p.clone()))
            .collect()
    }

    #[getter]
    pub fn description(&self) -> Option<crate::InstanceDescription> {
        self.inner
            .description
            .as_ref()
            .map(|desc| crate::InstanceDescription(desc.clone()))
    }

    #[getter]
    pub fn decision_variable_ids(&self) -> BTreeSet<u64> {
        self.inner
            .decision_variables()
            .keys()
            .map(|id| id.into_inner())
            .collect()
    }

    #[getter]
    pub fn parameter_ids(&self) -> BTreeSet<u64> {
        self.inner
            .parameters()
            .keys()
            .map(|id| id.into_inner())
            .collect()
    }

    /// Get a specific decision variable by ID
    pub fn get_decision_variable_by_id(&self, variable_id: u64) -> PyResult<DecisionVariable> {
        let var_id = VariableID::from(variable_id);
        let metadata = self.inner.variable_metadata();
        self.inner
            .decision_variables()
            .get(&var_id)
            .map(|var| DecisionVariable::from_parts(var.clone(), metadata.collect_for(var_id)))
            .ok_or_else(|| {
                PyKeyError::new_err(format!("Decision variable with ID {variable_id} not found"))
            })
    }

    /// Get a specific constraint by ID
    pub fn get_constraint_by_id(&self, constraint_id: u64) -> PyResult<Constraint> {
        let cid = ConstraintID::from(constraint_id);
        let metadata = self.inner.constraint_collection().metadata();
        self.inner
            .constraints()
            .get(&cid)
            .map(|c| Constraint::from_parts(c.clone(), metadata.collect_for(cid)))
            .ok_or_else(|| {
                PyKeyError::new_err(format!("Constraint with ID {constraint_id} not found"))
            })
    }

    /// Get a specific removed constraint by ID
    pub fn get_removed_constraint_by_id(&self, constraint_id: u64) -> PyResult<RemovedConstraint> {
        let cid = ConstraintID::from(constraint_id);
        let metadata = self.inner.constraint_collection().metadata();
        self.inner
            .removed_constraints()
            .get(&cid)
            .map(|(c, r)| {
                RemovedConstraint::from_parts(c.clone(), metadata.collect_for(cid), r.clone())
            })
            .ok_or_else(|| {
                PyKeyError::new_err(format!(
                    "Removed constraint with ID {constraint_id} not found"
                ))
            })
    }

    /// Get a specific named function by ID
    pub fn get_named_function_by_id(&self, named_function_id: u64) -> PyResult<NamedFunction> {
        self.inner
            .named_functions()
            .get(&NamedFunctionID::from(named_function_id))
            .map(|nf| NamedFunction(nf.clone()))
            .ok_or_else(|| {
                PyKeyError::new_err(format!(
                    "Named function with ID {named_function_id} not found"
                ))
            })
    }

    /// Get a specific parameter by ID
    pub fn get_parameter_by_id(&self, parameter_id: u64) -> PyResult<Parameter> {
        self.inner
            .parameters()
            .get(&VariableID::from(parameter_id))
            .map(|p| Parameter(p.clone()))
            .ok_or_else(|| {
                PyKeyError::new_err(format!("Parameter with ID {parameter_id} not found"))
            })
    }

    /// DataFrame of decision variables
    #[getter]
    pub fn decision_variables_df<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDataFrame>> {
        let var_meta_store = self.inner.variable_metadata().clone();
        let view: Vec<(ommx::DecisionVariableMetadata, &ommx::DecisionVariable)> = self
            .inner
            .decision_variables()
            .iter()
            .map(|(id, dv)| (var_meta_store.collect_for(*id), dv))
            .collect();
        entries_to_dataframe(
            py,
            view.iter()
                .map(|(m, dv)| crate::pandas::WithMetadata::new(*dv, m)),
            "id",
        )
    }

    /// DataFrame of constraints
    #[getter]
    pub fn constraints_df<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDataFrame>> {
        let meta_store = self.inner.constraint_collection().metadata().clone();
        let view: Vec<(
            ommx::ConstraintMetadata,
            ommx::ConstraintID,
            &ommx::Constraint,
        )> = self
            .inner
            .constraints()
            .iter()
            .map(|(id, c)| (meta_store.collect_for(*id), *id, c))
            .collect();
        entries_to_dataframe(
            py,
            view.iter()
                .map(|(m, id, c)| crate::pandas::WithMetadata::new((*id, *c), m)),
            "id",
        )
    }

    /// DataFrame of removed constraints
    #[getter]
    pub fn removed_constraints_df<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let meta_store = self.inner.constraint_collection().metadata().clone();
        let view: Vec<(
            ommx::ConstraintMetadata,
            ommx::ConstraintID,
            &(ommx::Constraint, ommx::RemovedReason),
        )> = self
            .inner
            .removed_constraints()
            .iter()
            .map(|(id, pair)| (meta_store.collect_for(*id), *id, pair))
            .collect();
        entries_to_dataframe(
            py,
            view.iter()
                .map(|(m, id, pair)| crate::pandas::WithMetadata::new((*id, *pair), m)),
            "id",
        )
    }

    /// DataFrame of named functions
    #[getter]
    pub fn named_functions_df<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDataFrame>> {
        entries_to_dataframe(py, self.inner.named_functions().values(), "id")
    }

    /// DataFrame of parameters
    #[getter]
    pub fn parameters_df<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDataFrame>> {
        entries_to_dataframe(py, self.inner.parameters().values(), "id")
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}
