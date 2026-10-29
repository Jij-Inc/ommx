use crate::{
    constraint_kind::{constraint_kind_collection, ConstraintKind},
    error::OmmxPyResult,
    pandas::{
        constraint_id_col, entries_to_dataframe, sorted_entries_to_dataframe, PyDataFrame,
        ToPandasEntry, WithSampleIds,
    },
    Solution,
};
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict, PyTuple},
    Bound, PyResult, Python,
};
use std::collections::{BTreeMap, BTreeSet};

/// Multiple candidates evaluated against the same optimization problem.
///
/// {class}`~ommx.Samples` contains candidate assignments identified by sample
/// IDs; a SampleSet adds their objective values, feasibility, evaluated variables
/// and constraints, and modeling labels. Samples corresponds to
/// {class}`~ommx.State`, while SampleSet corresponds to {class}`~ommx.Solution`.
/// Candidates may come from a sampler or be supplied manually.
///
/// Use {meth}`~ommx.Instance.evaluate_samples` with Samples, a dictionary of
/// sample IDs to assignments, or an iterable of assignments. Explicit IDs are
/// preserved; iterables receive IDs from 0. See {class}`~ommx.Samples` for
/// construction and repeated-assignment semantics.
///
/// **Evaluate several candidates**
///
/// Let's consider a simple optimization problem:
///
/// maximize x_1 + 2 x_2 + 3 x_3
/// subject to x_1 + x_2 + x_3 = 1
/// x_1, x_2, x_3 in {0, 1}
///
/// >>> from ommx import DecisionVariable, Instance, Sense
/// >>> x = [DecisionVariable.binary(i) for i in range(3)]
/// >>> instance = Instance.from_components(
/// ...     decision_variables=x,
/// ...     objective=x[0] + 2*x[1] + 3*x[2],
/// ...     constraints={0: sum(x) == 1},
/// ...     sense=Sense.Maximize,
/// ... )
///
/// with three samples:
///
/// >>> samples = {
/// ...     0: {0: 1, 1: 0, 2: 0},  # x1 = 1, x2 = x3 = 0
/// ...     1: {0: 0, 1: 0, 2: 1},  # x3 = 1, x1 = x2 = 0
/// ...     2: {0: 1, 1: 1, 2: 0},  # x1 = x2 = 1, x3 = 0 (infeasible)
/// ... } # ^ sample ID
///
/// Evaluate all candidates using the same Instance:
///
/// >>> sample_set = instance.evaluate_samples(samples)
/// >>> sample_set.summary  # doctest: +NORMALIZE_WHITESPACE
///            objective  feasible
/// sample_id
/// 1                3.0      True
/// 0                1.0      True
/// 2                3.0     False
///
/// {attr}`~ommx.SampleSet.summary` shows each sample's objective and feasibility.
/// Feasible candidates appear first, ordered by the objective according to
/// {attr}`~ommx.SampleSet.sense`. Feasibility checks variable kinds and bounds
/// and all retained constraints, including removed constraints.
/// Retrieve an individual sample as a Solution using its ID:
///
/// >>> solution = sample_set.get(sample_id=0)
/// >>> solution.objective
/// 1.0
///
/// {attr}`~ommx.SampleSet.best_feasible` selects the largest objective value
/// among feasible samples for this maximization problem:
///
/// >>> solution = sample_set.best_feasible
/// >>> solution.objective
/// 3.0
///
/// Minimization selects the smallest objective instead. This is the best among
/// the supplied candidates, not a proof of global optimality. Selection raises
/// ValueError when no candidate is feasible, including for an empty SampleSet.
///
/// **Inspect a candidate's values and diagnostics**
///
/// The Solution returned by {meth}`~ommx.SampleSet.get` retains variable labels,
/// evaluated constraints, named metrics, and the evaluation tolerance. Use its
/// extraction methods or query {meth}`~ommx.SampleSet.extract_decision_variables`,
/// {meth}`~ommx.SampleSet.extract_constraints`, and
/// {meth}`~ommx.SampleSet.extract_named_functions` with a sample ID directly.
/// {class}`~ommx.NamedFunction` records additional metrics without changing the
/// objective or adding feasibility conditions.
///
/// {attr}`~ommx.SampleSet.constraints` exposes per-constraint results across
/// samples through {class}`~ommx.SampledConstraintCollection`.
/// {meth}`~ommx.SampleSet.constraints_df` selects one family for a table;
/// {attr}`~ommx.SampleSet.summary` is the per-sample overview.
///
/// **Evaluate transformed problems**
///
/// Evaluate candidates with the transformed Instance supplied to the sampler.
/// It retains the fixed values and dependencies needed to restore variables,
/// removed constraints to inspect, and any preserved output objective.
/// Evaluation reports that output objective and sense, which may differ from
/// the active solver-facing objective.
///
/// >>> from ommx import Instance, Optimality, Sense
/// >>> original = Instance.maximize()
/// >>> x = original.new_binary("x")
/// >>> y = original.new_binary("y")
/// >>> original.objective = 3 * x + y
/// >>> prepared = original.partial_evaluate({x.id: 1})
/// >>> assert prepared.convert_active_objective(Sense.Minimize)
/// >>> results = prepared.evaluate_samples({7: {y.id: 0}, 9: {y.id: 1}})
/// >>> assert results.sense == Sense.Maximize
/// >>> assert results.objectives == {7: 3.0, 9: 4.0}
/// >>> assert results.get(7).state.entries == {x.id: 1.0, y.id: 0.0}
/// >>> assert results.best_feasible_id == 9
/// >>> assert results.best_feasible.optimality == Optimality.Unspecified
///
/// Successful value reconstruction, feasibility, and optimality remain separate
/// questions. {attr}`~ommx.SampleSet.feasible` includes retained removed
/// constraints; {attr}`~ommx.SampleSet.feasible_relaxed` excludes them while
/// still checking all variable domains. See {class}`~ommx.Solution` for output
/// interpretation and {meth}`~ommx.Instance.populate_state` for reconstruction,
/// defaults, and normalization. Choosing the best supplied candidate never
/// establishes optimality of the original or transformed problem.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct SampleSet {
    pub(crate) inner: ommx::SampleSet,
}

impl_solution_annotations!(SampleSet);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl SampleSet {
    /// Unique names across all constraint families, as a set of strings.
    ///
    /// Retained removed constraints are included; no sample ID is needed.
    /// The scope matches `extract_constraints`. Unnamed constraints are excluded;
    /// an explicitly empty name is included. Duplicate names appear once, even
    /// when extraction would reject duplicate subscripts.
    /// Each access returns a fresh set; editing it does not change this owner.
    #[getter]
    fn constraint_names(&self) -> BTreeSet<String> {
        self.inner.constraint_names()
    }

    #[staticmethod]
    pub fn from_v1_bytes(bytes: &Bound<PyBytes>) -> OmmxPyResult<Self> {
        let _guard = crate::TRACING.attach_parent_context(bytes.py());
        Ok(Self {
            inner: ommx::SampleSet::from_v1_bytes(bytes.as_bytes())?,
        })
    }

    #[staticmethod]
    pub fn from_v2_bytes(bytes: &Bound<PyBytes>) -> OmmxPyResult<Self> {
        let _guard = crate::TRACING.attach_parent_context(bytes.py());
        Ok(Self {
            inner: ommx::SampleSet::from_v2_bytes(bytes.as_bytes())?,
        })
    }

    pub fn to_v1_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        let _guard = crate::TRACING.attach_parent_context(py);
        PyBytes::new(py, &self.inner.to_v1_bytes())
    }

    pub fn to_v2_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        let _guard = crate::TRACING.attach_parent_context(py);
        PyBytes::new(py, &self.inner.to_v2_bytes())
    }

    /// Return one sample as a Solution.
    ///
    /// ``sample_id`` is the candidate's ID, not its row position in
    /// {attr}`~ommx.SampleSet.summary`. The independent result contains evaluated
    /// values, variable and constraint labels, retained removed constraints,
    /// named functions, reported objective and sense, and the stored tolerance.
    /// Changing the extracted Solution does not update the SampleSet.
    /// Raises KeyError if the sample ID does not exist.
    ///
    /// >>> from ommx import Instance, Solution
    /// >>> instance = Instance.maximize()
    /// >>> x = instance.new_binary("selected", subscripts=[0])
    /// >>> instance.objective = x
    /// >>> results = instance.evaluate_samples({10: {x.id: 0}, 20: {x.id: 1}})
    /// >>> assert list(results.summary.index) == [20, 10]
    /// >>> solution = results.get(sample_id=10)
    /// >>> assert isinstance(solution, Solution)
    /// >>> assert solution.objective == 0.0
    /// >>> assert solution.extract_decision_variables("selected") == {(0,): 0.0}
    pub fn get(&self, sample_id: u64) -> PyResult<Solution> {
        let sample_id = ommx::SampleID::from(sample_id);
        let solution = self.inner.get(sample_id).ok_or_else(|| {
            pyo3::exceptions::PyKeyError::new_err(format!(
                "Unknown sample ID: {}",
                sample_id.into_inner()
            ))
        })?;
        Ok(Solution { inner: solution })
    }

    /// Get sample by ID (alias for get method).
    ///
    /// Raises KeyError if the sample ID does not exist.
    pub fn get_sample_by_id(&self, sample_id: u64) -> PyResult<Solution> {
        self.get(sample_id)
    }

    /// Number of candidate IDs, including repeated assignments with different IDs.
    pub fn num_samples(&self) -> usize {
        self.inner.sample_ids().len()
    }

    /// All candidate IDs as an independent set of integers.
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
    /// ID of the best feasible candidate, using the same selection as
    /// {attr}`~ommx.SampleSet.best_feasible`.
    ///
    /// Raises ValueError if there is no feasible sample, including when empty.
    pub fn best_feasible_id(&self) -> OmmxPyResult<u64> {
        Ok(self.inner.best_feasible_id()?.into_inner())
    }

    #[getter]
    /// ID of the best candidate satisfying {attr}`~ommx.SampleSet.feasible_relaxed`.
    ///
    /// Uses the reported objective and {attr}`~ommx.SampleSet.sense`, like
    /// {attr}`~ommx.SampleSet.best_feasible_relaxed`.
    /// Raises ValueError if there is no such candidate, including when empty.
    pub fn best_feasible_relaxed_id(&self) -> OmmxPyResult<u64> {
        Ok(self.inner.best_feasible_relaxed_id()?.into_inner())
    }

    #[getter]
    /// The best feasible candidate as an independent Solution.
    ///
    /// Selects only candidates satisfying {attr}`~ommx.SampleSet.feasible`,
    /// including variable domains and retained removed constraints. Minimization
    /// selects the smallest reported objective; maximization selects the largest.
    /// Selection does not prove global optimality or change the extracted
    /// Solution's optimality status.
    ///
    /// Raises ValueError if there is no feasible candidate, including when the
    /// SampleSet is empty. There is no universal ordering of infeasible candidates:
    /// choose your own criteria from the constraint and variable diagnostics.
    /// The objective ordering in ``summary`` is not such a feasibility criterion.
    ///
    /// >>> from ommx import Instance
    /// >>> instance = Instance.maximize()
    /// >>> x = instance.new_binary("x")
    /// >>> instance.objective = x
    /// >>> _ = instance.add_constraint(x == 1, "required")
    /// >>> results = instance.evaluate_samples({10: {x.id: 0}, 20: {x.id: 1}})
    /// >>> assert results.best_feasible_id == 20
    /// >>> assert results.best_feasible.state.entries == {x.id: 1.0}
    /// >>> infeasible = instance.evaluate_samples([{x.id: 0}])
    /// >>> infeasible.best_feasible
    /// Traceback (most recent call last):
    /// ...
    /// ValueError: No feasible solution found
    pub fn best_feasible(&self) -> OmmxPyResult<Solution> {
        Ok(Solution {
            inner: self.inner.best_feasible()?,
        })
    }

    #[getter]
    /// The best candidate satisfying relaxed feasibility as an independent Solution.
    ///
    /// Uses {attr}`~ommx.SampleSet.feasible_relaxed`: removed constraints are
    /// excluded, while all variable kinds and bounds still apply. The selected
    /// candidate can therefore fail {attr}`~ommx.Solution.feasible`.
    /// Comparison still uses the reported objective and sense; it does not switch
    /// to an active solver-facing objective. Raises ValueError if there is no
    /// qualifying candidate, including when empty.
    pub fn best_feasible_relaxed(&self) -> OmmxPyResult<Solution> {
        Ok(Solution {
            inner: self.inner.best_feasible_relaxed()?,
        })
    }

    #[getter]
    /// Alias for {attr}`~ommx.SampleSet.best_feasible`, including removed constraints.
    /// Raises ValueError if there is no feasible sample.
    pub fn best_feasible_unrelaxed(&self) -> OmmxPyResult<Solution> {
        // Exactly the same as best_feasible
        self.best_feasible()
    }

    /// Reported objective values keyed by sample ID.
    ///
    /// Evaluation uses the Instance's preserved output objective when present,
    /// otherwise its active objective. An infeasible candidate can still have
    /// an objective value; read this together with {attr}`~ommx.SampleSet.feasible`.
    #[getter]
    pub fn objectives(&self) -> BTreeMap<u64, f64> {
        self.inner
            .objectives()
            .iter()
            .map(|(sample_id, objective)| (sample_id.into_inner(), *objective))
            .collect()
    }

    /// Get feasibility status for all samples, including variable bounds and kinds.
    ///
    /// Includes all retained regular and special constraints, even removed ones.
    /// Each value matches {attr}`~ommx.Solution.feasible` on the corresponding
    /// extracted Solution and uses {attr}`~ommx.SampleSet.feasibility_atol`.
    #[getter]
    pub fn feasible(&self) -> BTreeMap<u64, bool> {
        self.inner
            .feasible()
            .iter()
            .map(|(sample_id, &is_feasible)| (sample_id.into_inner(), is_feasible))
            .collect()
    }

    /// Get relaxed feasibility status for all samples.
    ///
    /// Removed constraints are excluded; variable bounds and kinds still apply.
    #[getter]
    pub fn feasible_relaxed(&self) -> BTreeMap<u64, bool> {
        self.inner
            .feasible_relaxed()
            .iter()
            .map(|(sample_id, &is_feasible)| (sample_id.into_inner(), is_feasible))
            .collect()
    }

    /// Alias for {attr}`~ommx.SampleSet.feasible`, keyed by sample ID.
    #[getter]
    pub fn feasible_unrelaxed(&self) -> BTreeMap<u64, bool> {
        self.feasible()
    }

    /// Absolute tolerance associated with the stored evaluation and feasibility results.
    ///
    /// Pass this to an extracted constraint's explicit feasibility query to use
    /// the enclosing result's threshold.
    #[getter]
    pub fn feasibility_atol(&self) -> f64 {
        self.inner.feasibility_atol().into_inner()
    }

    /// The sense associated with the reported objective values.
    ///
    /// This is the preserved output sense when recorded by the evaluated
    /// Instance, otherwise its active sense. ``summary`` and the ``best_feasible``
    /// selectors use it even if a transformation changed the solver-facing sense.
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
        let labels = self.inner.named_function_labels();
        self.inner
            .named_functions()
            .iter()
            .map(|(id, nf)| crate::SampledNamedFunction(*id, nf.clone(), labels.collect_for(*id)))
            .collect()
    }

    /// Live mapping of all constraint families, including removed results.
    /// Typed IDs select the family. Rows are fetched only when indexed.
    #[getter]
    pub fn constraints(
        slf: Bound<'_, Self>,
    ) -> crate::constraint_collection::SampledConstraintCollection {
        crate::constraint_collection::SampledConstraintCollection::new(slf.unbind())
    }

    /// Get decision variables for compatibility with existing Python code
    #[getter]
    pub fn decision_variables(&self) -> Vec<crate::SampledDecisionVariable> {
        let labels = self.inner.variable_labels();
        self.inner
            .decision_variables()
            .iter()
            .map(|(id, variable)| {
                crate::SampledDecisionVariable::from_parts(
                    *id,
                    variable.clone(),
                    labels.collect_for(*id),
                )
            })
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
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
    /// >>> y = [DecisionVariable.binary(i+3, name="y", subscripts=[i]) for i in range(2)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x + y,
    /// ...     objective=sum(x) + sum(y),
    /// ...     constraints={},
    /// ...     sense=Sense.Maximize,
    /// ... )
    /// >>> sample_set = instance.evaluate_samples({0: {i: 1 for i in range(5)}})
    /// >>> sorted(sample_set.decision_variable_names)
    /// ['x', 'y']
    #[getter]
    pub fn decision_variable_names(&self) -> BTreeSet<String> {
        self.inner.decision_variable_names()
    }

    /// Get all unique named function names in this sample set
    #[getter]
    pub fn named_function_names(&self) -> BTreeSet<String> {
        self.inner.named_function_names()
    }

    /// Extract decision variable values for a given name and sample ID.
    ///
    /// Returns a dictionary keyed by subscript tuples. A scalar variable uses
    /// ``()``. This is equivalent to extracting the name from ``get(sample_id)``;
    /// names and subscripts identify matches, not parameters or descriptions.
    ///
    /// Raises KeyError if the name or sample ID does not exist, and ValueError
    /// if the same subscript is found more than once.
    ///
    /// >>> from ommx import Instance
    /// >>> instance = Instance.maximize()
    /// >>> x = instance.new_binary("selected", subscripts=[0])
    /// >>> instance.objective = x
    /// >>> results = instance.evaluate_samples({7: {x.id: 1}})
    /// >>> assert results.extract_decision_variables("selected", 7) == {(0,): 1.0}
    /// >>> assert results.get(7).extract_decision_variables("selected") == {(0,): 1.0}
    pub fn extract_decision_variables<'py>(
        &self,
        py: Python<'py>,
        name: &str,
        sample_id: u64,
    ) -> OmmxPyResult<Bound<'py, PyDict>> {
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
    /// Raises KeyError if the sample ID does not exist, and ValueError if the
    /// same name and subscript combination is found multiple times.
    ///
    /// # Examples
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
    /// >>> y = [DecisionVariable.binary(i+3, name="y", subscripts=[i]) for i in range(2)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x + y,
    /// ...     objective=sum(x) + sum(y),
    /// ...     constraints={},
    /// ...     sense=Sense.Maximize,
    /// ... )
    /// >>> sample_set = instance.evaluate_samples({0: {i: 1 for i in range(5)}})
    /// >>> all_vars = sample_set.extract_all_decision_variables(0)
    /// >>> all_vars["x"]
    /// {(0,): 1.0, (1,): 1.0, (2,): 1.0}
    /// >>> all_vars["y"]
    /// {(0,): 1.0, (1,): 1.0}
    pub fn extract_all_decision_variables<'py>(
        &self,
        py: Python<'py>,
        sample_id: u64,
    ) -> OmmxPyResult<Bound<'py, PyDict>> {
        let sample_id = ommx::SampleID::from(sample_id);
        let extracted = self.inner.extract_all_decision_variables(sample_id)?;
        let result_dict = PyDict::new(py);
        for (name, variables) in extracted {
            let var_dict = PyDict::new(py);
            for (subscripts, value) in variables {
                let key_tuple = PyTuple::new(py, &subscripts)?;
                var_dict.set_item(key_tuple, value)?;
            }
            result_dict.set_item(name, var_dict)?;
        }
        Ok(result_dict)
    }

    /// Extract nonnegative violations of all constraint families by name for one sample.
    ///
    /// Raises KeyError if the name or sample ID does not exist. Raises
    /// ValueError if matching constraints share subscripts, including across families.
    /// Parameters and descriptions are ignored. Retained removed constraints are
    /// included. Regular constraints return violations, not signed function values.
    fn extract_constraints(
        &self,
        name: &str,
        sample_id: u64,
    ) -> OmmxPyResult<crate::subscript_map::SubscriptMap<f64>> {
        Ok(crate::subscript_map::SubscriptMap(
            self.inner.extract_constraints(name, sample_id.into())?,
        ))
    }

    /// Extract named function values for a given name and sample ID.
    ///
    /// Returns a dictionary keyed by subscript tuples, with ``()`` for a scalar
    /// metric. Named functions report candidate metrics without changing the
    /// objective or feasibility conditions. See {meth}`~ommx.Solution.extract_named_functions`
    /// for the corresponding single-candidate query.
    ///
    /// Raises KeyError if the name or sample ID does not exist, and ValueError
    /// if the same subscript is found more than once.
    ///
    /// >>> from ommx import DecisionVariable, Instance, NamedFunction, Sense
    /// >>> x = DecisionVariable.binary(0, name="selected")
    /// >>> cost = NamedFunction(id=10, function=4 * x, name="cost")
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x], objective=x, constraints={},
    /// ...     sense=Sense.Maximize, named_functions=[cost],
    /// ... )
    /// >>> results = instance.evaluate_samples({7: {x.id: 0}, 9: {x.id: 1}})
    /// >>> assert results.extract_named_functions("cost", 9) == {(): 4.0}
    /// >>> assert results.get(9).extract_named_functions("cost") == {(): 4.0}
    pub fn extract_named_functions<'py>(
        &self,
        py: Python<'py>,
        name: &str,
        sample_id: u64,
    ) -> OmmxPyResult<Bound<'py, PyDict>> {
        let sample_id = ommx::SampleID::from(sample_id);
        let extracted = self.inner.extract_named_functions(name, sample_id)?;
        let dict = PyDict::new(py);
        for (subscripts, value) in extracted {
            let key = PyTuple::new(py, &subscripts)?;
            dict.set_item(key, value)?;
        }
        Ok(dict)
    }

    /// Extract all named function values grouped by name for a given sample ID.
    ///
    /// Raises KeyError if the sample ID does not exist, and ValueError if the
    /// same name and subscript combination is found multiple times.
    pub fn extract_all_named_functions<'py>(
        &self,
        py: Python<'py>,
        sample_id: u64,
    ) -> OmmxPyResult<Bound<'py, PyDict>> {
        let sample_id = ommx::SampleID::from(sample_id);
        let extracted = self.inner.extract_all_named_functions(sample_id)?;
        let result_dict = PyDict::new(py);
        for (name, functions) in extracted {
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
        let labels = self.inner.variable_labels();
        self.inner
            .decision_variables()
            .get(&var_id)
            .map(|dv| {
                crate::SampledDecisionVariable::from_parts(
                    var_id,
                    dv.clone(),
                    labels.collect_for(var_id),
                )
            })
            .ok_or_else(|| {
                pyo3::exceptions::PyKeyError::new_err(format!(
                    "Unknown decision variable ID: {variable_id}"
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
            .map(|nf| {
                crate::SampledNamedFunction(
                    named_function_id,
                    nf.clone(),
                    self.inner
                        .named_function_labels()
                        .collect_for(named_function_id),
                )
            })
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

    /// A materialized overview with one row per sample ID.
    ///
    /// The index is ``sample_id`` and the columns are ``objective`` and
    /// ``feasible``. Feasibility matches {attr}`~ommx.SampleSet.feasible`, including
    /// variable kinds and bounds and all retained constraints, even removed ones.
    /// Feasible candidates appear first; within each feasibility group, objective
    /// values are ascending for minimization and descending for maximization,
    /// according to {attr}`~ommx.SampleSet.sense`.
    ///
    /// The index keeps the candidate's ID rather than its displayed row position;
    /// use that ID with {meth}`~ommx.SampleSet.get`. Identical assignments under
    /// different IDs remain separate rows. Editing the DataFrame does not update
    /// the SampleSet. See the {class}`~ommx.SampleSet` example for the table layout.
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

    /// DataFrame of decision variables with per-sample value columns.
    /// Static columns: id, kind, lower, upper, name, subscripts, description.
    /// Dynamic columns: one per sample_id (int) with the variable's value.
    #[pyo3(signature = (include = None))]
    pub fn decision_variables_df<'py>(
        &self,
        py: Python<'py>,
        include: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let flags = crate::pandas::IncludeFlags::from_optional(include)?;
        let sample_ids = sorted_sample_ids(&self.inner);
        let var_meta_store = self.inner.variable_labels().clone();
        let view: Vec<(
            ommx::DecisionVariableLabel,
            ommx::VariableID,
            &ommx::SampledDecisionVariable,
        )> = self
            .inner
            .decision_variables()
            .iter()
            .map(|(id, dv)| (var_meta_store.collect_for(*id), *id, dv))
            .collect();
        entries_to_dataframe(
            py,
            view.iter().map(|(m, id, dv)| {
                crate::pandas::WithModelingContext::new(
                    WithSampleIds {
                        item: (*id, *dv),
                        sample_ids: &sample_ids,
                    },
                    m,
                )
            }),
            "id",
            flags,
        )
    }

    /// DataFrame of sampled constraints, dispatched on `kind=`. See
    /// {meth}`ommx.Instance.constraints_df` for column / `kind=` /
    /// `include=` semantics. Adds dynamic per-sample columns
    /// (`value.{sample_id}`, `feasible.{sample_id}`, etc.) on top of
    /// the kind-specific core columns.
    ///
    /// `SampleSet` has no `removed=` parameter (no active/removed
    /// distinction at the sampled stage); reason data is gated by
    /// `"removed_reason"` in `include=`.
    #[pyo3(signature = (*, kind, include = None))]
    pub fn constraints_df<'py>(
        &self,
        py: Python<'py>,
        kind: ConstraintKind,
        include: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let id_col = constraint_id_col(kind);
        let flags = crate::pandas::IncludeFlags::from_optional(include)?;
        let sample_ids = sorted_sample_ids(&self.inner);
        constraint_kind_collection!(
            self.inner,
            kind,
            [
                constraints,
                indicator_constraints,
                one_hot_constraints,
                sos1_constraints
            ],
            |coll| {
                let meta = coll.context().clone();
                let removed_reasons = coll.removed_reasons();
                let mut entries: Vec<Bound<'py, pyo3::types::PyAny>> = Vec::new();
                for (id, c) in coll.inner().iter() {
                    let m = meta.collect_for(*id);
                    let dict = crate::pandas::WithModelingContext::new(
                        WithSampleIds {
                            item: (*id, c, self.inner.feasibility_atol()),
                            sample_ids: &sample_ids,
                        },
                        &m,
                    )
                    .to_pandas_entry(py)?;
                    if flags.removed_reason {
                        if let Some(reason) = removed_reasons.get(id) {
                            crate::pandas::set_removed_reason_columns(&dict, reason)?;
                        } else {
                            crate::pandas::set_removed_reason_na(&dict)?;
                        }
                    }
                    crate::pandas::apply_include_filter(&dict, flags)?;
                    crate::pandas::rename_id_column(&dict, id_col)?;
                    entries.push(dict.into_any());
                }
                crate::pandas::raw_entries_to_dataframe(py, entries, id_col)
            }
        )
    }

    /// DataFrame of named functions with per-sample value columns.
    /// Static columns: id, used_ids, name, subscripts, description, parameters.{key}.
    /// Dynamic columns: one per sample_id (int) with the function's evaluated value.
    #[pyo3(signature = (include = None))]
    pub fn named_functions_df<'py>(
        &self,
        py: Python<'py>,
        include: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let flags = crate::pandas::IncludeFlags::from_optional(include)?;
        let sample_ids = sorted_sample_ids(&self.inner);
        let nf_meta_store = self.inner.named_function_labels().clone();
        let nf_meta_view: Vec<(
            ommx::NamedFunctionLabel,
            ommx::NamedFunctionID,
            &ommx::SampledNamedFunction,
        )> = self
            .inner
            .named_functions()
            .iter()
            .map(|(id, nf)| (nf_meta_store.collect_for(*id), *id, nf))
            .collect();
        entries_to_dataframe(
            py,
            nf_meta_view.iter().map(|(m, id, nf)| {
                crate::pandas::WithModelingContext::new(
                    WithSampleIds {
                        item: (*id, *nf),
                        sample_ids: &sample_ids,
                    },
                    m,
                )
            }),
            "id",
            flags,
        )
    }

    /// Constraint context DataFrame (id-indexed). See
    /// {meth}`ommx.Instance.constraint_context_df` for column / `kind=`
    /// semantics. Reads from the sampled collection's context store.
    #[pyo3(signature = (*, kind))]
    pub fn constraint_context_df<'py>(
        &self,
        py: Python<'py>,
        kind: ConstraintKind,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let id_col = constraint_id_col(kind);
        constraint_kind_collection!(
            self.inner,
            kind,
            [
                constraints,
                indicator_constraints,
                one_hot_constraints,
                sos1_constraints
            ],
            |coll| {
                crate::pandas::constraint_context_dataframe(
                    py,
                    coll.context(),
                    coll.inner().keys().copied(),
                    id_col,
                )
            }
        )
    }

    /// Constraint parameters DataFrame (long format).
    #[pyo3(signature = (*, kind))]
    pub fn constraint_parameters_df<'py>(
        &self,
        py: Python<'py>,
        kind: ConstraintKind,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let id_col = constraint_id_col(kind);
        constraint_kind_collection!(
            self.inner,
            kind,
            [
                constraints,
                indicator_constraints,
                one_hot_constraints,
                sos1_constraints
            ],
            |coll| {
                crate::pandas::constraint_parameters_dataframe(
                    py,
                    coll.context(),
                    coll.inner().keys().copied(),
                    id_col,
                )
            }
        )
    }

    /// Constraint provenance DataFrame (long format).
    #[pyo3(signature = (*, kind))]
    pub fn constraint_provenance_df<'py>(
        &self,
        py: Python<'py>,
        kind: ConstraintKind,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let id_col = constraint_id_col(kind);
        constraint_kind_collection!(
            self.inner,
            kind,
            [
                constraints,
                indicator_constraints,
                one_hot_constraints,
                sos1_constraints
            ],
            |coll| {
                crate::pandas::constraint_provenance_dataframe(
                    py,
                    coll.context(),
                    coll.inner().keys().copied(),
                    id_col,
                )
            }
        )
    }

    /// Removed-constraint reasons DataFrame (long format).
    #[pyo3(signature = (*, kind))]
    pub fn constraint_removed_reasons_df<'py>(
        &self,
        py: Python<'py>,
        kind: ConstraintKind,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let id_col = constraint_id_col(kind);
        constraint_kind_collection!(
            self.inner,
            kind,
            [
                constraints,
                indicator_constraints,
                one_hot_constraints,
                sos1_constraints
            ],
            |coll| {
                crate::pandas::constraint_removed_reasons_dataframe(
                    py,
                    coll.removed_reasons().iter().map(|(id, r)| (*id, r)),
                    id_col,
                )
            }
        )
    }

    /// Decision-variable modeling-label DataFrame (id-indexed).
    pub fn variable_labels_df<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDataFrame>> {
        crate::pandas::variable_labels_dataframe(
            py,
            self.inner.variable_labels(),
            self.inner.decision_variables().keys().copied(),
            "variable_id",
        )
    }

    /// Decision-variable parameters DataFrame (long format).
    pub fn variable_parameters_df<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        crate::pandas::variable_parameters_dataframe(
            py,
            self.inner.variable_labels(),
            self.inner.decision_variables().keys().copied(),
            "variable_id",
        )
    }
}

fn sorted_sample_ids(inner: &ommx::SampleSet) -> Vec<ommx::SampleID> {
    let mut ids: Vec<_> = inner.sample_ids().iter().copied().collect();
    ids.sort();
    ids
}
