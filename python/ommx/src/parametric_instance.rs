use crate::{
    Constraint, ConstraintHints, DecisionVariable, Function, Instance, NamedFunction, Parameter,
    Parameters, RemovedConstraint, Sense,
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
        let inner = ommx::ParametricInstance::from_bytes(bytes.as_bytes())?;
        Ok(Self {
            inner,
            annotations: HashMap::new(),
        })
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.to_bytes())
    }

    #[staticmethod]
    #[pyo3(signature = (sense, objective, decision_variables, constraints, parameters, named_functions=None, description=None, constraint_hints=None))]
    pub fn from_components(
        sense: Sense,
        objective: Function,
        decision_variables: HashMap<u64, DecisionVariable>,
        constraints: HashMap<u64, Constraint>,
        parameters: HashMap<u64, Parameter>,
        named_functions: Option<HashMap<u64, NamedFunction>>,
        description: Option<crate::InstanceDescription>,
        constraint_hints: Option<ConstraintHints>,
    ) -> Result<Self> {
        let rust_decision_variables: BTreeMap<VariableID, ommx::DecisionVariable> =
            decision_variables
                .into_iter()
                .map(|(id, var)| (VariableID::from(id), var.0))
                .collect();

        let rust_constraints: BTreeMap<ConstraintID, ommx::Constraint> = constraints
            .into_iter()
            .map(|(id, constraint)| (ConstraintID::from(id), constraint.0))
            .collect();

        let rust_parameters: BTreeMap<VariableID, ommx::v1::Parameter> = parameters
            .into_iter()
            .map(|(id, param)| (VariableID::from(id), param.0))
            .collect();

        let mut builder = ommx::ParametricInstance::builder()
            .sense(sense.into())
            .objective(objective.0)
            .decision_variables(rust_decision_variables)
            .constraints(rust_constraints)
            .parameters(rust_parameters);

        if let Some(nfs) = named_functions {
            let rust_named_functions = nfs
                .into_iter()
                .map(|(id, named_function)| (NamedFunctionID::from(id), named_function.0))
                .collect();
            builder = builder.named_functions(rust_named_functions);
        }

        if let Some(hints) = constraint_hints {
            builder = builder.constraint_hints(hints.0);
        }

        if let Some(desc) = description {
            builder = builder.description(desc.0);
        }

        Ok(Self {
            inner: builder.build()?,
            annotations: HashMap::new(),
        })
    }

    pub fn with_parameters(&self, parameters: &Parameters) -> Result<Instance> {
        let instance = self.inner.clone().with_parameters(parameters.0.clone())?;
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
        self.inner
            .decision_variables()
            .values()
            .map(|var| DecisionVariable(var.clone()))
            .collect()
    }

    #[getter]
    pub fn constraints(&self) -> Vec<Constraint> {
        self.inner
            .constraints()
            .values()
            .map(|constraint| Constraint(constraint.clone()))
            .collect()
    }

    #[getter]
    pub fn removed_constraints(&self) -> Vec<RemovedConstraint> {
        self.inner
            .removed_constraints()
            .values()
            .map(|rc| RemovedConstraint(rc.clone()))
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
    pub fn constraint_hints(&self) -> ConstraintHints {
        ConstraintHints(self.inner.constraint_hints().clone())
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
        self.inner
            .decision_variables()
            .get(&VariableID::from(variable_id))
            .map(|var| DecisionVariable(var.clone()))
            .ok_or_else(|| {
                PyKeyError::new_err(format!("Decision variable with ID {variable_id} not found"))
            })
    }

    /// Get a specific constraint by ID
    pub fn get_constraint_by_id(&self, constraint_id: u64) -> PyResult<Constraint> {
        self.inner
            .constraints()
            .get(&ConstraintID::from(constraint_id))
            .map(|c| Constraint(c.clone()))
            .ok_or_else(|| {
                PyKeyError::new_err(format!("Constraint with ID {constraint_id} not found"))
            })
    }

    /// Get a specific removed constraint by ID
    pub fn get_removed_constraint_by_id(&self, constraint_id: u64) -> PyResult<RemovedConstraint> {
        self.inner
            .removed_constraints()
            .get(&ConstraintID::from(constraint_id))
            .map(|rc| RemovedConstraint(rc.clone()))
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

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}
