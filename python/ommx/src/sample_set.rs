use crate::{
    pandas::{entries_to_dataframe, sorted_entries_to_dataframe, PyDataFrame, WithSampleIds},
    Solution,
};
use anyhow::Result;
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict, PyTuple},
    Bound, PyResult, Python,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// The output of sampling-based optimization algorithms, e.g. simulated annealing (SA).
///
/// - Similar to `Solution` rather than the raw `State` message.
///   This class contains the sampled values of decision variables with the objective value, constraint violations,
///   feasibility, and metadata of constraints and decision variables.
/// - This class is usually created via `Instance.evaluate_samples`.
///
/// # Examples
///
/// Let's consider a simple optimization problem:
///
/// maximize x_1 + 2 x_2 + 3 x_3
/// subject to x_1 + x_2 + x_3 = 1
/// x_1, x_2, x_3 in {0, 1}
///
/// ```python
/// >>> x = [DecisionVariable.binary(i) for i in range(3)]
/// >>> instance = Instance.from_components(
/// ...     decision_variables=x,
/// ...     objective=x[0] + 2*x[1] + 3*x[2],
/// ...     constraints=[sum(x) == 1],
/// ...     sense=Instance.MAXIMIZE,
/// ... )
/// ```
///
/// with three samples:
///
/// ```python
/// >>> samples = {
/// ...     0: {0: 1, 1: 0, 2: 0},  # x1 = 1, x2 = x3 = 0
/// ...     1: {0: 0, 1: 0, 2: 1},  # x3 = 1, x1 = x2 = 0
/// ...     2: {0: 1, 1: 1, 2: 0},  # x1 = x2 = 1, x3 = 0 (infeasible)
/// ... } # ^ sample ID
/// ```
///
/// Note that this will be done by sampling-based solvers, but we do it manually here.
/// We can evaluate the samples via `Instance.evaluate_samples`:
///
/// ```python
/// >>> sample_set = instance.evaluate_samples(samples)
/// >>> sample_set.summary  # doctest: +NORMALIZE_WHITESPACE
///            objective  feasible
/// sample_id
/// 1                3.0      True
/// 0                1.0      True
/// 2                3.0     False
/// ```
///
/// The `summary` attribute shows the objective value, feasibility of each sample.
/// Note that this `feasible` column represents the feasibility of the original constraints, not the relaxed constraints.
/// You can get each sample by `get` as a `Solution` format:
///
/// ```python
/// >>> solution = sample_set.get(sample_id=0)
/// >>> solution.objective
/// 1.0
/// ```
///
/// `best_feasible` returns the best feasible sample, i.e. the largest objective value among feasible samples:
///
/// ```python
/// >>> solution = sample_set.best_feasible
/// >>> solution.objective
/// 3.0
/// ```
///
/// Of course, the sample of smallest objective value is returned for minimization problems.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct SampleSet {
    pub(crate) inner: ommx::SampleSet,
    pub(crate) annotations: HashMap<String, String>,
}

crate::annotations::impl_solution_annotations!(SampleSet, "org.ommx.v1.sample-set");

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl SampleSet {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        Ok(Self {
            inner: ommx::SampleSet::from_bytes(bytes.as_bytes())?,
            annotations: HashMap::new(),
        })
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.to_bytes())
    }

    pub fn get(&self, sample_id: u64) -> Result<Solution> {
        let sample_id = ommx::SampleID::from(sample_id);
        let solution = self.inner.get(sample_id)?;
        Ok(Solution {
            inner: solution,
            annotations: HashMap::new(),
        })
    }

    /// Get sample by ID (alias for get method)
    pub fn get_sample_by_id(&self, sample_id: u64) -> Result<Solution> {
        self.get(sample_id)
    }

    pub fn num_samples(&self) -> usize {
        self.inner.sample_ids().len()
    }

    pub fn sample_ids(&self) -> BTreeSet<u64> {
        self.inner
            .sample_ids()
            .iter()
            .map(|id| id.into_inner())
            .collect()
    }

    pub fn feasible_ids(&self) -> BTreeSet<u64> {
        self.inner
            .feasible_ids()
            .iter()
            .map(|id| id.into_inner())
            .collect()
    }

    pub fn feasible_relaxed_ids(&self) -> BTreeSet<u64> {
        self.inner
            .feasible_relaxed_ids()
            .iter()
            .map(|id| id.into_inner())
            .collect()
    }

    pub fn feasible_unrelaxed_ids(&self) -> BTreeSet<u64> {
        // For now, this is the same as feasible_ids since ommx::SampleSet doesn't distinguish
        self.feasible_ids()
    }

    #[getter]
    pub fn best_feasible_id(&self) -> Result<u64> {
        Ok(self.inner.best_feasible_id()?.into_inner())
    }

    #[getter]
    pub fn best_feasible_relaxed_id(&self) -> Result<u64> {
        Ok(self.inner.best_feasible_relaxed_id()?.into_inner())
    }

    #[getter]
    pub fn best_feasible(&self) -> Result<Solution> {
        Ok(Solution {
            inner: self.inner.best_feasible()?,
            annotations: HashMap::new(),
        })
    }

    #[getter]
    pub fn best_feasible_relaxed(&self) -> Result<Solution> {
        Ok(Solution {
            inner: self.inner.best_feasible_relaxed()?,
            annotations: HashMap::new(),
        })
    }

    #[getter]
    pub fn best_feasible_unrelaxed(&self) -> Result<Solution> {
        // Exactly the same as best_feasible
        self.best_feasible()
    }

    /// Get objectives for all samples
    #[getter]
    pub fn objectives(&self) -> BTreeMap<u64, f64> {
        self.inner
            .objectives()
            .iter()
            .map(|(sample_id, objective)| (sample_id.into_inner(), *objective))
            .collect()
    }

    /// Get feasibility status for all samples
    #[getter]
    pub fn feasible(&self) -> BTreeMap<u64, bool> {
        self.inner
            .feasible()
            .iter()
            .map(|(sample_id, &is_feasible)| (sample_id.into_inner(), is_feasible))
            .collect()
    }

    /// Get relaxed feasibility status for all samples
    #[getter]
    pub fn feasible_relaxed(&self) -> BTreeMap<u64, bool> {
        self.inner
            .feasible_relaxed()
            .iter()
            .map(|(sample_id, &is_feasible)| (sample_id.into_inner(), is_feasible))
            .collect()
    }

    /// Get unrelaxed feasibility status for all samples
    #[getter]
    pub fn feasible_unrelaxed(&self) -> BTreeMap<u64, bool> {
        self.feasible()
    }

    /// Get the optimization sense (minimize or maximize)
    #[getter]
    pub fn sense(&self) -> crate::Sense {
        match self.inner.sense() {
            ommx::Sense::Minimize => crate::Sense::Minimize,
            ommx::Sense::Maximize => crate::Sense::Maximize,
        }
    }

    /// Get named functions for compatibility with existing Python code
    #[getter]
    pub fn named_functions(&self) -> Vec<crate::SampledNamedFunction> {
        self.inner
            .named_functions()
            .values()
            .map(|nf| crate::SampledNamedFunction(nf.clone()))
            .collect()
    }

    /// Get constraints for compatibility with existing Python code
    #[getter]
    pub fn constraints(&self) -> Vec<crate::SampledConstraint> {
        self.inner
            .constraints()
            .values()
            .map(|constraint| crate::SampledConstraint(constraint.clone()))
            .collect()
    }

    /// Get decision variables for compatibility with existing Python code
    #[getter]
    pub fn decision_variables(&self) -> Vec<crate::SampledDecisionVariable> {
        self.inner
            .decision_variables()
            .values()
            .map(|variable| crate::SampledDecisionVariable(variable.clone()))
            .collect()
    }

    /// Get sample IDs as a list (property version)
    #[getter]
    pub fn sample_ids_list(&self) -> Vec<u64> {
        self.inner
            .sample_ids()
            .iter()
            .map(|&sample_id| sample_id.into_inner())
            .collect()
    }

    /// Get all unique decision variable names in this sample set.
    ///
    /// Returns a set of all unique variable names. Variables without names are not included.
    ///
    /// # Examples
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
    /// >>> y = [DecisionVariable.binary(i+3, name="y", subscripts=[i]) for i in range(2)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x + y,
    /// ...     objective=sum(x) + sum(y),
    /// ...     constraints=[],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// >>> sample_set = instance.evaluate_samples({0: {i: 1 for i in range(5)}})
    /// >>> sorted(sample_set.decision_variable_names)
    /// ['x', 'y']
    /// ```
    #[getter]
    pub fn decision_variable_names(&self) -> BTreeSet<String> {
        self.inner.decision_variable_names()
    }

    /// Get all unique named function names in this sample set
    #[getter]
    pub fn named_function_names(&self) -> BTreeSet<String> {
        self.inner.named_function_names()
    }

    /// Extract decision variable values for a given name and sample ID
    pub fn extract_decision_variables<'py>(
        &self,
        py: Python<'py>,
        name: &str,
        sample_id: u64,
    ) -> Result<Bound<'py, PyDict>> {
        let sample_id = ommx::SampleID::from(sample_id);
        let extracted = self.inner.extract_decision_variables(name, sample_id)?;
        let dict = PyDict::new(py);
        for (subscripts, value) in extracted {
            // Convert Vec<i64> to tuple for use as dict key
            let key = PyTuple::new(py, &subscripts)?;
            dict.set_item(key, value)?;
        }
        Ok(dict)
    }

    /// Extract all decision variables grouped by name for a given sample ID.
    ///
    /// Returns a mapping from variable name to a mapping from subscripts to values.
    /// This is useful for extracting all variables at once in a structured format.
    /// Variables without names are not included in the result.
    ///
    /// Raises ValueError if a decision variable with parameters is found, or if the same
    /// name and subscript combination is found multiple times, or if the sample ID is invalid.
    ///
    /// # Examples
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
    /// >>> y = [DecisionVariable.binary(i+3, name="y", subscripts=[i]) for i in range(2)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x + y,
    /// ...     objective=sum(x) + sum(y),
    /// ...     constraints=[],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// >>> sample_set = instance.evaluate_samples({0: {i: 1 for i in range(5)}})
    /// >>> all_vars = sample_set.extract_all_decision_variables(0)
    /// >>> all_vars["x"]
    /// {(0,): 1.0, (1,): 1.0, (2,): 1.0}
    /// >>> all_vars["y"]
    /// {(0,): 1.0, (1,): 1.0}
    /// ```
    pub fn extract_all_decision_variables<'py>(
        &self,
        py: Python<'py>,
        sample_id: u64,
    ) -> Result<Bound<'py, PyDict>> {
        let sample_id = ommx::SampleID::from(sample_id);
        let result_dict = PyDict::new(py);
        for (name, variables) in self.inner.extract_all_decision_variables(sample_id)? {
            let var_dict = PyDict::new(py);
            for (subscripts, value) in variables {
                let key_tuple = PyTuple::new(py, &subscripts)?;
                var_dict.set_item(key_tuple, value)?;
            }
            result_dict.set_item(name, var_dict)?;
        }
        Ok(result_dict)
    }

    /// Extract constraint values for a given name and sample ID
    pub fn extract_constraints<'py>(
        &self,
        py: Python<'py>,
        name: &str,
        sample_id: u64,
    ) -> Result<Bound<'py, PyDict>> {
        let sample_id = ommx::SampleID::from(sample_id);
        let extracted = self.inner.extract_constraints(name, sample_id)?;
        let dict = PyDict::new(py);
        for (subscripts, value) in extracted {
            let key = PyTuple::new(py, &subscripts)?;
            dict.set_item(key, value)?;
        }
        Ok(dict)
    }

    /// Extract named function values for a given name and sample ID
    pub fn extract_named_functions<'py>(
        &self,
        py: Python<'py>,
        name: &str,
        sample_id: u64,
    ) -> Result<Bound<'py, PyDict>> {
        let sample_id = ommx::SampleID::from(sample_id);
        let extracted = self.inner.extract_named_functions(name, sample_id)?;
        let dict = PyDict::new(py);
        for (subscripts, value) in extracted {
            let key = PyTuple::new(py, &subscripts)?;
            dict.set_item(key, value)?;
        }
        Ok(dict)
    }

    /// Extract all named function values grouped by name for a given sample ID
    pub fn extract_all_named_functions<'py>(
        &self,
        py: Python<'py>,
        sample_id: u64,
    ) -> Result<Bound<'py, PyDict>> {
        let sample_id = ommx::SampleID::from(sample_id);
        let result_dict = PyDict::new(py);
        for (name, functions) in self.inner.extract_all_named_functions(sample_id)? {
            let func_dict = PyDict::new(py);
            for (subscripts, value) in functions {
                let key_tuple = PyTuple::new(py, &subscripts)?;
                func_dict.set_item(key_tuple, value)?;
            }
            result_dict.set_item(name, func_dict)?;
        }
        Ok(result_dict)
    }

    /// Get a specific sampled decision variable by ID
    pub fn get_decision_variable_by_id(
        &self,
        variable_id: u64,
    ) -> PyResult<crate::SampledDecisionVariable> {
        let var_id = ommx::VariableID::from(variable_id);
        self.inner
            .decision_variables()
            .get(&var_id)
            .map(|dv| crate::SampledDecisionVariable(dv.clone()))
            .ok_or_else(|| {
                pyo3::exceptions::PyKeyError::new_err(format!(
                    "Unknown decision variable ID: {variable_id}"
                ))
            })
    }

    /// Get a specific sampled constraint by ID  
    pub fn get_constraint_by_id(&self, constraint_id: u64) -> PyResult<crate::SampledConstraint> {
        let constraint_id = ommx::ConstraintID::from(constraint_id);
        self.inner
            .constraints()
            .get(&constraint_id)
            .map(|sc| crate::SampledConstraint(sc.clone()))
            .ok_or_else(|| {
                pyo3::exceptions::PyKeyError::new_err(format!(
                    "Unknown constraint ID: {constraint_id}"
                ))
            })
    }

    /// Get a specific sampled named function by ID
    pub fn get_named_function_by_id(
        &self,
        named_function_id: u64,
    ) -> PyResult<crate::SampledNamedFunction> {
        let named_function_id = ommx::NamedFunctionID::from(named_function_id);
        self.inner
            .named_functions()
            .get(&named_function_id)
            .map(|nf| crate::SampledNamedFunction(nf.clone()))
            .ok_or_else(|| {
                pyo3::exceptions::PyKeyError::new_err(format!(
                    "Unknown named function ID: {named_function_id}"
                ))
            })
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }

    /// Summary DataFrame with columns: objective, feasible. Sorted by feasible desc then objective.
    /// Index is sample_id.
    #[getter]
    pub fn summary<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDataFrame>> {
        let feasible_map = self.inner.feasible();
        let ascending = matches!(self.inner.sense(), ommx::Sense::Minimize);
        let entries: Vec<_> = self
            .inner
            .objectives()
            .iter()
            .map(|(sample_id, &objective)| {
                let dict = PyDict::new(py);
                dict.set_item("sample_id", sample_id.into_inner())?;
                dict.set_item("objective", objective)?;
                dict.set_item(
                    "feasible",
                    feasible_map.get(sample_id).copied().unwrap_or(false),
                )?;
                Ok(dict.into_any())
            })
            .collect::<PyResult<_>>()?;
        sorted_entries_to_dataframe(
            py,
            entries,
            &["feasible", "objective"],
            &[false, ascending],
            "sample_id",
        )
    }

    /// Summary DataFrame with per-constraint feasibility columns.
    /// Index is sample_id.
    #[getter]
    pub fn summary_with_constraints<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let feasible_map = self.inner.feasible();
        let constraints = self.inner.constraints();
        let ascending = matches!(self.inner.sense(), ommx::Sense::Minimize);

        // Build constraint labels
        let constraint_labels: Vec<(ommx::ConstraintID, String)> = constraints
            .iter()
            .map(|(&id, sc)| {
                let label = match sc.metadata.name.as_deref().filter(|n| !n.is_empty()) {
                    Some(name) => {
                        if sc.metadata.subscripts.is_empty() {
                            name.to_string()
                        } else {
                            let subs: Vec<String> = sc
                                .metadata
                                .subscripts
                                .iter()
                                .map(|s| s.to_string())
                                .collect();
                            format!("{}[{}]", name, subs.join(", "))
                        }
                    }
                    None => id.into_inner().to_string(),
                };
                (id, label)
            })
            .collect();

        let entries: Vec<_> = self
            .inner
            .objectives()
            .iter()
            .map(|(sample_id, &objective)| {
                let dict = PyDict::new(py);
                dict.set_item("sample_id", sample_id.into_inner())?;
                dict.set_item("objective", objective)?;
                dict.set_item(
                    "feasible",
                    feasible_map.get(sample_id).copied().unwrap_or(false),
                )?;
                for (constraint_id, label) in &constraint_labels {
                    let c_feasible = constraints[constraint_id]
                        .stage
                        .feasible
                        .get(sample_id)
                        .copied()
                        .unwrap_or(false);
                    dict.set_item(label.as_str(), c_feasible)?;
                }
                Ok(dict.into_any())
            })
            .collect::<PyResult<_>>()?;
        sorted_entries_to_dataframe(
            py,
            entries,
            &["feasible", "objective"],
            &[false, ascending],
            "sample_id",
        )
    }

    /// DataFrame of decision variables with per-sample value columns.
    /// Static columns: id, kind, lower, upper, name, subscripts, description.
    /// Dynamic columns: one per sample_id (int) with the variable's value.
    #[getter]
    pub fn decision_variables_df<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDataFrame>> {
        let sample_ids = sorted_sample_ids(&self.inner);
        entries_to_dataframe(
            py,
            self.inner
                .decision_variables()
                .values()
                .map(|item| WithSampleIds {
                    item,
                    sample_ids: &sample_ids,
                }),
            "id",
        )
    }

    /// DataFrame of constraints with per-sample value and feasibility columns.
    /// Static columns: id, equality, used_ids, name, subscripts, description.
    /// Dynamic columns: value.{sample_id} and feasible.{sample_id} for each sample.
    #[getter]
    pub fn constraints_df<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDataFrame>> {
        let sample_ids = sorted_sample_ids(&self.inner);
        entries_to_dataframe(
            py,
            self.inner.constraints().values().map(|item| WithSampleIds {
                item,
                sample_ids: &sample_ids,
            }),
            "id",
        )
    }

    /// DataFrame of removed constraint reasons.
    ///
    /// Columns: id (index), removed_reason, removed_reason.{key}
    ///
    /// Can be joined with {attr}`constraints_df` using the `id` index.
    #[getter]
    pub fn removed_reasons_df<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDataFrame>> {
        use crate::pandas::RemovedReasonEntry;
        entries_to_dataframe(
            py,
            self.inner
                .constraints()
                .removed_reasons()
                .iter()
                .map(|(id, reason)| RemovedReasonEntry {
                    id: id.into_inner(),
                    reason,
                }),
            "id",
        )
    }

    /// DataFrame of indicator constraints with per-sample value, feasibility, and indicator_active columns.
    /// Static columns: id, indicator_variable_id, equality, used_ids, name, subscripts, description.
    /// Dynamic columns: value.{sample_id}, feasible.{sample_id}, indicator_active.{sample_id} for each sample.
    #[getter]
    pub fn indicator_constraints_df<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let sample_ids = sorted_sample_ids(&self.inner);
        entries_to_dataframe(
            py,
            self.inner
                .indicator_constraints()
                .values()
                .map(|item| WithSampleIds {
                    item,
                    sample_ids: &sample_ids,
                }),
            "id",
        )
    }

    /// DataFrame of removed indicator constraint reasons.
    ///
    /// Columns: id (index), removed_reason, removed_reason.{key}
    ///
    /// Can be joined with {attr}`indicator_constraints_df` using the `id` index.
    #[getter]
    pub fn indicator_removed_reasons_df<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        use crate::pandas::RemovedReasonEntry;
        entries_to_dataframe(
            py,
            self.inner
                .indicator_constraints()
                .removed_reasons()
                .iter()
                .map(|(id, reason)| RemovedReasonEntry {
                    id: id.into_inner(),
                    reason,
                }),
            "id",
        )
    }

    /// DataFrame of named functions with per-sample value columns.
    /// Static columns: id, used_ids, name, subscripts, description, parameters.
    /// Dynamic columns: one per sample_id (int) with the function's evaluated value.
    #[getter]
    pub fn named_functions_df<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDataFrame>> {
        let sample_ids = sorted_sample_ids(&self.inner);
        entries_to_dataframe(
            py,
            self.inner
                .named_functions()
                .values()
                .map(|item| WithSampleIds {
                    item,
                    sample_ids: &sample_ids,
                }),
            "id",
        )
    }
}

fn sorted_sample_ids(inner: &ommx::SampleSet) -> Vec<ommx::SampleID> {
    let mut ids: Vec<_> = inner.sample_ids().iter().copied().collect();
    ids.sort();
    ids
}
