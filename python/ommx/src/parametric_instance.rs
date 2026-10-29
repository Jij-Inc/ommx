use crate::constraint_id::{
    ConstraintIDInput, ConstraintMapInput, IndicatorConstraintIDInput, OneHotConstraintIDInput,
    Sos1ConstraintIDInput,
};
use crate::{
    constraint_kind::{constraint_kind_collection, ConstraintKind},
    error::OmmxPyResult,
    pandas::{constraint_id_col, entries_to_dataframe, PyDataFrame, ToPandasEntry},
    Constraint, DecisionVariable, Function, Instance, NamedFunction, OutputObjective, Parameter,
    Sense,
};
use ommx::{ConstraintID, NamedFunctionID, VariableID};
use pyo3::{
    exceptions::{PyKeyError, PyValueError},
    prelude::*,
    types::PyBytes,
    Bound, PyAny,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// A family of optimization problems with fixed variables and constraints.
///
/// A ParametricInstance leaves some coefficients or constants symbolic as
/// {class}`~ommx.Parameter` objects. The model's caller supplies their values
/// with {meth}`~ommx.ParametricInstance.with_parameters` to obtain an ordinary
/// {class}`~ommx.Instance` for evaluation, preparation, or a solver adapter.
/// Parameters are inputs chosen before solving; they are not decision variables
/// for the solver to optimize.
///
/// **Reuse a problem under different numerical conditions**
///
/// Here the two items and their scores stay fixed, while the allowed number
/// of selected items changes. The original family remains available after
/// each call to ``with_parameters``.
///
/// >>> from ommx import ConstraintID, DecisionVariable, Parameter, ParametricInstance, Sense
/// >>> xs = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(2)]
/// >>> capacity = Parameter(2, name="capacity", description="Maximum selected items")
/// >>> family = ParametricInstance.from_components(
/// ...     decision_variables=xs, parameters=[capacity],
/// ...     objective=3 * xs[0] + 2 * xs[1],
/// ...     constraints={0: xs[0] + xs[1] <= capacity}, sense=Sense.Maximize,
/// ... )
/// >>> one_item = family.with_parameters({capacity.id: 1})
/// >>> two_items = family.with_parameters({capacity.id: 2})
/// >>> candidate = {xs[0].id: 1, xs[1].id: 1}
/// >>> assert not one_item.evaluate(candidate).feasible
/// >>> assert two_items.evaluate(candidate).feasible
/// >>> assert two_items.evaluate(candidate).objective == 5.0
/// >>> assert {v.id for v in one_item.decision_variables} == {v.id for v in two_items.decision_variables}
/// >>> assert set(one_item.constraints) == set(two_items.constraints) == {ConstraintID(0)}
/// >>> assert family.parameter_ids == {capacity.id}
///
/// Materialization preserves decision-variable and constraint IDs. Those IDs
/// let callers associate results with the same items and conditions across
/// instances; they do not imply identical feasibility or optimal solutions.
/// Parameters can also appear as objective and constraint coefficients, as in
/// the weighted knapsack example in {meth}`~ommx.ParametricInstance.from_components`.
///
/// **Parameter identities and modeling labels**
///
/// Parameters and decision variables use one ID space and must not overlap.
/// Here the decision variables have IDs 0 and 1, and the parameter has ID 2.
/// Both can appear in arithmetic expressions, but only decision variables
/// receive candidate values after materialization. Parameter names, subscripts,
/// string-valued metadata, and descriptions describe model inputs; the IDs
/// identify the numerical values passed to ``with_parameters``.
///
/// {attr}`~ommx.ParametricInstance.parameters` and
/// {attr}`~ommx.ParametricInstance.parameter_ids` list declared parameters.
/// {meth}`~ommx.ParametricInstance.parameters_df` collects their definitions
/// and labels into a DataFrame, not a table of supplied numerical values.
///
/// >>> table = family.parameters_df()
/// >>> assert table.loc[capacity.id, "name"] == "capacity"
/// >>> assert table.loc[capacity.id, "description"] == "Maximum selected items"
///
/// {meth}`~ommx.ParametricInstance.decision_variables_df` and
/// {meth}`~ommx.ParametricInstance.constraints_df` provide the corresponding
/// model tables; the latter requires an explicit ``kind``. For required IDs,
/// extra keys, and numerical substitution rules, read
/// {meth}`~ommx.ParametricInstance.with_parameters`.
///
/// **When to rebuild the model**
///
/// Parameter values do not generate variable or constraint definitions. If an
/// input changes the number of items or the model's structure, build new
/// components in Python or use a modeler that generates that structure. This
/// differs from changing numerical coefficients within an existing family.
/// No Experiment is needed to construct or materialize a ParametricInstance.
/// Use {class}`~ommx.experiment.Experiment` separately when recording the
/// conditions and results of multiple trials is useful.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct ParametricInstance {
    pub(crate) inner: ommx::ParametricInstance,
}

impl_instance_annotations!(ParametricInstance);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl ParametricInstance {
    /// Unique decision-variable names as a set of strings, excluding parameters.
    /// Unnamed variables are excluded; explicitly empty names are included.
    /// Each access returns a fresh set; editing it does not change this owner.
    #[getter]
    fn decision_variable_names(&self) -> BTreeSet<String> {
        self.inner.decision_variable_names()
    }

    /// Unique names across all constraint families, as a set of strings.
    ///
    /// Only active constraints are included.
    /// The scope matches `extract_constraints`. Unnamed constraints are excluded;
    /// an explicitly empty name is included. Duplicate names appear once, even
    /// when extraction would reject duplicate subscripts.
    /// Each access returns a fresh set; editing it does not change this owner.
    #[getter]
    fn constraint_names(&self) -> BTreeSet<String> {
        self.inner.constraint_names()
    }

    /// Deserialize a parametric instance from v1 protobuf bytes.
    ///
    /// Raises {class}`ValueError` if the protobuf payload is malformed or
    /// semantically invalid.
    #[staticmethod]
    pub fn from_v1_bytes(bytes: &Bound<PyBytes>) -> OmmxPyResult<Self> {
        let _guard = crate::TRACING.attach_parent_context(bytes.py());
        Ok(Self {
            inner: ommx::ParametricInstance::from_v1_bytes(bytes.as_bytes())?,
        })
    }

    /// Deserialize a parametric instance from v2 protobuf bytes.
    ///
    /// Raises {class}`ValueError` if the protobuf payload is malformed or
    /// semantically invalid.
    #[staticmethod]
    pub fn from_v2_bytes(bytes: &Bound<PyBytes>) -> OmmxPyResult<Self> {
        let _guard = crate::TRACING.attach_parent_context(bytes.py());
        Ok(Self {
            inner: ommx::ParametricInstance::from_v2_bytes(bytes.as_bytes())?,
        })
    }

    /// Serialize this parametric instance in the OMMX v1 wire format.
    ///
    /// # Errors
    ///
    /// Serialization raises ``RuntimeError`` whenever an output objective is present because v1 cannot represent it.
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x], objective=x, constraints={}, sense=Sense.Maximize
    /// ... )
    /// >>> assert instance.convert_active_objective(Sense.Minimize)
    /// >>> assert instance.convert_active_objective(Sense.Maximize)
    /// >>> parametric = instance.as_parametric_instance()
    /// >>> try:
    /// ...     parametric.to_v1_bytes()
    /// ... except RuntimeError:
    /// ...     pass
    /// ... else:
    /// ...     raise AssertionError("v1 serialization accepted an output objective")
    pub fn to_v1_bytes<'py>(&self, py: Python<'py>) -> OmmxPyResult<Bound<'py, PyBytes>> {
        let _guard = crate::TRACING.attach_parent_context(py);
        Ok(PyBytes::new(py, &self.inner.to_v1_bytes()?))
    }

    /// Serialize this parametric instance in the OMMX v2 wire format.
    ///
    /// # Postconditions
    ///
    /// A v2 round-trip preserves both active and output objective semantics through materialization.
    ///
    /// >>> from ommx import DecisionVariable, Instance, ParametricInstance, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> source = Instance.from_components(
    /// ...     decision_variables=[x], objective=x, constraints={7: x == 1}, sense=Sense.Minimize
    /// ... )
    /// >>> parametric = source.uniform_penalty_method()
    /// >>> parameter_id = parametric.parameters[0].id
    /// >>> restored = ParametricInstance.from_v2_bytes(parametric.to_v2_bytes())
    /// >>> materialized = restored.with_parameters({parameter_id: 2.0})
    /// >>> assert materialized.sense == Sense.Minimize
    /// >>> assert materialized.objective.evaluate({0: 0}) == 2.0
    /// >>> solution = materialized.evaluate({0: 0})
    /// >>> assert (solution.sense, solution.objective) == (Sense.Minimize, 0.0)
    pub fn to_v2_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        let _guard = crate::TRACING.attach_parent_context(py);
        PyBytes::new(py, &self.inner.to_v2_bytes())
    }

    pub fn __str__(&self) -> String {
        self.inner.format_summary()
    }

    pub fn __repr__(&self) -> String {
        self.inner.format_summary()
    }

    /// Build a parametric instance from its model components.
    ///
    /// Constraint mappings accept integer keys or IDs of the matching family.
    /// Integers are promoted to that family's ID. IDs of another family raise
    /// {class}`TypeError`; keys that become duplicates raise {class}`ValueError`.
    /// Returned collections always use typed IDs.
    ///
    /// Raises {class}`ValueError` if a decision variable and parameter use the
    /// same ID.
    ///
    /// **Parameterize objective coefficients, weights, and a capacity**
    ///
    /// This two-item knapsack family fixes the item count and uses parameters
    /// for both coefficients and the right-hand-side constant. Parameter IDs
    /// are separate from the decision-variable IDs in the shared ID space.
    ///
    /// >>> from ommx import DecisionVariable, Parameter, ParametricInstance, Sense
    /// >>> n = 2
    /// >>> xs = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(n)]
    /// >>> profits = [Parameter(n + i, name="profit", subscripts=[i]) for i in range(n)]
    /// >>> weights = [Parameter(2 * n + i, name="weight", subscripts=[i]) for i in range(n)]
    /// >>> capacity = Parameter(3 * n, name="capacity")
    /// >>> family = ParametricInstance.from_components(
    /// ...     decision_variables=xs, parameters=profits + weights + [capacity],
    /// ...     objective=sum(p * x for p, x in zip(profits, xs)),
    /// ...     constraints={0: sum(w * x for w, x in zip(weights, xs)) <= capacity},
    /// ...     sense=Sense.Maximize,
    /// ... )
    /// >>> values = {p.id: value for p, value in zip(profits, [3, 2])}
    /// >>> values.update({w.id: value for w, value in zip(weights, [2, 1])})
    /// >>> values[capacity.id] = 2
    /// >>> instance = family.with_parameters(values)
    /// >>> solution = instance.evaluate({xs[0].id: 1, xs[1].id: 0})
    /// >>> assert solution.feasible and solution.objective == 3.0
    /// >>> assert not instance.evaluate({xs[0].id: 1, xs[1].id: 1}).feasible
    #[staticmethod]
    #[pyo3(signature = (*, sense, objective, decision_variables, constraints, parameters, indicator_constraints=None, one_hot_constraints=None, sos1_constraints=None, named_functions=None, description=None))]
    pub(crate) fn from_components(
        sense: Sense,
        objective: Function,
        decision_variables: Vec<DecisionVariable>,
        constraints: ConstraintMapInput<ConstraintIDInput, Constraint>,
        parameters: Vec<Parameter>,
        indicator_constraints: Option<
            ConstraintMapInput<IndicatorConstraintIDInput, crate::IndicatorConstraint>,
        >,
        one_hot_constraints: Option<
            ConstraintMapInput<OneHotConstraintIDInput, crate::OneHotConstraint>,
        >,
        sos1_constraints: Option<ConstraintMapInput<Sos1ConstraintIDInput, crate::Sos1Constraint>>,
        named_functions: Option<Vec<NamedFunction>>,
        description: Option<crate::InstanceDescription>,
    ) -> OmmxPyResult<Self> {
        let mut rust_decision_variables = BTreeMap::new();
        let mut variable_labels = ommx::VariableLabelStore::default();
        for var in decision_variables {
            let id = var.0;
            variable_labels.insert(id, var.2);
            if rust_decision_variables.insert(id, var.1).is_some() {
                return Err(PyValueError::new_err(format!(
                    "Duplicate decision variable ID: {}",
                    id.into_inner()
                ))
                .into());
            }
        }

        let mut constraint_context = ommx::ConstraintContextStore::default();
        let rust_constraints: BTreeMap<ConstraintID, ommx::Constraint> = constraints
            .into_iter()
            .map(|(id, c)| {
                let cid = ConstraintID::from(id);
                constraint_context.insert(cid, c.1);
                (cid, c.0)
            })
            .collect();

        let rust_parameters =
            ommx::ParameterTable::from_v1_parameters(parameters.into_iter().map(|p| p.0))?;

        let mut builder = ommx::ParametricInstance::builder()
            .sense(sense.into())
            .objective(objective.0)
            .decision_variables(rust_decision_variables)
            .variable_labels(variable_labels)
            .constraints(rust_constraints)
            .parameters(rust_parameters);

        let mut indicator_context_pairs: Vec<(
            ommx::IndicatorConstraintID,
            ommx::ConstraintContext,
        )> = Vec::new();
        if let Some(ics) = indicator_constraints {
            let rust_indicator_constraints: BTreeMap<
                ommx::IndicatorConstraintID,
                ommx::IndicatorConstraint,
            > = ics
                .into_iter()
                .map(|(id, ic)| {
                    let iid = ommx::IndicatorConstraintID::from(id);
                    indicator_context_pairs.push((iid, ic.1));
                    (iid, ic.0)
                })
                .collect();
            builder = builder.indicator_constraints(rust_indicator_constraints);
        }

        let mut one_hot_context = ommx::ConstraintContextStore::default();
        if let Some(ohs) = one_hot_constraints {
            let rust_one_hot_constraints: BTreeMap<
                ommx::OneHotConstraintID,
                ommx::OneHotConstraint,
            > = ohs
                .into_iter()
                .map(|(id, oh)| {
                    let oid = ommx::OneHotConstraintID::from(id);
                    one_hot_context.insert(oid, oh.1);
                    (oid, oh.0)
                })
                .collect();
            builder = builder.one_hot_constraints(rust_one_hot_constraints);
        }

        let mut sos1_context = ommx::ConstraintContextStore::default();
        if let Some(s1s) = sos1_constraints {
            let rust_sos1_constraints: BTreeMap<ommx::Sos1ConstraintID, ommx::Sos1Constraint> = s1s
                .into_iter()
                .map(|(id, s1)| {
                    let sid = ommx::Sos1ConstraintID::from(id);
                    sos1_context.insert(sid, s1.1);
                    (sid, s1.0)
                })
                .collect();
            builder = builder.sos1_constraints(rust_sos1_constraints);
        }

        let mut named_function_labels = ommx::NamedFunctionLabelStore::default();
        if let Some(nfs) = named_functions {
            let mut rust_named_functions = BTreeMap::new();
            for nf in nfs {
                let id = nf.0;
                named_function_labels.insert(id, nf.2);
                if rust_named_functions.insert(id, nf.1).is_some() {
                    return Err(PyValueError::new_err(format!(
                        "Duplicate named function ID: {}",
                        id.into_inner()
                    ))
                    .into());
                }
            }
            builder = builder.named_functions(rust_named_functions);
        }

        if let Some(desc) = description {
            builder = builder.description(desc.0);
        }

        let mut indicator_context = ommx::ConstraintContextStore::default();
        for (id, context) in indicator_context_pairs {
            indicator_context.insert(id, context);
        }
        let inner = builder
            .constraint_context(constraint_context)
            .indicator_constraint_context(indicator_context)
            .one_hot_constraint_context(one_hot_context)
            .sos1_constraint_context(sos1_context)
            .named_function_labels(named_function_labels)
            .build()?;

        Ok(Self { inner })
    }

    /// Create trivial empty instance of minimization with zero objective, no constraints, and no decision variables and parameters.
    #[staticmethod]
    pub fn empty() -> OmmxPyResult<Self> {
        Self::from_components(
            Sense::Minimize,
            Function(ommx::Function::Zero),
            Vec::new(),
            Default::default(),
            Vec::new(),
            None,
            None,
            None,
            None,
            None,
        )
    }

    /// Substitute parameters to yield an instance.
    ///
    /// Provide a dict mapping every declared parameter ID to its numerical value,
    /// including parameters not referenced by an expression. The original
    /// {class}`~ommx.ParametricInstance` is unchanged and can be materialized again.
    /// Missing declared IDs raise ``RuntimeError``.
    ///
    /// The method checks that declared IDs are included, not that the key sets are
    /// equal. Additional keys are accepted and participate in substitution wherever
    /// referenced. In particular, an extra decision-variable ID can also substitute
    /// that coordinate in expressions. For parameter-only materialization, supply
    /// exactly the IDs listed by {attr}`~ommx.ParametricInstance.parameters`.
    ///
    /// Substitution uses the SDK's default absolute tolerance and the expression
    /// partial-evaluation rules; see {meth}`~ommx.Function.partial_evaluate`.
    /// This is not a feasibility check or decision-variable kind/bound validation.
    /// There is no blanket finiteness check of all supplied values: a referenced
    /// non-finite value can fail during substitution, while an unreferenced value
    /// need not be examined. Successful materialization does not prove that all
    /// supplied values are finite or that the resulting problem is feasible.
    ///
    /// >>> from ommx import DecisionVariable, Parameter, ParametricInstance, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> p, unused = Parameter(1, name="p"), Parameter(2, name="unused")
    /// >>> family = ParametricInstance.from_components(
    /// ...     decision_variables=[x], parameters=[p, unused], objective=x + p,
    /// ...     constraints={}, sense=Sense.Minimize
    /// ... )
    /// >>> try:
    /// ...     family.with_parameters({p.id: 3})
    /// ... except RuntimeError as error:
    /// ...     assert "Missing parameters" in str(error)
    /// ... else:
    /// ...     raise AssertionError("an omitted declared parameter was accepted")
    /// >>> concrete = family.with_parameters({p.id: 3, unused.id: 0})
    /// >>> assert concrete.objective.evaluate({x.id: 1}) == 4.0
    ///
    /// # Postconditions
    ///
    /// Penalty conversion preserves existing output semantics, or captures the pre-penalty active objective when no output objective exists. Materialization substitutes parameters in the active energy, and an existing output objective remains explicit even if specialization makes it structurally equal to the active objective.
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> source = Instance.from_components(
    /// ...     decision_variables=[x], objective=x, constraints={7: x == 1}, sense=Sense.Minimize
    /// ... )
    /// >>> parametric = source.uniform_penalty_method()
    /// >>> parameter_id = parametric.parameters[0].id
    /// >>> materialized = parametric.with_parameters({parameter_id: 2.0})
    /// >>> assert materialized.objective.evaluate({0: 0}) == 2.0
    /// >>> solution = materialized.evaluate({0: 0})
    /// >>> assert (solution.sense, solution.objective, solution.feasible) == (Sense.Minimize, 0.0, False)
    pub fn with_parameters(&self, parameters: HashMap<u64, f64>) -> OmmxPyResult<Instance> {
        let mut v1_params = ommx::v1::Parameters::default();
        v1_params.entries = parameters;
        let instance = self.inner.clone().with_parameters(v1_params)?;
        Ok(Instance { inner: instance })
    }

    /// Format a function using this parametric instance's decision-variable and parameter labels.
    ///
    /// The plain {func}`repr` / {func}`str` representation of {class}`Function`
    /// remains context-free and renders raw ``x<id>`` symbols such as ``x1``.
    /// Use this method when labels from this parametric instance should be used instead.
    #[pyo3(signature = (function, max_terms=None, max_chars=None))]
    pub fn format_function(
        &self,
        function: Function,
        max_terms: Option<usize>,
        max_chars: Option<usize>,
    ) -> OmmxPyResult<String> {
        let opts = ommx::FunctionFormatOptions {
            max_terms,
            max_chars,
        };
        if opts == ommx::FunctionFormatOptions::default() {
            Ok(self.inner.format_function(&function.0)?)
        } else {
            Ok(self.inner.format_function_with(&function.0, opts)?.text)
        }
    }

    /// Build a bounded notebook display object for a function in this parametric instance's context.
    ///
    /// By default this returns a preview capped at 100 complete terms and
    /// 20,000 characters. Use {meth}`format_function` for an unbounded plain
    /// text string.
    #[pyo3(signature = (function, max_terms=Some(100), max_chars=Some(20000)))]
    pub fn display_function(
        &self,
        function: Function,
        max_terms: Option<usize>,
        max_chars: Option<usize>,
    ) -> OmmxPyResult<crate::display::FunctionDisplay> {
        let formatted = self.inner.format_function_with(
            &function.0,
            ommx::FunctionFormatOptions {
                max_terms,
                max_chars,
            },
        )?;
        Ok(crate::display::FunctionDisplay::new(formatted))
    }

    /// Substitute decision variables with function expressions (in-place).
    ///
    /// Replaces each given decision variable with the provided function in the
    /// objective and all active constraints. Replacement functions may still
    /// reference parameters; those parameter references remain in this
    /// parametric instance and are evaluated later by {meth}`with_parameters`.
    /// Assignment targets must be existing decision variable IDs. Replacement
    /// expressions may only reference existing decision variable or parameter
    /// IDs; parameter IDs cannot be assignment targets.
    ///
    /// **Args:**
    /// - `assignments`: A dict mapping decision variable IDs to the function
    ///   expressions that should replace them.
    ///
    /// **Important:**
    /// This method performs an algebraic rewrite. It does not automatically
    /// translate the substituted variable's bound or kind into constraints on
    /// the replacement expression. If the substitution must preserve the
    /// optimization problem, the caller must provide a domain-preserving
    /// encoding or add the required linking and bound constraints explicitly.
    ///
    /// Raises ``ValueError`` on cyclic or recursive assignments, undefined
    /// IDs, when a parameter ID is used as an assignment target, or when
    /// substituting a variable that is a member of an indicator, one-hot, or
    /// SOS1 constraint.
    ///
    /// # Postconditions
    ///
    /// Substitution rewrites active expressions while materialized output evaluation restores the substituted variable.
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> b = DecisionVariable.binary(1)
    /// >>> source = Instance.from_components(
    /// ...     decision_variables=[x, b], objective=x, constraints={}, sense=Sense.Maximize
    /// ... )
    /// >>> assert source.convert_active_objective(Sense.Minimize)
    /// >>> parametric = source.as_parametric_instance()
    /// >>> parametric.substitute({0: b})
    /// >>> materialized = parametric.with_parameters({})
    /// >>> assert materialized.required_ids() == {1}
    /// >>> assert materialized.objective.evaluate({1: 1}) == -1.0
    /// >>> solution = materialized.evaluate({1: 1})
    /// >>> assert (solution.sense, solution.objective) == (Sense.Maximize, 1.0)
    #[pyo3(signature = (assignments))]
    pub fn substitute(
        &mut self,
        py: Python<'_>,
        assignments: HashMap<u64, Function>,
    ) -> OmmxPyResult<()> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let iter = assignments
            .into_iter()
            .map(|(id, f)| (VariableID::from(id), f.0));
        ommx::substitute(&mut self.inner, iter)?;
        Ok(())
    }

    #[getter]
    pub fn sense(&self) -> Sense {
        (*self.inner.sense()).into()
    }

    #[getter]
    pub fn objective(&self) -> Function {
        Function(self.inner.objective().clone())
    }

    /// Read-only output objective retained through parameter materialization,
    /// if one has been captured.
    ///
    /// # Postconditions
    ///
    /// Conversion preserves both absence and an explicit output objective equal to the active pair.
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> source = Instance.from_components(
    /// ...     decision_variables=[x], objective=x, constraints={}, sense=Sense.Maximize
    /// ... )
    /// >>> assert source.as_parametric_instance().output_objective is None
    /// >>> assert source.convert_active_objective(Sense.Minimize)
    /// >>> assert source.convert_active_objective(Sense.Maximize)
    /// >>> parametric = source.as_parametric_instance()
    /// >>> output = parametric.output_objective
    /// >>> assert output is not None
    /// >>> assert output.sense == parametric.sense
    /// >>> assert output.function.almost_equal(parametric.objective)
    #[getter]
    pub fn output_objective(&self) -> Option<OutputObjective> {
        self.inner.output_objective().cloned().map(Into::into)
    }

    /// List of all decision variables in the parametric instance sorted by
    /// their IDs.
    ///
    /// Returns a list of {class}`~ommx.AttachedDecisionVariable` write-through
    /// handles. See {attr}`~ommx.Instance.decision_variables` for the same
    /// shape on `Instance`.
    #[getter]
    pub fn decision_variables(slf: Bound<'_, Self>) -> Vec<crate::AttachedDecisionVariable> {
        let py = slf.py();
        let ids: Vec<VariableID> = slf
            .borrow()
            .inner
            .decision_variables()
            .keys()
            .copied()
            .collect();
        let py_instance: Py<Self> = slf.unbind();
        ids.into_iter()
            .map(|id| {
                crate::AttachedDecisionVariable::from_parametric(py_instance.clone_ref(py), id)
            })
            .collect()
    }

    /// Add a decision variable to this parametric instance. Returns an
    /// {class}`~ommx.AttachedDecisionVariable` bound to the variable's
    /// id — a write-through handle for further label updates.
    ///
    /// Raises {class}`ValueError` if the variable's id collides with an
    /// existing decision variable or parameter. Substituted variables retain
    /// their ids and therefore also count as existing decision variables.
    pub fn add_decision_variable(
        slf: Bound<'_, Self>,
        variable: DecisionVariable,
    ) -> OmmxPyResult<crate::AttachedDecisionVariable> {
        let id = {
            let mut inst = slf.borrow_mut();
            inst.inner
                .add_decision_variable(variable.0, variable.1, variable.2)?
        };
        Ok(crate::AttachedDecisionVariable::from_parametric(
            slf.unbind(),
            id,
        ))
    }

    /// Look up the {class}`~ommx.AttachedDecisionVariable` for the given
    /// id — a write-through handle.
    pub fn attached_decision_variable(
        slf: Bound<'_, Self>,
        variable_id: u64,
    ) -> PyResult<crate::AttachedDecisionVariable> {
        let id = VariableID::from(variable_id);
        if !slf.borrow().inner.decision_variables().contains_key(&id) {
            return Err(PyKeyError::new_err(format!(
                "Decision variable with ID {variable_id} not found"
            )));
        }
        Ok(crate::AttachedDecisionVariable::from_parametric(
            slf.unbind(),
            id,
        ))
    }

    /// Extract active constraints of every family by name, keyed by subscript tuples.
    ///
    /// Values are attached handles, just as in `constraints`: edits update this
    /// model. The returned dictionary snapshots the matching keys; it does not
    /// track later label or membership changes. Removed constraints are excluded.
    /// Parameters and descriptions do not participate in matching.
    ///
    /// Raises KeyError if no active constraint has this name, and ValueError if
    /// matching constraints share subscripts, including across different families.
    /// Use typed-ID lookup in `constraints` to distinguish ambiguous labels.
    fn extract_constraints(
        slf: Bound<'_, Self>,
        name: &str,
    ) -> PyResult<
        crate::subscript_map::SubscriptMap<crate::constraint_collection::ConstraintCollectionValue>,
    > {
        let py = slf.py();
        Self::constraints(slf).extract_by_name(py, name)
    }

    /// Live mapping of all active constraint families, keyed by typed IDs.
    /// The view retains this owner and fetches each constraint on demand.
    #[getter]
    pub fn constraints(slf: Bound<'_, Self>) -> crate::constraint_collection::ConstraintCollection {
        crate::constraint_collection::ConstraintCollection::new(crate::ConstraintHost::Parametric(
            slf.unbind(),
        ))
    }

    /// Add a regular constraint to this parametric instance.
    ///
    /// Picks an unused {class}`~ommx.ConstraintID`, drains the wrapper's
    /// context snapshot into this parametric instance's SoA store, and
    /// returns an {class}`~ommx.AttachedConstraint` bound to the new id.
    /// The input {class}`~ommx.Constraint` is not mutated; subsequent
    /// writes that should land on this parametric instance must go through
    /// the returned handle.
    ///
    /// Raises {class}`ValueError` if the constraint references an id that is
    /// neither a defined decision variable nor a defined parameter, or if it
    /// references an id currently used as a substitution-dependency key.
    pub fn add_constraint(
        slf: Bound<'_, Self>,
        constraint: Constraint,
    ) -> OmmxPyResult<crate::AttachedConstraint> {
        let id = {
            let mut inst = slf.borrow_mut();
            inst.inner.add_constraint(constraint.0, constraint.1)?
        };
        Ok(crate::AttachedConstraint::from_parametric(slf.unbind(), id))
    }

    /// Live mapping of all removed constraint families, keyed by typed IDs.
    /// The view retains this owner and fetches each constraint on demand.
    #[getter]
    pub fn removed_constraints(
        slf: Bound<'_, Self>,
    ) -> crate::constraint_collection::RemovedConstraintCollection {
        crate::constraint_collection::RemovedConstraintCollection::new(
            crate::ConstraintHost::Parametric(slf.unbind()),
        )
    }

    /// Add an indicator constraint to this parametric instance.
    ///
    /// Picks an unused {class}`~ommx.IndicatorConstraintID`, drains the
    /// wrapper's context snapshot into this parametric instance's SoA
    /// store, and returns an
    /// {class}`~ommx.AttachedIndicatorConstraint` bound to the new id.
    ///
    /// Raises {class}`ValueError` if the constraint references an id that
    /// is neither a defined decision variable nor a defined parameter, or
    /// if it references an id currently used as a substitution-dependency
    /// key.
    pub fn add_indicator_constraint(
        slf: Bound<'_, Self>,
        constraint: crate::IndicatorConstraint,
    ) -> OmmxPyResult<crate::AttachedIndicatorConstraint> {
        let id = {
            let mut inst = slf.borrow_mut();
            inst.inner
                .add_indicator_constraint(constraint.0, constraint.1)?
        };
        Ok(crate::AttachedIndicatorConstraint::from_parametric(
            slf.unbind(),
            id,
        ))
    }

    /// Add a one-hot constraint to this parametric instance.
    pub fn add_one_hot_constraint(
        slf: Bound<'_, Self>,
        constraint: crate::OneHotConstraint,
    ) -> OmmxPyResult<crate::AttachedOneHotConstraint> {
        let id = {
            let mut inst = slf.borrow_mut();
            inst.inner
                .add_one_hot_constraint(constraint.0, constraint.1)?
        };
        Ok(crate::AttachedOneHotConstraint::from_parametric(
            slf.unbind(),
            id,
        ))
    }

    /// Add a SOS1 constraint to this parametric instance.
    pub fn add_sos1_constraint(
        slf: Bound<'_, Self>,
        constraint: crate::Sos1Constraint,
    ) -> OmmxPyResult<crate::AttachedSos1Constraint> {
        let id = {
            let mut inst = slf.borrow_mut();
            inst.inner.add_sos1_constraint(constraint.0, constraint.1)?
        };
        Ok(crate::AttachedSos1Constraint::from_parametric(
            slf.unbind(),
            id,
        ))
    }

    #[getter]
    pub fn named_functions(&self) -> Vec<NamedFunction> {
        let labels = self.inner.named_function_labels();
        self.inner
            .named_functions()
            .iter()
            .map(|(id, nf)| NamedFunction(*id, nf.clone(), labels.collect_for(*id)))
            .collect()
    }

    /// Declared parameters and their modeling labels, sorted by ID.
    ///
    /// These are independent definitions, not assigned numerical values.
    /// Supply values through {meth}`~ommx.ParametricInstance.with_parameters`.
    #[getter]
    pub fn parameters(&self) -> Vec<Parameter> {
        self.inner
            .parameters()
            .to_v1_parameters()
            .into_iter()
            .map(Parameter)
            .collect()
    }

    #[getter]
    pub fn description(&self) -> Option<crate::InstanceDescription> {
        self.inner
            .description
            .clone()
            .map(crate::InstanceDescription)
    }

    #[getter]
    pub fn decision_variable_ids(&self) -> BTreeSet<u64> {
        self.inner
            .decision_variables()
            .keys()
            .map(|id| id.into_inner())
            .collect()
    }

    /// IDs of all declared parameters, including those unused in expressions.
    ///
    /// {meth}`~ommx.ParametricInstance.with_parameters` requires a value for
    /// every ID in this set. The set is independent of the owner.
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
        let labels = self.inner.variable_labels();
        self.inner
            .decision_variables()
            .get(&var_id)
            .map(|var| {
                DecisionVariable::from_parts(var_id, var.clone(), labels.collect_for(var_id))
            })
            .ok_or_else(|| {
                PyKeyError::new_err(format!("Decision variable with ID {variable_id} not found"))
            })
    }

    /// Get a specific named function by ID
    pub fn get_named_function_by_id(&self, named_function_id: u64) -> PyResult<NamedFunction> {
        let id = NamedFunctionID::from(named_function_id);
        self.inner
            .named_functions()
            .get(&id)
            .map(|nf| {
                NamedFunction(
                    id,
                    nf.clone(),
                    self.inner.named_function_labels().collect_for(id),
                )
            })
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
            .to_v1_parameter(VariableID::from(parameter_id))
            .map(Parameter)
            .ok_or_else(|| {
                PyKeyError::new_err(format!("Parameter with ID {parameter_id} not found"))
            })
    }

    /// DataFrame of decision variables
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
            &ommx::DecisionVariable,
            Option<f64>,
        )> = self
            .inner
            .decision_variables()
            .iter()
            .map(|(id, dv)| {
                (
                    var_meta_store.collect_for(*id),
                    *id,
                    dv,
                    self.inner.fixed_decision_variable_value(*id),
                )
            })
            .collect();
        entries_to_dataframe(
            py,
            view.iter().map(|(m, id, dv, fixed_value)| {
                crate::pandas::WithModelingContext::new((*id, *dv, *fixed_value), m)
            }),
            "id",
            flags,
        )
    }

    /// DataFrame of constraints, dispatched on `kind=`. See
    /// {meth}`ommx.Instance.constraints_df` for column / `kind=` /
    /// `include=` / `removed=` semantics.
    #[pyo3(signature = (*, kind, include = None, removed = false))]
    pub fn constraints_df<'py>(
        &self,
        py: Python<'py>,
        kind: ConstraintKind,
        include: Option<Vec<String>>,
        removed: bool,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let id_col = constraint_id_col(kind);
        let mut flags = crate::pandas::IncludeFlags::from_optional(include)?;
        if removed {
            flags.removed_reason = true;
        }
        constraint_kind_collection!(
            self.inner,
            kind,
            [
                constraint_collection,
                indicator_constraint_collection,
                one_hot_constraint_collection,
                sos1_constraint_collection
            ],
            |coll| {
                let meta = coll.context().clone();
                let active = coll.active();
                let removed_map = coll.removed();
                let mut entries: Vec<Bound<'py, pyo3::types::PyAny>> = Vec::new();
                if removed {
                    let mut ai = active.iter().peekable();
                    let mut ri = removed_map.iter().peekable();
                    loop {
                        let pick_active = match (ai.peek(), ri.peek()) {
                            (Some((aid, _)), Some((rid, _))) => aid <= rid,
                            (Some(_), None) => true,
                            (None, Some(_)) => false,
                            (None, None) => break,
                        };
                        if pick_active {
                            let (id, c) = ai.next().unwrap();
                            let m = meta.collect_for(*id);
                            let dict = crate::pandas::WithModelingContext::new((*id, c), &m)
                                .to_pandas_entry(py)?;
                            crate::pandas::set_removed_reason_na(&dict)?;
                            crate::pandas::apply_include_filter(&dict, flags)?;
                            crate::pandas::rename_id_column(&dict, id_col)?;
                            entries.push(dict.into_any());
                        } else {
                            let (id, pair) = ri.next().unwrap();
                            let m = meta.collect_for(*id);
                            let dict = crate::pandas::WithModelingContext::new((*id, pair), &m)
                                .to_pandas_entry(py)?;
                            crate::pandas::apply_include_filter(&dict, flags)?;
                            crate::pandas::rename_id_column(&dict, id_col)?;
                            entries.push(dict.into_any());
                        }
                    }
                } else {
                    for (id, c) in active.iter() {
                        let m = meta.collect_for(*id);
                        let dict = crate::pandas::WithModelingContext::new((*id, c), &m)
                            .to_pandas_entry(py)?;
                        if flags.removed_reason {
                            crate::pandas::set_removed_reason_na(&dict)?;
                        }
                        crate::pandas::apply_include_filter(&dict, flags)?;
                        crate::pandas::rename_id_column(&dict, id_col)?;
                        entries.push(dict.into_any());
                    }
                }
                crate::pandas::raw_entries_to_dataframe(py, entries, id_col)
            }
        )
    }

    /// DataFrame of named functions
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
            &ommx::NamedFunction,
        )> = self
            .inner
            .named_functions()
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

    /// Collect declared parameter definitions and modeling labels into a DataFrame.
    ///
    /// The index is the parameter ID, named ``id``. By default, the table
    /// includes ``name``, ``subscripts``, ``description``, and flattened
    /// ``parameters.<key>`` metadata columns. ``include=[]`` omits optional
    /// columns; ``include=["label"]`` selects the modeling labels and
    /// ``include=["parameters"]`` selects the string-valued metadata.
    ///
    /// These labels are not the numerical assignments supplied to
    /// {meth}`~ommx.ParametricInstance.with_parameters`. Editing the DataFrame
    /// does not change this ParametricInstance. See the class example for
    /// selecting a parameter's name and description by ID.
    #[pyo3(signature = (include = None))]
    pub fn parameters_df<'py>(
        &self,
        py: Python<'py>,
        include: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let flags = crate::pandas::IncludeFlags::from_optional(include)?;
        let parameters = self.inner.parameters().to_v1_parameters();
        entries_to_dataframe(py, parameters.iter(), "id", flags)
    }

    /// Constraint context DataFrame (id-indexed). See
    /// {meth}`ommx.Instance.constraint_context_df` for column / `kind=`
    /// semantics.
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
                constraint_collection,
                indicator_constraint_collection,
                one_hot_constraint_collection,
                sos1_constraint_collection
            ],
            |coll| {
                crate::pandas::constraint_context_dataframe(
                    py,
                    coll.context(),
                    coll.active().keys().chain(coll.removed().keys()).copied(),
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
                constraint_collection,
                indicator_constraint_collection,
                one_hot_constraint_collection,
                sos1_constraint_collection
            ],
            |coll| {
                crate::pandas::constraint_parameters_dataframe(
                    py,
                    coll.context(),
                    coll.active().keys().chain(coll.removed().keys()).copied(),
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
                constraint_collection,
                indicator_constraint_collection,
                one_hot_constraint_collection,
                sos1_constraint_collection
            ],
            |coll| {
                crate::pandas::constraint_provenance_dataframe(
                    py,
                    coll.context(),
                    coll.active().keys().chain(coll.removed().keys()).copied(),
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
                constraint_collection,
                indicator_constraint_collection,
                one_hot_constraint_collection,
                sos1_constraint_collection
            ],
            |coll| {
                crate::pandas::constraint_removed_reasons_dataframe(
                    py,
                    coll.removed().iter().map(|(id, (_, r))| (*id, r)),
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
}
