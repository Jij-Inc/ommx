use crate::{
    Constraint, ConstraintHints, DecisionVariable, Function, ParametricInstance, RemovedConstraint,
    Rng, SampleSet, Samples, Sense, Solution, VariableBound,
};
use anyhow::Result;
use ommx::{ConstraintID, Evaluate, Message, Parse, VariableID};
use pyo3::{
    exceptions::PyKeyError,
    prelude::*,
    types::{PyBytes, PyDict},
    Bound, PyAny,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[derive(Clone)]
pub struct Instance(pub ommx::Instance);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Instance {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        Ok(Self(ommx::Instance::from_bytes(bytes.as_bytes())?))
    }

    #[staticmethod]
    #[pyo3(signature = (sense, objective, decision_variables, constraints, description = None, constraint_hints = None))]
    pub fn from_components(
        sense: Sense,
        objective: Function,
        decision_variables: HashMap<u64, DecisionVariable>,
        constraints: HashMap<u64, Constraint>,
        description: Option<InstanceDescription>,
        constraint_hints: Option<ConstraintHints>,
    ) -> Result<Self> {
        let rust_sense = sense.into();

        let rust_decision_variables: BTreeMap<VariableID, ommx::DecisionVariable> =
            decision_variables
                .into_iter()
                .map(|(id, var)| (VariableID::from(id), var.0))
                .collect();

        let rust_constraints: BTreeMap<ConstraintID, ommx::Constraint> = constraints
            .into_iter()
            .map(|(id, constraint)| (ConstraintID::from(id), constraint.0))
            .collect();

        let rust_constraint_hints = constraint_hints.map(|hints| hints.0).unwrap_or_default();

        let mut instance = ommx::Instance::new(
            rust_sense,
            objective.0,
            rust_decision_variables,
            rust_constraints,
        )?
        .with_constraint_hints(rust_constraint_hints)?;

        // Set description if provided
        if let Some(desc) = description {
            instance.description = Some(desc.0);
        }

        Ok(Self(instance))
    }

    #[getter]
    pub fn sense(&self) -> Sense {
        self.0.sense().into()
    }

    #[getter]
    pub fn objective(&self) -> Function {
        Function(self.0.objective().clone())
    }

    #[setter]
    pub fn set_objective(&mut self, objective: Function) -> Result<()> {
        self.0.set_objective(objective.0)?;
        Ok(())
    }

    /// List of all decision variables in the instance sorted by their IDs.
    #[getter]
    pub fn decision_variables(&self) -> Vec<DecisionVariable> {
        self.0
            .decision_variables()
            .values()
            .map(|var| DecisionVariable(var.clone()))
            .collect()
    }

    /// List of all decision variables in the instance sorted by their IDs.
    #[getter]
    pub fn constraints(&self) -> Vec<Constraint> {
        self.0
            .constraints()
            .values()
            .map(|constraint| Constraint(constraint.clone()))
            .collect()
    }

    /// List of all removed constraints in the instance sorted by their IDs.
    #[getter]
    pub fn removed_constraints(&self) -> Vec<RemovedConstraint> {
        self.0
            .removed_constraints()
            .values()
            .map(|removed_constraint| RemovedConstraint(removed_constraint.clone()))
            .collect()
    }

    #[getter]
    pub fn description(&self) -> Option<InstanceDescription> {
        // Convert Option<v1::instance::Description> to Option<InstanceDescription>
        self.0
            .description
            .as_ref()
            .map(|desc| InstanceDescription(desc.clone()))
    }

    #[getter]
    pub fn constraint_hints(&self) -> ConstraintHints {
        ConstraintHints(self.0.constraint_hints().clone())
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        let buf = self.0.to_bytes();
        PyBytes::new(py, &buf)
    }

    pub fn required_ids(&self) -> BTreeSet<u64> {
        self.0
            .required_ids()
            .into_iter()
            .map(|id| id.into_inner())
            .collect()
    }

    pub fn as_qubo_format<'py>(&self, py: Python<'py>) -> Result<(Bound<'py, PyDict>, f64)> {
        let inner: ommx::v1::Instance = self.0.clone().into();
        let (qubo, constant) = inner.as_qubo_format()?;
        Ok((serde_pyobject::to_pyobject(py, &qubo)?.extract()?, constant))
    }

    pub fn as_hubo_format<'py>(&self, py: Python<'py>) -> Result<(Bound<'py, PyDict>, f64)> {
        let inner: ommx::v1::Instance = self.0.clone().into();
        let (hubo, constant) = inner.as_hubo_format()?;
        Ok((serde_pyobject::to_pyobject(py, &hubo)?.extract()?, constant))
    }

    pub fn as_parametric_instance(&self) -> ParametricInstance {
        ParametricInstance(self.0.clone().into())
    }

    pub fn penalty_method(&self) -> Result<ParametricInstance> {
        let parametric_instance = self.0.clone().penalty_method()?;
        Ok(ParametricInstance(parametric_instance))
    }

    pub fn uniform_penalty_method(&self) -> Result<ParametricInstance> {
        let parametric_instance = self.0.clone().uniform_penalty_method()?;
        Ok(ParametricInstance(parametric_instance))
    }

    pub fn evaluate(&self, state: &Bound<PyBytes>) -> Result<Solution> {
        let state = ommx::v1::State::decode(state.as_bytes())?;
        let solution = self.0.evaluate(&state, ommx::ATol::default())?;
        Ok(Solution(solution))
    }

    pub fn partial_evaluate<'py>(
        &mut self,
        py: Python<'py>,
        state: &Bound<PyBytes>,
    ) -> Result<Bound<'py, PyBytes>> {
        let state = ommx::v1::State::decode(state.as_bytes())?;
        self.0.partial_evaluate(&state, ommx::ATol::default())?;
        let inner: ommx::v1::Instance = self.0.clone().into();
        Ok(PyBytes::new(py, &inner.encode_to_vec()))
    }

    pub fn evaluate_samples(&self, samples: &Samples) -> Result<SampleSet> {
        let v1_samples: ommx::v1::Samples = samples.0.clone().into();
        Ok(SampleSet(
            self.0
                .evaluate_samples(&v1_samples, ommx::ATol::default())?,
        ))
    }

    pub fn random_state(&self, rng: &Rng) -> Result<crate::State> {
        let strategy = self.0.arbitrary_state();
        let mut rng_guard = rng
            .lock()
            .map_err(|_| anyhow::anyhow!("Cannot get lock for RNG"))?;
        let state = ommx::random::sample(&mut rng_guard, strategy);
        Ok(crate::State(state))
    }

    #[pyo3(signature = (
        rng,
        *,
        num_different_samples = *ommx::random::SamplesParameters::default().num_different_samples(),
        num_samples = *ommx::random::SamplesParameters::default().num_samples(),
        max_sample_id = None
    ))]
    pub fn random_samples(
        &self,
        rng: &Rng,
        num_different_samples: usize,
        num_samples: usize,
        max_sample_id: Option<u64>,
    ) -> Result<crate::Samples> {
        let max_sample_id = max_sample_id.unwrap_or(num_samples as u64);
        let params = ommx::random::SamplesParameters::new(
            num_different_samples,
            num_samples,
            max_sample_id,
        )?;

        let strategy = self.0.arbitrary_samples(params);
        let mut rng_guard = rng
            .lock()
            .map_err(|_| anyhow::anyhow!("Cannot get lock for RNG"))?;
        let samples = ommx::random::sample(&mut rng_guard, strategy);
        Ok(crate::Samples(samples))
    }

    pub fn relax_constraint(
        &mut self,
        constraint_id: u64,
        removed_reason: String,
        removed_reason_parameters: HashMap<String, String>,
    ) -> Result<()> {
        self.0.relax_constraint(
            constraint_id.into(),
            removed_reason,
            removed_reason_parameters,
        )?;
        Ok(())
    }

    pub fn restore_constraint(&mut self, constraint_id: u64) -> Result<()> {
        self.0.restore_constraint(constraint_id.into())?;
        Ok(())
    }

    pub fn log_encode(&mut self, integer_variable_ids: BTreeSet<u64>) -> Result<()> {
        for id in integer_variable_ids.iter() {
            self.0.log_encode((*id).into())?;
        }
        Ok(())
    }

    pub fn convert_inequality_to_equality_with_integer_slack(
        &mut self,
        constraint_id: u64,
        max_integer_range: u64,
    ) -> Result<()> {
        let mut inner: ommx::v1::Instance = self.0.clone().into();
        inner.convert_inequality_to_equality_with_integer_slack(
            constraint_id,
            max_integer_range,
            ommx::ATol::default(),
        )?;
        self.0 = Parse::parse(inner, &())?;
        Ok(())
    }

    pub fn add_integer_slack_to_inequality(
        &mut self,
        constraint_id: u64,
        slack_upper_bound: u64,
    ) -> Result<Option<f64>> {
        let mut inner: ommx::v1::Instance = self.0.clone().into();
        let result = inner.add_integer_slack_to_inequality(constraint_id, slack_upper_bound)?;
        self.0 = Parse::parse(inner, &())?;
        Ok(result)
    }

    pub fn decision_variable_analysis(&self) -> DecisionVariableAnalysis {
        DecisionVariableAnalysis(self.0.analyze_decision_variables())
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

    pub fn as_minimization_problem(&mut self) -> bool {
        self.0.as_minimization_problem()
    }

    pub fn as_maximization_problem(&mut self) -> bool {
        self.0.as_maximization_problem()
    }

    /// Get a specific decision variable by ID
    pub fn get_decision_variable_by_id(&self, variable_id: u64) -> PyResult<DecisionVariable> {
        self.0
            .decision_variables()
            .get(&VariableID::from(variable_id))
            .map(|var| DecisionVariable(var.clone()))
            .ok_or_else(|| {
                PyKeyError::new_err(format!("Decision variable with ID {variable_id} not found"))
            })
    }

    /// Get a specific constraint by ID
    pub fn get_constraint_by_id(&self, constraint_id: u64) -> PyResult<Constraint> {
        self.0
            .constraints()
            .get(&ConstraintID::from(constraint_id))
            .map(|constraint| Constraint(constraint.clone()))
            .ok_or_else(|| {
                PyKeyError::new_err(format!("Constraint with ID {constraint_id} not found"))
            })
    }

    /// Get a specific removed constraint by ID
    pub fn get_removed_constraint_by_id(&self, constraint_id: u64) -> PyResult<RemovedConstraint> {
        self.0
            .removed_constraints()
            .get(&ConstraintID::from(constraint_id))
            .map(|removed_constraint| RemovedConstraint(removed_constraint.clone()))
            .ok_or_else(|| {
                PyKeyError::new_err(format!(
                    "Removed constraint with ID {constraint_id} not found"
                ))
            })
    }

    /// Reduce binary powers in the instance.
    ///
    /// This method replaces binary powers in the instance with their equivalent linear expressions.
    /// For binary variables, x^n = x for any n >= 1, so we can reduce higher powers to linear terms.
    ///
    /// Returns `True` if any reduction was performed, `False` otherwise.
    pub fn reduce_binary_power(&mut self) -> bool {
        self.0.reduce_binary_power()
    }

    #[staticmethod]
    pub fn load_mps(path: String) -> Result<Self> {
        let instance = ommx::mps::load(path)?;
        Ok(Self(instance))
    }

    #[pyo3(signature = (path, compress = true))]
    pub fn save_mps(&self, path: String, compress: bool) -> Result<()> {
        ommx::mps::save(&self.0, path, compress)?;
        Ok(())
    }

    #[staticmethod]
    pub fn load_qplib(path: String) -> Result<Self> {
        let instance = ommx::qplib::load(path)?;
        Ok(Self(instance))
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct DecisionVariableAnalysis(ommx::DecisionVariableAnalysis);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl DecisionVariableAnalysis {
    pub fn used_binary(&self) -> BTreeMap<u64, VariableBound> {
        self.0
            .used_binary()
            .into_iter()
            .map(|(id, bound)| (id.into_inner(), VariableBound(bound)))
            .collect()
    }

    pub fn used_integer(&self) -> BTreeMap<u64, VariableBound> {
        self.0
            .used_integer()
            .into_iter()
            .map(|(id, bound)| (id.into_inner(), VariableBound(bound)))
            .collect()
    }

    pub fn used_continuous(&self) -> BTreeMap<u64, VariableBound> {
        self.0
            .used_continuous()
            .into_iter()
            .map(|(id, bound)| (id.into_inner(), VariableBound(bound)))
            .collect()
    }

    pub fn used_semi_integer(&self) -> BTreeMap<u64, VariableBound> {
        self.0
            .used_semi_integer()
            .into_iter()
            .map(|(id, bound)| (id.into_inner(), VariableBound(bound)))
            .collect()
    }

    pub fn used_semi_continuous(&self) -> BTreeMap<u64, VariableBound> {
        self.0
            .used_semi_continuous()
            .into_iter()
            .map(|(id, bound)| (id.into_inner(), VariableBound(bound)))
            .collect()
    }

    pub fn used_decision_variable_ids(&self) -> BTreeSet<u64> {
        self.0.used().iter().map(|id| id.into_inner()).collect()
    }

    pub fn all_decision_variable_ids(&self) -> BTreeSet<u64> {
        self.0.all().iter().map(|id| id.into_inner()).collect()
    }

    pub fn used_in_objective(&self) -> BTreeSet<u64> {
        self.0
            .used_in_objective()
            .iter()
            .map(|id| id.into_inner())
            .collect()
    }

    pub fn used_in_constraints(&self) -> BTreeMap<u64, BTreeSet<u64>> {
        self.0
            .used_in_constraints()
            .iter()
            .map(|(constraint_id, variable_ids)| {
                (
                    **constraint_id,
                    variable_ids.iter().map(|id| id.into_inner()).collect(),
                )
            })
            .collect()
    }

    pub fn fixed(&self) -> BTreeMap<u64, f64> {
        self.0
            .fixed()
            .iter()
            .map(|(id, value)| (id.into_inner(), *value))
            .collect()
    }

    pub fn irrelevant(&self) -> BTreeSet<u64> {
        self.0
            .irrelevant()
            .keys()
            .map(|id| id.into_inner())
            .collect()
    }

    pub fn dependent(&self) -> BTreeSet<u64> {
        self.0
            .dependent()
            .keys()
            .map(|id| id.into_inner())
            .collect()
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[derive(Clone)]
pub struct InstanceDescription(ommx::v1::instance::Description);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl InstanceDescription {
    #[new]
    #[pyo3(signature = (name = None, description = None, authors = None, created_by = None))]
    pub fn new(
        name: Option<String>,
        description: Option<String>,
        authors: Option<Vec<String>>,
        created_by: Option<String>,
    ) -> Self {
        let mut desc = ommx::v1::instance::Description::default();
        desc.name = name;
        desc.description = description;
        desc.authors = authors.unwrap_or_default();
        desc.created_by = created_by;
        Self(desc)
    }
    #[getter]
    pub fn name(&self) -> Option<String> {
        self.0.name.clone()
    }

    #[getter]
    pub fn description(&self) -> Option<String> {
        self.0.description.clone()
    }

    #[getter]
    pub fn authors(&self) -> Vec<String> {
        self.0.authors.clone()
    }

    #[getter]
    pub fn created_by(&self) -> Option<String> {
        self.0.created_by.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "InstanceDescription(name={:?}, description={:?}, authors={:?}, created_by={:?})",
            self.0.name, self.0.description, self.0.authors, self.0.created_by
        )
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}
