use crate::constraint_id::ConstraintIDInput;
use crate::{
    constraint_kind::{constraint_kind_collection, ConstraintKind},
    error::OmmxPyResult,
    pandas::{constraint_id_col, entries_to_dataframe, PyDataFrame, ToPandasEntry},
};
use pyo3::{
    exceptions::PyKeyError,
    exceptions::PyRuntimeError,
    prelude::*,
    types::{PyBytes, PyDict, PyTuple},
    Bound,
};
use std::collections::BTreeSet;

/// A candidate evaluated against a particular optimization problem.
///
/// A {class}`~ommx.State` maps variable IDs to values; it does not say whether
/// those values satisfy a problem. A Solution adds the objective value,
/// feasibility results, evaluated variables and constraints, and their modeling
/// labels. Use {meth}`~ommx.Instance.evaluate` with a State or an ID-to-value
/// dictionary to obtain one, whether the candidate came from a solver or was
/// supplied manually.
///
/// **Evaluate a candidate**
///
/// This problem maximizes ``x + y`` while forbidding both binary variables
/// from being 1. Evaluation checks one assignment; it does not search for a
/// better assignment or prove optimality.
///
/// >>> from ommx import Instance, Optimality, State
/// >>> instance = Instance.maximize()
/// >>> x = instance.new_binary("x")
/// >>> y = instance.new_binary("y")
/// >>> instance.objective = x + y
/// >>> exclusive = instance.add_constraint(x * y == 0, "exclusive")
/// >>> state = State({x.id: 1, y.id: 0})
/// >>> solution = instance.evaluate(state)
/// >>> assert solution.state.entries == {x.id: 1.0, y.id: 0.0}
/// >>> assert solution.feasible
/// >>> assert solution.objective == 1.0
/// >>> assert solution.optimality == Optimality.Unspecified
///
/// **Read feasibility, objective, and optimality separately**
///
/// | Property | Meaning | Does not establish |
/// | --- | --- | --- |
/// | {attr}`~ommx.Solution.feasible` | Variable domains and all retained constraints are satisfied | No better candidate exists |
/// | {attr}`~ommx.Solution.objective` | The candidate's reported objective value | Feasibility or optimality |
/// | {attr}`~ommx.Solution.optimality` | Recorded optimality status, for example supplied by an adapter | A proof performed by ``evaluate`` |
///
/// An infeasible candidate can still have an objective value when its
/// expressions can be evaluated. It cannot be selected as a better feasible
/// solution merely because that value looks preferable.
///
/// >>> infeasible = instance.evaluate({x.id: 1, y.id: 1})
/// >>> assert infeasible.objective == 2.0
/// >>> assert not infeasible.feasible
///
/// Feasibility includes variable ``kind`` and bounds, not only constraints.
/// Floating-point checks use the tolerance supplied to ``evaluate(..., atol=...)``;
/// {attr}`~ommx.Solution.feasibility_atol` records it. The exact rules for
/// discrete-value normalization, reconstruction, and omitted values belong to
/// {meth}`~ommx.Instance.populate_state`.
///
/// **Inspect evaluated values**
///
/// {meth}`~ommx.Solution.decision_variables_df` combines variable definitions
/// and labels with evaluated values. {meth}`~ommx.Solution.constraints_df`
/// selects one constraint family and includes its evaluation diagnostics.
/// For a regular constraint ``f(x) <= 0``, the signed value ``f(x)`` and the
/// nonnegative violation ``max(0, f(x))`` are different quantities.
///
/// For queries without pandas, {attr}`~ommx.Solution.constraints` is a live
/// {class}`~ommx.EvaluatedConstraintCollection` indexed by typed constraint IDs.
/// It includes active constraints and constraints retained after removal.
/// Each fetched row is an independent snapshot; editing it does not update
/// the Solution. Use an owner method such as
/// {meth}`~ommx.Solution.set_dual_variable` when changing result data.
/// {class}`~ommx.ConstraintFamilyView` documents family-filtered queries, and
/// {class}`~ommx.ConstraintIDBase` documents the extensible ID interface.
///
/// {meth}`~ommx.Solution.extract_decision_variables` and
/// {meth}`~ommx.Solution.extract_constraints` use names and subscript tuples to
/// retrieve variable values and constraint violations for application-level
/// analysis. {class}`~ommx.NamedFunction` expressions registered on the Instance
/// provide additional reported metrics through
/// {meth}`~ommx.Solution.extract_named_functions` or
/// {meth}`~ommx.Solution.named_functions_df`. These metrics do not add an
/// objective or a feasibility condition.
///
/// **Evaluate a transformed problem**
///
/// Evaluate a solver's returned State with the transformed Instance that was
/// supplied to it. That Instance retains the fixed values, dependencies,
/// removed constraints, and output objective needed to interpret the result.
/// Passing the State directly to the original Instance may lose that context
/// or leave required original variables unspecified.
///
/// >>> from ommx import Instance, Optimality, Sense
/// >>> original = Instance.maximize()
/// >>> x = original.new_binary("x")
/// >>> y = original.new_binary("y")
/// >>> original.objective = 3 * x + y
/// >>> prepared = original.partial_evaluate({x.id: 1})
/// >>> assert prepared.convert_active_objective(Sense.Minimize)
/// >>> restored = prepared.evaluate({y.id: 0})
/// >>> assert restored.state.entries == {x.id: 1.0, y.id: 0.0}
/// >>> assert prepared.objective.evaluate({y.id: 0}) == -3.0
/// >>> assert (restored.sense, restored.objective) == (Sense.Maximize, 3.0)
/// >>> assert restored.optimality == Optimality.Unspecified
///
/// Fixed and dependent values are reconstructed. Otherwise irrelevant variables
/// omitted from the candidate receive defaults; a default is not a value found
/// by the solver. Supply such a value when it matters to reported metrics.
/// Distinguish successful reconstruction, feasibility including removed
/// constraints, and transport of objective or optimality information.
/// {attr}`~ommx.Solution.feasible_relaxed` excludes removed constraints but
/// still checks variable domains. Output objective and optimality transport
/// follow {attr}`~ommx.Instance.output_objective` and
/// {meth}`~ommx.Instance.map_active_optimality`, respectively.
///
/// Use {class}`~ommx.SampleSet` to evaluate multiple candidates with the same
/// rules. Being best among the candidates obtained does not establish global
/// optimality. Solution annotations can also be persisted in protobuf payloads
/// and mirrored to OMMX Artifact descriptors.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct Solution {
    pub(crate) inner: ommx::Solution,
}

impl_solution_annotations!(Solution);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Solution {
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
            inner: ommx::Solution::from_v1_bytes(bytes.as_bytes())?,
        })
    }

    #[staticmethod]
    pub fn from_v2_bytes(bytes: &Bound<PyBytes>) -> OmmxPyResult<Self> {
        let _guard = crate::TRACING.attach_parent_context(bytes.py());
        Ok(Self {
            inner: ommx::Solution::from_v2_bytes(bytes.as_bytes())?,
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

    /// Class constant for optimal solutions
    #[classattr]
    #[pyo3(name = "OPTIMAL")]
    fn class_optimal() -> crate::Optimality {
        crate::Optimality::Optimal
    }

    /// Class constant for non-optimal solutions
    #[classattr]
    #[pyo3(name = "NOT_OPTIMAL")]
    fn class_not_optimal() -> crate::Optimality {
        crate::Optimality::NotOptimal
    }

    /// Class constant for LP-relaxed solutions
    #[classattr]
    #[pyo3(name = "LP_RELAXED")]
    fn class_lp_relaxed() -> crate::Relaxation {
        crate::Relaxation::LpRelaxed
    }

    /// The reported objective value of this candidate.
    ///
    /// Evaluation uses the Instance's preserved output objective when one is
    /// present; otherwise it uses the active objective. Read the value together
    /// with {attr}`~ommx.Solution.sense` and {attr}`~ommx.Solution.feasible`.
    /// An infeasible candidate can have an objective value, and the value alone
    /// establishes neither feasibility nor optimality.
    #[getter]
    pub fn objective(&self) -> f64 {
        *self.inner.objective()
    }

    /// An independent State containing the evaluated variable values.
    ///
    /// Includes retained fixed and dependent values restored by evaluation and
    /// defaults supplied for omitted irrelevant variables. See
    /// {meth}`~ommx.Instance.populate_state` for the reconstruction rules.
    #[getter]
    pub fn state(&self) -> crate::State {
        crate::State(self.inner.state())
    }

    /// Whether all retained constraints and variable domains are satisfied.
    ///
    /// Checks all regular and special constraints, including removed constraints,
    /// together with the kind and bounds of every retained variable, using
    /// {attr}`~ommx.Solution.feasibility_atol`. This is an alias for
    /// {attr}`~ommx.Solution.feasible_unrelaxed`; it does not establish optimality.
    ///
    /// Compatibility: The meaning of this property has changed from Python SDK 1.7.0.
    /// Previously, this property represents the feasibility of the remaining constraints only,
    /// i.e. excluding relaxed constraints.
    /// From Python SDK 1.7.0, this property represents the feasibility of all constraints,
    /// including relaxed constraints.
    #[getter]
    pub fn feasible(&self) -> bool {
        // The `feasible` means feasible in the unrelaxed problem
        self.inner.feasible()
    }

    /// Whether active constraints and all retained variable domains are satisfied.
    ///
    /// Excludes constraints retained after removal, across all constraint families.
    /// Variable kind and bound checks are still applied with
    /// {attr}`~ommx.Solution.feasibility_atol`. A result can satisfy this property
    /// while failing {attr}`~ommx.Solution.feasible` because a removed constraint
    /// is violated.
    ///
    /// >>> from ommx import Instance
    /// >>> instance = Instance.minimize()
    /// >>> x = instance.new_binary("x")
    /// >>> instance.objective = x
    /// >>> required = instance.add_constraint(x == 1, "required")
    /// >>> instance.relax_constraint(required.constraint_id, "manual relaxation")
    /// >>> solution = instance.evaluate({x.id: 0})
    /// >>> assert solution.feasible_relaxed
    /// >>> assert not solution.feasible
    /// >>> assert solution.constraints[required.constraint_id].violation() == 1.0
    #[getter]
    pub fn feasible_relaxed(&self) -> bool {
        self.inner.feasible_relaxed()
    }

    /// Alias for {attr}`~ommx.Solution.feasible`, including removed constraints
    /// and every retained variable's kind and bounds.
    #[getter]
    pub fn feasible_unrelaxed(&self) -> bool {
        self.inner.feasible()
    }

    /// Absolute tolerance associated with the stored evaluation and feasibility results.
    ///
    /// Pass this to an extracted constraint's explicit feasibility query to use
    /// the enclosing result's threshold.
    #[getter]
    pub fn feasibility_atol(&self) -> f64 {
        self.inner.feasibility_atol().into_inner()
    }

    /// The optimization sense associated with the reported objective value.
    ///
    /// This is the preserved output sense when an output objective is recorded
    /// on the evaluated Instance, and otherwise its active sense. It can differ
    /// from the solver-facing sense after a transformation. Raises RuntimeError
    /// for older serialized results that do not record a sense.
    #[getter]
    pub fn sense(&self) -> PyResult<crate::Sense> {
        let sense = (*self.inner.sense()).ok_or_else(|| {
            PyRuntimeError::new_err(
                "This property is available for ommx.Solution generated by OMMX v2.0.0 or later.",
            )
        })?;
        Ok(sense.into())
    }

    /// The recorded optimality status, not a result of candidate evaluation.
    ///
    /// {meth}`~ommx.Instance.evaluate` produces ``Optimality.Unspecified``.
    /// An adapter may attach a solver's status, or a caller may set this property;
    /// setting it does not prove the claim. For a transformed formulation, use
    /// {meth}`~ommx.Instance.map_active_optimality` to determine which status
    /// transports to the reported output problem.
    #[getter]
    pub fn optimality(&self) -> crate::Optimality {
        self.inner.optimality.into()
    }

    /// Get the relaxation status
    #[getter]
    pub fn relaxation(&self) -> crate::Relaxation {
        self.inner.relaxation.into()
    }

    /// Set the optimality status
    #[setter]
    pub fn set_optimality(&mut self, optimality: crate::Optimality) {
        self.inner.optimality = optimality.into();
    }

    /// Set the relaxation status
    #[setter]
    pub fn set_relaxation(&mut self, relaxation: crate::Relaxation) {
        self.inner.relaxation = relaxation.into();
    }

    /// Get evaluated decision variables as a list sorted by ID
    #[getter]
    pub fn decision_variables(&self) -> Vec<crate::EvaluatedDecisionVariable> {
        // BTreeMap is already sorted by key
        let labels = self.inner.variable_labels();
        self.inner
            .decision_variables()
            .iter()
            .map(|(id, dv)| {
                crate::EvaluatedDecisionVariable::from_parts(
                    *id,
                    dv.clone(),
                    labels.collect_for(*id),
                )
            })
            .collect()
    }

    /// Live mapping of all constraint families, including removed results.
    ///
    /// Typed IDs select the family. Rows are fetched only when indexed, as
    /// independent snapshots with labels, provenance, removal reasons, and
    /// evaluation diagnostics. Changing a snapshot does not update this
    /// Solution; subsequent lookups read the owner's current result.
    /// Use {meth}`~ommx.Solution.set_dual_variable` to update a stored regular
    /// constraint's dual value.
    ///
    /// >>> from ommx import Instance
    /// >>> instance = Instance.minimize()
    /// >>> x = instance.new_binary("x")
    /// >>> capacity = instance.add_constraint(x <= 2, "capacity")
    /// >>> solution = instance.evaluate({x.id: 0})
    /// >>> rows = solution.constraints
    /// >>> row = rows[capacity.constraint_id]
    /// >>> assert row.evaluated_value == -2.0
    /// >>> assert row.violation() == 0.0
    /// >>> assert row.is_feasible(atol=solution.feasibility_atol)
    /// >>> row.dual_variable = 9.0
    /// >>> assert rows[capacity.constraint_id].dual_variable is None
    /// >>> solution.set_dual_variable(capacity.constraint_id, 2.0)
    /// >>> assert rows[capacity.constraint_id].dual_variable == 2.0
    /// >>> assert row.dual_variable == 9.0
    ///
    /// {class}`~ommx.EvaluatedConstraintCollection` specifies membership,
    /// lookup, iteration, and equality. Its ``of_kind`` method returns a
    /// {class}`~ommx.ConstraintFamilyView` for one family without collecting
    /// rows. Use {meth}`~ommx.Solution.constraints_df` for a materialized table.
    #[getter]
    pub fn constraints(
        slf: Bound<'_, Self>,
    ) -> crate::constraint_collection::EvaluatedConstraintCollection {
        crate::constraint_collection::EvaluatedConstraintCollection::new(slf.unbind())
    }

    /// Get evaluated named functions as a list sorted by ID
    #[getter]
    pub fn named_functions(&self) -> Vec<crate::EvaluatedNamedFunction> {
        let labels = self.inner.named_function_labels();
        self.inner
            .evaluated_named_functions()
            .iter()
            .map(|(id, nf)| crate::EvaluatedNamedFunction(*id, nf.clone(), labels.collect_for(*id)))
            .collect()
    }

    #[getter]
    pub fn decision_variable_ids(&self) -> BTreeSet<u64> {
        self.inner
            .decision_variable_ids()
            .iter()
            .map(|id| id.into_inner())
            .collect()
    }

    /// Get all unique decision variable names in this solution.
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
    /// >>> solution = instance.evaluate({i: 1 for i in range(5)})
    /// >>> sorted(solution.decision_variable_names)
    /// ['x', 'y']
    #[getter]
    pub fn decision_variable_names(&self) -> BTreeSet<String> {
        self.inner.decision_variable_names()
    }

    #[getter]
    pub fn named_function_ids(&self) -> BTreeSet<u64> {
        self.inner
            .named_function_ids()
            .iter()
            .map(|id| id.into_inner())
            .collect()
    }

    /// Get all unique named function names in this solution
    #[getter]
    pub fn named_function_names(&self) -> BTreeSet<String> {
        self.inner.named_function_names()
    }

    /// Extract the values of decision variables based on the `name` with `subscripts` key.
    ///
    /// Raises KeyError if no decision variable has the requested name, and
    /// ValueError if the same subscript is found more than once.
    ///
    /// # Examples
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints={0: sum(x) == 1},
    /// ...     sense=Sense.Maximize,
    /// ... )
    /// >>> solution = instance.evaluate({i: 1 for i in range(3)})
    /// >>> solution.extract_decision_variables("x")
    /// {(0,): 1.0, (1,): 1.0, (2,): 1.0}
    pub fn extract_decision_variables<'py>(
        &self,
        py: Python<'py>,
        name: &str,
    ) -> OmmxPyResult<Bound<'py, PyDict>> {
        let extracted = self.inner.extract_decision_variables(name)?;
        let dict = PyDict::new(py);
        for (subscripts, value) in extracted {
            let key_tuple = PyTuple::new(py, &subscripts)?;
            dict.set_item(key_tuple, value)?;
        }
        Ok(dict)
    }

    /// Extract all decision variables grouped by name.
    ///
    /// Returns a mapping from variable name to a mapping from subscripts to values.
    /// This is useful for extracting all variables at once in a structured format.
    /// Variables without names are not included in the result.
    ///
    /// Raises ValueError if the same name and subscript combination is found
    /// multiple times.
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
    /// >>> solution = instance.evaluate({i: 1 for i in range(5)})
    /// >>> all_vars = solution.extract_all_decision_variables()
    /// >>> all_vars["x"]
    /// {(0,): 1.0, (1,): 1.0, (2,): 1.0}
    /// >>> all_vars["y"]
    /// {(0,): 1.0, (1,): 1.0}
    pub fn extract_all_decision_variables<'py>(
        &self,
        py: Python<'py>,
    ) -> OmmxPyResult<Bound<'py, PyDict>> {
        let extracted = self.inner.extract_all_decision_variables()?;
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

    /// Extract nonnegative violations of all constraint families by name, keyed by subscript tuples.
    ///
    /// Discover available names with {attr}`~ommx.Solution.constraint_names`.
    /// Matching uses the name and subscripts only; a scalar constraint has the
    /// key ``()``. The returned dictionary captures values at extraction time.
    ///
    /// Raises KeyError if no constraint has the requested name. Raises
    /// ValueError if matching constraints share subscripts, including across families.
    /// Parameters and descriptions are ignored. Retained removed constraints are
    /// included. Regular constraints return violations, not signed function values.
    /// Indicator violations account for activation, while OneHot and SOS1 use
    /// their respective metrics; see {meth}`~ommx.Solution.total_violation`.
    /// Use {meth}`~ommx.Solution.get_constraint_value` or the regular row's
    /// ``evaluated_value`` when the signed value is needed.
    ///
    /// A transformation may copy a source constraint's labels onto generated
    /// constraints. If both survive in the result with the same name and
    /// subscripts, distinguish them through typed-ID lookup in
    /// {attr}`~ommx.Solution.constraints` instead of name extraction.
    ///
    /// Related methods on {meth}`~ommx.Instance.extract_constraints` and
    /// {meth}`~ommx.ParametricInstance.extract_constraints` return attached
    /// handles for active constraints; {meth}`~ommx.SampleSet.extract_constraints`
    /// returns violations for a selected sample, including removed constraints.
    ///
    /// # Examples
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> c0 = (x[0] + x[1] == 1).set_name("c").add_subscripts([0])
    /// >>> c1 = (x[1] + x[2] == 1).set_name("c").add_subscripts([1])
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints={0: c0, 1: c1},
    /// ...     sense=Sense.Maximize,
    /// ... )
    /// >>> solution = instance.evaluate({0: 1, 1: 0, 2: 1})
    /// >>> solution.extract_constraints("c")
    /// {(0,): 0.0, (1,): 0.0}
    ///
    /// A feasible inequality can have a negative signed value and zero violation:
    ///
    /// >>> from ommx import Instance
    /// >>> named = Instance.minimize()
    /// >>> x = named.new_binary("x")
    /// >>> capacity = named.add_constraint(x <= 2, "capacity", subscripts=[0])
    /// >>> assert named.constraint_names == {"capacity"}
    /// >>> assert named.extract_constraints("capacity")[(0,)] == capacity
    /// >>> assert named.as_parametric_instance().extract_constraints("capacity")[(0,)].name == "capacity"
    /// >>> solution = named.evaluate({x.id: 0})
    /// >>> assert solution.extract_constraints("capacity") == {(0,): 0.0}
    /// >>> assert solution.get_constraint_value(capacity.constraint_id) == -2.0
    /// >>> samples = named.evaluate_samples({7: {x.id: 0}})
    /// >>> assert samples.extract_constraints("capacity", 7) == {(0,): 0.0}
    fn extract_constraints(
        &self,
        name: &str,
    ) -> OmmxPyResult<crate::subscript_map::SubscriptMap<f64>> {
        Ok(crate::subscript_map::SubscriptMap(
            self.inner.extract_constraints(name)?,
        ))
    }

    /// Extract named functions by name with subscripts as key (returns a Python dict).
    ///
    /// Named functions report additional metrics for the evaluated candidate;
    /// registering one does not change the objective or add a constraint.
    /// Discover names with {attr}`~ommx.Solution.named_function_names`, or use
    /// {meth}`~ommx.Solution.named_functions_df` for a table including labels.
    /// A scalar metric uses the empty tuple ``()`` as its key.
    ///
    /// Raises KeyError if no named function has the requested name, and
    /// ValueError if the same subscript is found more than once.
    ///
    /// >>> from ommx import DecisionVariable, Instance, NamedFunction, Sense
    /// >>> x = DecisionVariable.binary(0, name="selected")
    /// >>> cost = NamedFunction(id=10, function=4 * x, name="cost")
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x], objective=x, constraints={},
    /// ...     sense=Sense.Maximize, named_functions=[cost],
    /// ... )
    /// >>> solution = instance.evaluate({x.id: 1})
    /// >>> assert solution.objective == 1.0
    /// >>> assert solution.extract_named_functions("cost") == {(): 4.0}
    /// >>> assert solution.named_functions_df().loc[cost.id, "value"] == 4.0
    pub fn extract_named_functions<'py>(
        &self,
        py: Python<'py>,
        name: &str,
    ) -> OmmxPyResult<Bound<'py, PyDict>> {
        let extracted = self.inner.extract_named_functions(name)?;
        let dict = PyDict::new(py);
        for (subscripts, value) in extracted {
            let key_tuple = PyTuple::new(py, &subscripts)?;
            dict.set_item(key_tuple, value)?;
        }
        Ok(dict)
    }

    /// Extract all named functions grouped by name (returns a Python dict).
    ///
    /// Raises ValueError if the same name and subscript combination is found
    /// multiple times.
    pub fn extract_all_named_functions<'py>(
        &self,
        py: Python<'py>,
    ) -> OmmxPyResult<Bound<'py, PyDict>> {
        let extracted = self.inner.extract_all_named_functions()?;
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

    /// Set the dual variable value for a specific constraint by ID.
    ///
    /// Raises KeyError if the constraint ID does not exist.
    pub(crate) fn set_dual_variable(
        &mut self,
        constraint_id: ConstraintIDInput,
        value: Option<f64>,
    ) -> OmmxPyResult<()> {
        let constraint_id = ommx::ConstraintID::from(constraint_id);
        self.inner.set_dual_variable(constraint_id, value)?;
        Ok(())
    }

    /// Get a specific evaluated decision variable by ID
    pub fn get_decision_variable_by_id(
        &self,
        variable_id: u64,
    ) -> PyResult<crate::EvaluatedDecisionVariable> {
        let var_id = ommx::VariableID::from(variable_id);
        let labels = self.inner.variable_labels();
        self.inner
            .decision_variables()
            .get(&var_id)
            .map(|dv| {
                crate::EvaluatedDecisionVariable::from_parts(
                    var_id,
                    dv.clone(),
                    labels.collect_for(var_id),
                )
            })
            .ok_or_else(|| {
                PyKeyError::new_err(format!("Unknown decision variable ID: {variable_id}"))
            })
    }

    /// Get the evaluated value of a specific constraint by ID
    pub(crate) fn get_constraint_value(
        &self,
        constraint_id: ConstraintIDInput,
    ) -> OmmxPyResult<f64> {
        let constraint_id = ommx::ConstraintID::from(constraint_id);
        Ok(self.inner.get_constraint_value(constraint_id)?)
    }

    /// Get the dual variable value for a specific constraint by ID
    pub(crate) fn get_dual_variable(
        &self,
        constraint_id: ConstraintIDInput,
    ) -> OmmxPyResult<Option<f64>> {
        let constraint_id = ommx::ConstraintID::from(constraint_id);
        Ok(self.inner.get_dual_variable(constraint_id)?)
    }

    /// Get a specific evaluated named function by ID
    pub fn get_named_function_by_id(
        &self,
        named_function_id: u64,
    ) -> PyResult<crate::EvaluatedNamedFunction> {
        let named_function_id = ommx::NamedFunctionID::from(named_function_id);
        self.inner
            .evaluated_named_functions()
            .get(&named_function_id)
            .map(|nf| {
                crate::EvaluatedNamedFunction(
                    named_function_id,
                    nf.clone(),
                    self.inner
                        .named_function_labels()
                        .collect_for(named_function_id),
                )
            })
            .ok_or_else(|| {
                PyKeyError::new_err(format!(
                    "Unknown named function ID: {}",
                    named_function_id.into_inner()
                ))
            })
    }

    /// A materialized DataFrame of variable definitions, labels, and evaluated values.
    ///
    /// The integer ``id`` index identifies variables. Columns include ``kind``,
    /// ``lower``, ``upper``, and ``value``, plus modeling labels such as ``name``,
    /// ``subscripts``, ``description``, and ``parameters.{key}``. Evaluated rows
    /// have no ``substituted_value`` field, so that compatibility column is NA;
    /// read the populated value from ``value``. ``include`` selects optional
    /// label/sidecar columns as on {meth}`~ommx.Instance.decision_variables_df`.
    /// Editing this table does not update the Solution.
    ///
    /// >>> from ommx import Instance
    /// >>> instance = Instance.maximize()
    /// >>> x = instance.new_binary("selected", subscripts=[0])
    /// >>> instance.objective = x
    /// >>> solution = instance.evaluate({x.id: 1})
    /// >>> table = solution.decision_variables_df()
    /// >>> assert table.loc[x.id, "name"] == "selected"
    /// >>> assert table.loc[x.id, "value"] == 1.0
    /// >>> table.loc[x.id, "value"] = 0.0
    /// >>> assert solution.state.entries[x.id] == 1.0
    #[pyo3(signature = (include = None))]
    pub fn decision_variables_df<'py>(
        &self,
        py: Python<'py>,
        include: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let flags = crate::pandas::IncludeFlags::from_optional(include)?;
        let var_meta_store = self.inner.variable_labels().clone();
        let view: Vec<(
            ommx::DecisionVariableLabel,
            ommx::VariableID,
            &ommx::EvaluatedDecisionVariable,
        )> = self
            .inner
            .decision_variables()
            .iter()
            .map(|(id, dv)| (var_meta_store.collect_for(*id), *id, dv))
            .collect();
        entries_to_dataframe(
            py,
            view.iter()
                .map(|(m, id, dv)| crate::pandas::WithModelingContext::new((*id, *dv), m)),
            "id",
            flags,
        )
    }

    /// DataFrame of evaluated constraints, including `feasible` and `violation` for every kind.
    ///
    /// ``kind`` is required: ``"regular"``, ``"indicator"``, ``"one_hot"``, or
    /// ``"sos1"``. The index contains typed IDs of the selected family. Rows
    /// retain their labels and evaluation diagnostics; editing this materialized
    /// table does not update the Solution.
    ///
    /// For regular constraints, ``value`` is the signed left-hand-side value:
    /// equality requires zero and inequality requires a nonpositive value,
    /// within {attr}`~ommx.Solution.feasibility_atol`. The nonnegative
    /// ``violation`` column and per-row ``feasible`` are therefore distinct
    /// from ``value``. Per-constraint feasibility does not check variable domains.
    /// The `violation` column uses {meth}`constraint_violation`. See
    /// {meth}`ommx.Instance.constraints_df` for column / `kind=` /
    /// `include=` semantics.
    ///
    /// `Solution` has no `removed=` parameter (no active/removed
    /// distinction at the evaluated stage); reason data is gated by
    /// `"removed_reason"` in `include=`. When the flag is on, rows for
    /// constraints removed before evaluation get `removed_reason` /
    /// `removed_reason.{key}` columns populated; other rows have NA.
    /// The related metadata queries {meth}`~ommx.Solution.constraint_context_df`,
    /// {meth}`~ommx.Solution.constraint_parameters_df`,
    /// {meth}`~ommx.Solution.constraint_provenance_df`, and
    /// {meth}`~ommx.Solution.constraint_removed_reasons_df` also require ``kind``.
    ///
    /// >>> from ommx import Instance
    /// >>> instance = Instance.minimize()
    /// >>> x = instance.new_binary("x")
    /// >>> capacity = instance.add_constraint(x <= 2, "capacity")
    /// >>> solution = instance.evaluate({x.id: 0})
    /// >>> table = solution.constraints_df(kind="regular")
    /// >>> assert table.loc[capacity.constraint_id, "value"] == -2.0
    /// >>> assert table.loc[capacity.constraint_id, "violation"] == 0.0
    /// >>> assert table.loc[capacity.constraint_id, "feasible"]
    #[pyo3(signature = (*, kind, include = None))]
    pub fn constraints_df<'py>(
        &self,
        py: Python<'py>,
        kind: ConstraintKind,
        include: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let id_col = constraint_id_col(kind);
        let flags = crate::pandas::IncludeFlags::from_optional(include)?;
        constraint_kind_collection!(
            self.inner,
            kind,
            [
                evaluated_constraints,
                evaluated_indicator_constraints,
                evaluated_one_hot_constraints,
                evaluated_sos1_constraints
            ],
            |coll| {
                let meta = coll.context().clone();
                let removed_reasons = coll.removed_reasons();
                let mut entries: Vec<Bound<'py, pyo3::types::PyAny>> = Vec::new();
                for (id, c) in coll.inner().iter() {
                    let m = meta.collect_for(*id);
                    let dict = crate::pandas::WithModelingContext::new(
                        (*id, c, self.inner.feasibility_atol()),
                        &m,
                    )
                    .to_pandas_entry(py)?;
                    dict.set_item("violation", self.constraint_violation((*id).into())?)?;
                    if flags.removed_reason {
                        // Always emit the `removed_reason` column when
                        // requested so its existence in the resulting
                        // DataFrame is a function of the flag, not of
                        // whether any row happens to carry a reason.
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

    /// DataFrame of evaluated named functions
    ///
    /// Columns: id (index), value, used_ids, name, subscripts, description, parameters.{key}
    #[pyo3(signature = (include = None))]
    pub fn named_functions_df<'py>(
        &self,
        py: Python<'py>,
        include: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let flags = crate::pandas::IncludeFlags::from_optional(include)?;
        let nf_meta_store = self.inner.named_function_labels().clone();
        let nf_meta_view: Vec<(
            ommx::NamedFunctionLabel,
            ommx::NamedFunctionID,
            &ommx::EvaluatedNamedFunction,
        )> = self
            .inner
            .evaluated_named_functions()
            .iter()
            .map(|(id, nf)| (nf_meta_store.collect_for(*id), *id, nf))
            .collect();
        entries_to_dataframe(
            py,
            nf_meta_view
                .iter()
                .map(|(m, id, nf)| crate::pandas::WithModelingContext::new((*id, *nf), m)),
            "id",
            flags,
        )
    }

    /// Constraint context DataFrame (id-indexed). See
    /// {meth}`ommx.Instance.constraint_context_df` for column / `kind=`
    /// semantics. Reads from the evaluated collection's context store.
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
                evaluated_constraints,
                evaluated_indicator_constraints,
                evaluated_one_hot_constraints,
                evaluated_sos1_constraints
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
                evaluated_constraints,
                evaluated_indicator_constraints,
                evaluated_one_hot_constraints,
                evaluated_sos1_constraints
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
                evaluated_constraints,
                evaluated_indicator_constraints,
                evaluated_one_hot_constraints,
                evaluated_sos1_constraints
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
                evaluated_constraints,
                evaluated_indicator_constraints,
                evaluated_one_hot_constraints,
                evaluated_sos1_constraints
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

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }

    /// Sum the nonnegative scalar violation of every constraint, including removed constraints.
    ///
    /// - Equality: `abs(f(x))`; inequality: `max(0, f(x))`.
    /// - Indicator: the inner violation when active, otherwise zero.
    /// - OneHot: `min_i (abs(x_i - 1) + sum_{j != i} abs(x_j))`.
    /// - SOS1: `min_i sum_{j != i} abs(x_j)`.
    ///
    /// Each constraint is feasible exactly when its violation is at most the
    /// evaluation tolerance. This threshold applies to each constraint separately,
    /// not to the total. Zero therefore implies that all constraints are feasible.
    /// Variable bound and kind violations
    /// are not added. Values use the evaluated state after discrete-value
    /// canonicalization. Lowering need not preserve the metric: a retained
    /// original and its generated constraints each contribute.
    ///
    /// Use {meth}`constraint_violation` for individual values, also available in
    /// the `violation` column of {meth}`constraints_df` for every constraint kind.
    pub fn total_violation(&self) -> f64 {
        self.inner.total_violation()
    }

    /// Get one constraint's nonnegative scalar violation.
    ///
    /// Uses the definitions in {meth}`total_violation`, including for removed
    /// constraints. The ID type selects its family. Raises `KeyError` when
    /// the ID is absent from that constraint family.
    pub fn constraint_violation(
        &self,
        constraint_id: crate::constraint_id::AnyConstraintID,
    ) -> PyResult<f64> {
        use crate::constraint_id::AnyConstraintID;
        let violation = match constraint_id {
            AnyConstraintID::Regular(id) => self.inner.constraint_violation(id.into()),
            AnyConstraintID::Indicator(id) => self.inner.indicator_constraint_violation(id.into()),
            AnyConstraintID::OneHot(id) => self.inner.one_hot_constraint_violation(id.into()),
            AnyConstraintID::Sos1(id) => self.inner.sos1_constraint_violation(id.into()),
        };
        violation
            .ok_or_else(|| PyKeyError::new_err(format!("Unknown constraint ID: {constraint_id:?}")))
    }
}
