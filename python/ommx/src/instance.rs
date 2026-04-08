use crate::{
    Constraint, ConstraintHints, DecisionVariable, Function, NamedFunction, ParametricInstance,
    RemovedConstraint, Rng, SampleSet, Samples, Sense, Solution, State, VariableBound,
};
use anyhow::Result;
use ommx::{ConstraintID, Evaluate, NamedFunctionID, Parse, VariableID};
use pyo3::{
    exceptions::PyKeyError,
    prelude::*,
    types::{PyBytes, PyDict},
    Bound, PyAny,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct Instance {
    pub(crate) inner: ommx::Instance,
    pub(crate) annotations: HashMap<String, String>,
}

crate::annotations::impl_instance_annotations!(Instance, "org.ommx.v1.instance");

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Instance {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        Ok(Self {
            inner: ommx::Instance::from_bytes(bytes.as_bytes())?,
            annotations: HashMap::new(),
        })
    }

    #[staticmethod]
    #[pyo3(signature = (*, sense, objective, decision_variables, constraints, named_functions=None, description=None, constraint_hints=None))]
    pub fn from_components(
        sense: Sense,
        objective: Function,
        decision_variables: Vec<DecisionVariable>,
        constraints: Vec<Constraint>,
        named_functions: Option<Vec<NamedFunction>>,
        description: Option<InstanceDescription>,
        constraint_hints: Option<ConstraintHints>,
    ) -> Result<Self> {
        let mut rust_decision_variables = BTreeMap::new();
        for var in decision_variables {
            let id = var.0.id();
            if rust_decision_variables.insert(id, var.0).is_some() {
                anyhow::bail!("Duplicate decision variable ID: {}", id.into_inner());
            }
        }

        let mut rust_constraints = BTreeMap::new();
        for c in constraints {
            let id = c.0.id;
            if rust_constraints.insert(id, c.0).is_some() {
                anyhow::bail!("Duplicate constraint ID: {}", id.into_inner());
            }
        }

        let mut builder = ommx::Instance::builder()
            .sense(sense.into())
            .objective(objective.0)
            .decision_variables(rust_decision_variables)
            .constraints(rust_constraints);

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

    /// Create trivial empty instance of minimization with zero objective, no constraints, and no decision variables.
    #[staticmethod]
    pub fn empty() -> Result<Self> {
        Self::from_components(
            Sense::Minimize,
            Function(ommx::Function::Zero),
            Vec::new(),
            Vec::new(),
            None,
            None,
            None,
        )
    }

    #[classattr]
    #[pyo3(name = "MAXIMIZE")]
    fn class_maximize() -> Sense {
        Sense::Maximize
    }

    #[classattr]
    #[pyo3(name = "MINIMIZE")]
    fn class_minimize() -> Sense {
        Sense::Minimize
    }

    #[classattr]
    #[pyo3(name = "Description")]
    fn class_description(py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(py.get_type::<InstanceDescription>().into_any().unbind())
    }

    #[getter]
    pub fn sense(&self) -> Sense {
        self.inner.sense().into()
    }

    #[getter]
    pub fn objective(&self) -> Function {
        Function(self.inner.objective().clone())
    }

    #[setter]
    pub fn set_objective(&mut self, objective: Function) -> Result<()> {
        self.inner.set_objective(objective.0)?;
        Ok(())
    }

    /// Get all unique decision variable names in this instance
    #[getter]
    pub fn decision_variable_names(&self) -> BTreeSet<String> {
        self.inner.decision_variable_names()
    }

    /// Get all unique named function names in this instance
    #[getter]
    pub fn named_function_names(&self) -> BTreeSet<String> {
        self.inner.named_function_names()
    }

    /// List of all decision variables in the instance sorted by their IDs.
    #[getter]
    pub fn decision_variables(&self) -> Vec<DecisionVariable> {
        self.inner
            .decision_variables()
            .values()
            .map(|var| DecisionVariable(var.clone()))
            .collect()
    }

    /// List of all decision variables in the instance sorted by their IDs.
    #[getter]
    pub fn constraints(&self) -> Vec<Constraint> {
        self.inner
            .constraints()
            .values()
            .map(|constraint| Constraint(constraint.clone()))
            .collect()
    }

    /// List of all removed constraints in the instance sorted by their IDs.
    #[getter]
    pub fn removed_constraints(&self) -> Vec<RemovedConstraint> {
        self.inner
            .removed_constraints()
            .values()
            .map(|removed_constraint| RemovedConstraint(removed_constraint.clone()))
            .collect()
    }

    /// List of all named functions in the instance sorted by their IDs.
    #[getter]
    pub fn named_functions(&self) -> Vec<NamedFunction> {
        self.inner
            .named_functions()
            .values()
            .map(|named_function| NamedFunction(named_function.clone()))
            .collect()
    }

    #[getter]
    pub fn description(&self) -> Option<InstanceDescription> {
        // Convert Option<v1::instance::Description> to Option<InstanceDescription>
        self.inner
            .description
            .as_ref()
            .map(|desc| InstanceDescription(desc.clone()))
    }

    #[getter]
    pub fn constraint_hints(&self) -> ConstraintHints {
        ConstraintHints(self.inner.constraint_hints().clone())
    }

    #[getter]
    pub fn used_decision_variables(&self) -> Vec<DecisionVariable> {
        self.inner
            .used_decision_variables()
            .values()
            .map(|&var| DecisionVariable(var.clone()))
            .collect()
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        let buf = self.inner.to_bytes();
        PyBytes::new(py, &buf)
    }

    pub fn required_ids(&self) -> BTreeSet<u64> {
        self.inner
            .required_ids()
            .into_iter()
            .map(|id| id.into_inner())
            .collect()
    }

    pub fn as_qubo_format<'py>(&self, py: Python<'py>) -> Result<(Bound<'py, PyDict>, f64)> {
        let inner: ommx::v1::Instance = self.inner.clone().into();
        let (qubo, constant) = inner.as_qubo_format()?;
        Ok((
            serde_pyobject::to_pyobject(py, &qubo)?
                .extract()
                .map_err(|e| anyhow::anyhow!("{}", e))?,
            constant,
        ))
    }

    pub fn as_hubo_format<'py>(&self, py: Python<'py>) -> Result<(Bound<'py, PyDict>, f64)> {
        let inner: ommx::v1::Instance = self.inner.clone().into();
        let (hubo, constant) = inner.as_hubo_format()?;
        Ok((
            serde_pyobject::to_pyobject(py, &hubo)?
                .extract()
                .map_err(|e| anyhow::anyhow!("{}", e))?,
            constant,
        ))
    }

    /// Convert the instance to a QUBO format.
    ///
    /// This is a driver API that calls multiple conversion steps in order.
    #[pyo3(signature = (*, uniform_penalty_weight=None, penalty_weights=None, inequality_integer_slack_max_range=31))]
    pub fn to_qubo<'py>(
        &mut self,
        py: Python<'py>,
        uniform_penalty_weight: Option<f64>,
        penalty_weights: Option<HashMap<u64, f64>>,
        inequality_integer_slack_max_range: u64,
    ) -> Result<(Bound<'py, PyDict>, f64)> {
        let is_converted = self.as_minimization_problem();
        self.check_no_continuous_variables("QUBO")?;
        self.qubo_hubo_pipeline(
            uniform_penalty_weight,
            penalty_weights,
            inequality_integer_slack_max_range,
        )?;
        self.log_encode(BTreeSet::new())?;
        let result = self.as_qubo_format(py)?;
        if is_converted {
            self.as_maximization_problem();
        }
        Ok(result)
    }

    /// Convert the instance to a HUBO format.
    ///
    /// This is a driver API that calls multiple conversion steps in order.
    #[pyo3(signature = (*, uniform_penalty_weight=None, penalty_weights=None, inequality_integer_slack_max_range=31))]
    pub fn to_hubo<'py>(
        &mut self,
        py: Python<'py>,
        uniform_penalty_weight: Option<f64>,
        penalty_weights: Option<HashMap<u64, f64>>,
        inequality_integer_slack_max_range: u64,
    ) -> Result<(Bound<'py, PyDict>, f64)> {
        let is_converted = self.as_minimization_problem();
        self.check_no_continuous_variables("HUBO")?;
        self.qubo_hubo_pipeline(
            uniform_penalty_weight,
            penalty_weights,
            inequality_integer_slack_max_range,
        )?;
        self.log_encode(BTreeSet::new())?;
        let result = self.as_hubo_format(py)?;
        if is_converted {
            self.as_maximization_problem();
        }
        Ok(result)
    }

    pub fn as_parametric_instance(&self) -> ParametricInstance {
        ParametricInstance {
            inner: self.inner.clone().into(),
            annotations: HashMap::new(),
        }
    }

    pub fn penalty_method(&self) -> Result<ParametricInstance> {
        let parametric_instance = self.inner.clone().penalty_method()?;
        Ok(ParametricInstance {
            inner: parametric_instance,
            annotations: HashMap::new(),
        })
    }

    pub fn uniform_penalty_method(&self) -> Result<ParametricInstance> {
        let parametric_instance = self.inner.clone().uniform_penalty_method()?;
        Ok(ParametricInstance {
            inner: parametric_instance,
            annotations: HashMap::new(),
        })
    }

    /// Evaluate the instance with the given state.
    ///
    /// Args:
    ///     state: A State object, dict[int, float], or iterable of (int, float) tuples
    ///     atol: Optional absolute tolerance for evaluation
    ///
    /// Returns:
    ///     Solution containing objective value, constraint evaluations, and feasibility
    #[pyo3(signature = (state, *, atol=None))]
    pub fn evaluate(&self, state: State, atol: Option<f64>) -> PyResult<Solution> {
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?,
            None => ommx::ATol::default(),
        };
        let solution = self
            .inner
            .evaluate(&state.0, atol)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(Solution {
            inner: solution,
            annotations: HashMap::new(),
        })
    }

    /// Partially evaluate the instance with the given state.
    ///
    /// Args:
    ///     state: A State object, dict[int, float], or iterable of (int, float) tuples
    ///     atol: Optional absolute tolerance for evaluation
    ///
    /// Returns:
    ///     Self (modified in-place) for method chaining
    #[pyo3(signature = (state, *, atol=None))]
    pub fn partial_evaluate(&self, state: State, atol: Option<f64>) -> PyResult<Self> {
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?,
            None => ommx::ATol::default(),
        };
        let mut new_inner = self.inner.clone();
        new_inner
            .partial_evaluate(&state.0, atol)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(Self {
            inner: new_inner,
            annotations: self.annotations.clone(),
        })
    }

    #[pyo3(signature = (samples, *, atol=None))]
    pub fn evaluate_samples(
        &self,
        samples: Bound<'_, PyAny>,
        atol: Option<f64>,
    ) -> Result<SampleSet> {
        // Accept Samples object or anything Samples.__new__ can handle (dict, list, etc.)
        let samples: Samples = if let Ok(s) = samples.extract::<Samples>() {
            s
        } else {
            Samples::new(samples)?
        };
        let v1_samples: ommx::v1::Samples = samples.0.into();
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        Ok(SampleSet {
            inner: self.inner.evaluate_samples(&v1_samples, atol)?,
            annotations: HashMap::new(),
        })
    }

    pub fn random_state(&self, rng: &Rng) -> Result<crate::State> {
        let strategy = self.inner.arbitrary_state();
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

        let strategy = self.inner.arbitrary_samples(params);
        let mut rng_guard = rng
            .lock()
            .map_err(|_| anyhow::anyhow!("Cannot get lock for RNG"))?;
        let samples = ommx::random::sample(&mut rng_guard, strategy);
        Ok(crate::Samples(samples))
    }

    #[pyo3(signature = (constraint_id, reason, **parameters))]
    pub fn relax_constraint(
        &mut self,
        constraint_id: u64,
        reason: String,
        parameters: Option<HashMap<String, String>>,
    ) -> Result<()> {
        self.inner.relax_constraint(
            constraint_id.into(),
            reason,
            parameters.unwrap_or_default(),
        )?;
        Ok(())
    }

    pub fn restore_constraint(&mut self, constraint_id: u64) -> Result<()> {
        self.inner.restore_constraint(constraint_id.into())?;
        Ok(())
    }

    #[pyo3(signature = (decision_variable_ids=BTreeSet::new()))]
    pub fn log_encode(&mut self, decision_variable_ids: BTreeSet<u64>) -> Result<()> {
        let ids: BTreeSet<u64> = if decision_variable_ids.is_empty() {
            // Auto-detect: find all used integer decision variables
            let analysis = self.inner.analyze_decision_variables();
            let integer_ids: BTreeSet<u64> = analysis
                .used_integer()
                .into_iter()
                .map(|(id, _)| id.into_inner())
                .collect();
            if integer_ids.is_empty() {
                return Ok(());
            }
            integer_ids
        } else {
            decision_variable_ids
        };
        for id in ids.iter() {
            self.inner.log_encode((*id).into())?;
        }
        Ok(())
    }

    pub fn convert_inequality_to_equality_with_integer_slack(
        &mut self,
        constraint_id: u64,
        max_integer_range: u64,
    ) -> Result<()> {
        let mut inner: ommx::v1::Instance = self.inner.clone().into();
        inner.convert_inequality_to_equality_with_integer_slack(
            constraint_id,
            max_integer_range,
            ommx::ATol::default(),
        )?;
        self.inner = Parse::parse(inner, &())?;
        Ok(())
    }

    pub fn add_integer_slack_to_inequality(
        &mut self,
        constraint_id: u64,
        slack_upper_bound: u64,
    ) -> Result<Option<f64>> {
        let mut inner: ommx::v1::Instance = self.inner.clone().into();
        let result = inner.add_integer_slack_to_inequality(constraint_id, slack_upper_bound)?;
        self.inner = Parse::parse(inner, &())?;
        Ok(result)
    }

    pub fn decision_variable_analysis(&self) -> DecisionVariableAnalysis {
        DecisionVariableAnalysis(self.inner.analyze_decision_variables())
    }

    pub fn stats<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyDict>> {
        let stats = self.inner.stats();
        serde_pyobject::to_pyobject(py, &stats)?
            .extract()
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// DataFrame of decision variables
    #[getter]
    pub fn decision_variables_df<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let pandas = py.import("pandas")?;
        let na = pandas.getattr("NA")?;
        let entries: Vec<_> = self
            .inner
            .decision_variables()
            .values()
            .map(|v| DecisionVariable(v.clone()).as_pandas_entry(py, &na))
            .collect::<PyResult<_>>()?;
        let df = pandas.call_method1("DataFrame", (entries,))?;
        if df.getattr("empty")?.extract::<bool>()? {
            return Ok(df);
        }
        df.call_method1("set_index", ("id",))
    }

    /// DataFrame of constraints
    #[getter]
    pub fn constraints_df<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let pandas = py.import("pandas")?;
        let entries: Vec<_> = self
            .inner
            .constraints()
            .values()
            .map(|c| Constraint(c.clone())._as_pandas_entry(py))
            .collect::<PyResult<_>>()?;
        let df = pandas.call_method1("DataFrame", (entries,))?;
        if df.getattr("empty")?.extract::<bool>()? {
            return Ok(df);
        }
        df.call_method1("set_index", ("id",))
    }

    /// DataFrame of removed constraints
    #[getter]
    pub fn removed_constraints_df<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let pandas = py.import("pandas")?;
        let entries: Vec<_> = self
            .inner
            .removed_constraints()
            .values()
            .map(|rc| RemovedConstraint(rc.clone())._as_pandas_entry(py))
            .collect::<PyResult<_>>()?;
        let df = pandas.call_method1("DataFrame", (entries,))?;
        if df.getattr("empty")?.extract::<bool>()? {
            return Ok(df);
        }
        df.call_method1("set_index", ("id",))
    }

    /// DataFrame of named functions
    #[getter]
    pub fn named_functions_df<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let pandas = py.import("pandas")?;
        let entries: Vec<_> = self
            .inner
            .named_functions()
            .values()
            .map(|nf| NamedFunction(nf.clone())._as_pandas_entry(py))
            .collect::<PyResult<_>>()?;
        let df = pandas.call_method1("DataFrame", (entries,))?;
        if df.getattr("empty")?.extract::<bool>()? {
            return Ok(df);
        }
        df.call_method1("set_index", ("id",))
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }

    pub fn as_minimization_problem(&mut self) -> bool {
        self.inner.as_minimization_problem()
    }

    pub fn as_maximization_problem(&mut self) -> bool {
        self.inner.as_maximization_problem()
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
            .map(|constraint| Constraint(constraint.clone()))
            .ok_or_else(|| {
                PyKeyError::new_err(format!("Constraint with ID {constraint_id} not found"))
            })
    }

    /// Get a specific removed constraint by ID
    pub fn get_removed_constraint_by_id(&self, constraint_id: u64) -> PyResult<RemovedConstraint> {
        self.inner
            .removed_constraints()
            .get(&ConstraintID::from(constraint_id))
            .map(|removed_constraint| RemovedConstraint(removed_constraint.clone()))
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
            .map(|named_function| NamedFunction(named_function.clone()))
            .ok_or_else(|| {
                PyKeyError::new_err(format!(
                    "Named function with ID {named_function_id} not found"
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
        self.inner.reduce_binary_power()
    }

    #[staticmethod]
    pub fn load_mps(path: String) -> Result<Self> {
        let instance = ommx::mps::load(path)?;
        Ok(Self {
            inner: instance,
            annotations: HashMap::new(),
        })
    }

    #[pyo3(signature = (path, compress = true))]
    pub fn save_mps(&self, path: String, compress: bool) -> Result<()> {
        ommx::mps::save(&self.inner, path, compress)?;
        Ok(())
    }

    #[staticmethod]
    pub fn load_qplib(path: String) -> Result<Self> {
        let instance = ommx::qplib::load(path)?;
        Ok(Self {
            inner: instance,
            annotations: HashMap::new(),
        })
    }

    /// Generate folded stack format for memory profiling.
    ///
    /// This generates a format compatible with flamegraph visualization tools.
    /// Each line has format: "frame1;frame2;...;frameN bytes"
    ///
    /// Returns:
    ///     str: Folded stack format string that can be visualized with flamegraph tools
    ///
    /// Example:
    ///     >>> instance = Instance(...)
    ///     >>> folded = instance.logical_memory_profile()
    ///     >>> # Save to file and visualize with: flamegraph.pl folded.txt > memory.svg
    ///     >>> with open("folded.txt", "w") as f:
    ///     ...     f.write(folded)
    pub fn logical_memory_profile(&self) -> String {
        ommx::logical_memory::logical_memory_to_folded(&self.inner)
    }
}

impl Instance {
    pub(crate) fn check_no_continuous_variables(&self, format_name: &str) -> Result<()> {
        let continuous_ids: Vec<u64> = self
            .inner
            .analyze_decision_variables()
            .used_continuous()
            .into_iter()
            .map(|(id, _)| id.into_inner())
            .collect();
        if !continuous_ids.is_empty() {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "Continuous variables are not supported in {} conversion: IDs={:?}",
                format_name, continuous_ids
            ))
            .into());
        }
        Ok(())
    }

    /// Shared pipeline for to_qubo/to_hubo: handle inequality constraints, apply penalty method.
    pub(crate) fn qubo_hubo_pipeline(
        &mut self,
        uniform_penalty_weight: Option<f64>,
        penalty_weights: Option<HashMap<u64, f64>>,
        inequality_integer_slack_max_range: u64,
    ) -> Result<()> {
        // Prepare inequality constraints
        let ineq_ids: Vec<ConstraintID> = self
            .inner
            .constraints()
            .iter()
            .filter(|(_, c)| c.equality == ommx::Equality::LessThanOrEqualToZero)
            .map(|(id, _)| *id)
            .collect();
        for ineq_id in ineq_ids {
            let id_u64 = ineq_id.into_inner();
            // Try exact integer slack first, fall back to approximate
            if self
                .convert_inequality_to_equality_with_integer_slack(
                    id_u64,
                    inequality_integer_slack_max_range,
                )
                .is_err()
            {
                self.add_integer_slack_to_inequality(id_u64, inequality_integer_slack_max_range)?;
            }
        }

        // Penalty method
        if !self.inner.constraints().is_empty() {
            if uniform_penalty_weight.is_some() && penalty_weights.is_some() {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "Both uniform_penalty_weight and penalty_weights are specified. Please choose one."
                ).into());
            }
            if let Some(pw) = penalty_weights {
                let pi = self.inner.clone().penalty_method()?;
                // Map constraint IDs (from parameter subscripts) to penalty weights
                let mut weights = HashMap::new();
                for p in pi.parameters().values() {
                    let constraint_id = p.subscripts.first().copied().ok_or_else(|| {
                        anyhow::anyhow!("Penalty parameter {} has no subscripts", p.id)
                    })? as u64;
                    let w = pw.get(&constraint_id).ok_or_else(|| {
                        anyhow::anyhow!(
                            "No penalty weight provided for constraint ID {}",
                            constraint_id
                        )
                    })?;
                    weights.insert(VariableID::from(p.id).into_inner(), *w);
                }
                let mut v1_params = ommx::v1::Parameters::default();
                v1_params.entries = weights;
                self.inner = pi.with_parameters(v1_params)?;
            } else {
                let weight = uniform_penalty_weight.unwrap_or(1.0);
                let pi = self.inner.clone().uniform_penalty_method()?;
                let param_id = pi
                    .parameters()
                    .keys()
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("No penalty weight parameter found"))?;
                let mut v1_params = ommx::v1::Parameters::default();
                v1_params.entries.insert(param_id.into_inner(), weight);
                self.inner = pi.with_parameters(v1_params)?;
            }
        }

        Ok(())
    }
}

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
pub struct DecisionVariableAnalysis(ommx::DecisionVariableAnalysis);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
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

    pub fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let obj = serde_pyobject::to_pyobject(py, &self.0)?;
        Ok(obj.cast::<PyDict>()?.clone())
    }

    pub fn __repr__(&self) -> String {
        self.0.to_string()
    }
}

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct InstanceDescription(pub(crate) ommx::v1::instance::Description);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
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
