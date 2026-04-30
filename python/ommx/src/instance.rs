use crate::{
    pandas::{
        constraint_id_col, constraint_kind_collection, entries_to_dataframe, ConstraintKind,
        PyDataFrame, ToPandasEntry,
    },
    Constraint, DecisionVariable, Function, NamedFunction, ParametricInstance, RemovedConstraint,
    Rng, SampleSet, Samples, Sense, Solution, State, VariableBound,
};
use anyhow::Result;
use ommx::{ConstraintID, Evaluate, NamedFunctionID, VariableID};
use pyo3::{
    exceptions::PyKeyError,
    prelude::*,
    types::{PyBytes, PyDict},
    Bound, PyAny,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Optimization problem instance.
///
/// Note that this class also contains annotations like {attr}`~ommx.v1.Instance.title` which are not contained in protobuf message but stored in OMMX artifact.
/// These annotations are loaded from annotations while reading from OMMX artifact.
///
/// # Examples
///
/// Create an instance for KnapSack Problem
///
/// ```python
/// >>> from ommx.v1 import Instance, DecisionVariable
/// ```
///
/// Profit and weight of items
///
/// ```python
/// >>> p = [10, 13, 18, 31, 7, 15]
/// >>> w = [11, 15, 20, 35, 10, 33]
/// ```
///
/// Decision variables
///
/// ```python
/// >>> x = [DecisionVariable.binary(i) for i in range(6)]
/// ```
///
/// Objective and constraint
///
/// ```python
/// >>> objective = sum(p[i] * x[i] for i in range(6))
/// >>> constraint = sum(w[i] * x[i] for i in range(6)) <= 47
/// ```
///
/// Compose as an instance
///
/// ```python
/// >>> instance = Instance.from_components(
/// ...     decision_variables=x,
/// ...     objective=objective,
/// ...     constraints=[constraint],
/// ...     sense=Instance.MAXIMIZE,
/// ... )
/// ```
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
    pub fn from_bytes(py: Python<'_>, bytes: &Bound<PyBytes>) -> Result<Self> {
        let _guard = crate::TRACING.attach_parent_context(py);
        Ok(Self {
            inner: ommx::Instance::from_bytes(bytes.as_bytes())?,
            annotations: HashMap::new(),
        })
    }

    /// Create an instance from its components.
    ///
    /// **Args:**
    /// - `sense`: Optimization sense (minimize or maximize)
    /// - `objective`: Objective function
    /// - `decision_variables`: List of decision variables
    /// - `constraints`: List of constraints
    /// - `named_functions`: Optional list of named functions
    /// - `description`: Optional instance description
    ///
    /// **Returns:**
    /// A new Instance
    #[staticmethod]
    #[pyo3(signature = (*, sense, objective, decision_variables, constraints, indicator_constraints=None, one_hot_constraints=None, sos1_constraints=None, named_functions=None, description=None))]
    pub fn from_components(
        sense: Sense,
        objective: Function,
        decision_variables: Vec<DecisionVariable>,
        constraints: BTreeMap<u64, Constraint>,
        indicator_constraints: Option<BTreeMap<u64, crate::IndicatorConstraint>>,
        one_hot_constraints: Option<BTreeMap<u64, crate::OneHotConstraint>>,
        sos1_constraints: Option<BTreeMap<u64, crate::Sos1Constraint>>,
        named_functions: Option<Vec<NamedFunction>>,
        description: Option<InstanceDescription>,
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

        let mut builder = ommx::Instance::builder()
            .sense(sense.into())
            .objective(objective.0)
            .decision_variables(rust_decision_variables)
            .constraints(rust_constraints);

        let mut indicator_metadata_pairs: Vec<(
            ommx::IndicatorConstraintID,
            ommx::ConstraintMetadata,
        )> = Vec::new();
        if let Some(ics) = indicator_constraints {
            let rust_indicator_constraints: BTreeMap<
                ommx::IndicatorConstraintID,
                ommx::IndicatorConstraint,
            > = ics
                .into_iter()
                .map(|(id, ic)| {
                    let iid = ommx::IndicatorConstraintID::from(id);
                    indicator_metadata_pairs.push((iid, ic.1));
                    (iid, ic.0)
                })
                .collect();
            builder = builder.indicator_constraints(rust_indicator_constraints);
        }

        let mut one_hot_metadata_pairs: Vec<(ommx::OneHotConstraintID, ommx::ConstraintMetadata)> =
            Vec::new();
        if let Some(ohs) = one_hot_constraints {
            let rust_one_hot_constraints: BTreeMap<
                ommx::OneHotConstraintID,
                ommx::OneHotConstraint,
            > = ohs
                .into_iter()
                .map(|(id, oh)| {
                    let oid = ommx::OneHotConstraintID::from(id);
                    one_hot_metadata_pairs.push((oid, oh.1));
                    (oid, oh.0)
                })
                .collect();
            builder = builder.one_hot_constraints(rust_one_hot_constraints);
        }

        let mut sos1_metadata_pairs: Vec<(ommx::Sos1ConstraintID, ommx::ConstraintMetadata)> =
            Vec::new();
        if let Some(s1s) = sos1_constraints {
            let rust_sos1_constraints: BTreeMap<ommx::Sos1ConstraintID, ommx::Sos1Constraint> = s1s
                .into_iter()
                .map(|(id, s1)| {
                    let sid = ommx::Sos1ConstraintID::from(id);
                    sos1_metadata_pairs.push((sid, s1.1));
                    (sid, s1.0)
                })
                .collect();
            builder = builder.sos1_constraints(rust_sos1_constraints);
        }

        let mut named_function_metadata_pairs: Vec<(
            ommx::NamedFunctionID,
            ommx::NamedFunctionMetadata,
        )> = Vec::new();
        if let Some(nfs) = named_functions {
            let mut rust_named_functions = BTreeMap::new();
            for nf in nfs {
                let id = nf.0.id;
                named_function_metadata_pairs.push((id, nf.1));
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
        // Drain wrapper-side metadata snapshots into the instance's SoA stores.
        let var_meta = inner.variable_metadata_mut();
        for (id, m) in variable_metadata_pairs {
            var_meta.insert(id, m);
        }
        let constraint_meta = inner.constraint_metadata_mut();
        for (id, m) in constraint_metadata_pairs {
            constraint_meta.insert(id, m);
        }
        let indicator_meta = inner.indicator_constraint_metadata_mut();
        for (id, m) in indicator_metadata_pairs {
            indicator_meta.insert(id, m);
        }
        let one_hot_meta = inner.one_hot_constraint_metadata_mut();
        for (id, m) in one_hot_metadata_pairs {
            one_hot_meta.insert(id, m);
        }
        let sos1_meta = inner.sos1_constraint_metadata_mut();
        for (id, m) in sos1_metadata_pairs {
            sos1_meta.insert(id, m);
        }
        let nf_meta = inner.named_function_metadata_mut();
        for (id, m) in named_function_metadata_pairs {
            nf_meta.insert(id, m);
        }

        Ok(Self {
            inner,
            annotations: HashMap::new(),
        })
    }

    /// Create trivial empty instance of minimization with zero objective, no constraints, and no decision variables.
    ///
    /// # Examples
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance
    /// >>> instance = Instance.empty()
    /// >>> instance.sense == Instance.MINIMIZE
    /// True
    /// ```
    #[staticmethod]
    pub fn empty() -> Result<Self> {
        Self::from_components(
            Sense::Minimize,
            Function(ommx::Function::Zero),
            Vec::new(),
            BTreeMap::new(),
            None,
            None,
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

    #[gen_stub(override_return_type(type_repr = "type[InstanceDescription]"))]
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
    ///
    /// Returns a list of {class}`~ommx.v1.AttachedDecisionVariable` write-through
    /// handles. Each handle reads its kind / bound / metadata live from this
    /// instance's SoA store and writes metadata mutations back through to it.
    /// Handles also participate in arithmetic to build expressions
    /// (`x + y`, `2 * x` etc.) — only their id is consumed for that, no host
    /// borrow is taken. Call
    /// {meth}`~ommx.v1.AttachedDecisionVariable.detach` if you need an
    /// independent {class}`~ommx.v1.DecisionVariable` snapshot.
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
            .map(|id| crate::AttachedDecisionVariable::from_instance(py_instance.clone_ref(py), id))
            .collect()
    }

    /// Add a decision variable to this instance.
    ///
    /// Drains the wrapper's metadata snapshot into this instance's SoA
    /// store and returns an {class}`~ommx.v1.AttachedDecisionVariable`
    /// bound to the variable's id — a write-through handle for further
    /// metadata mutation. The original wrapper is not modified.
    ///
    /// Raises {class}`ValueError` if the variable's id collides with an
    /// existing variable, parameter, or substitution-dependency key.
    pub fn add_decision_variable(
        slf: Bound<'_, Self>,
        variable: DecisionVariable,
    ) -> Result<crate::AttachedDecisionVariable> {
        let id = {
            let mut inst = slf.borrow_mut();
            inst.inner.add_decision_variable(variable.0, variable.1)?
        };
        Ok(crate::AttachedDecisionVariable::from_instance(
            slf.unbind(),
            id,
        ))
    }

    /// Return an {class}`~ommx.v1.AttachedDecisionVariable` bound to the
    /// given id — a write-through handle whose metadata setters update
    /// this instance's SoA store. The handle also participates in
    /// arithmetic via `ToFunction` (only its id is consumed). Call
    /// {meth}`~ommx.v1.AttachedDecisionVariable.detach` to obtain an
    /// independent {class}`~ommx.v1.DecisionVariable` snapshot.
    ///
    /// Raises {class}`KeyError` if no variable with `variable_id` exists.
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
        Ok(crate::AttachedDecisionVariable::from_instance(
            slf.unbind(),
            id,
        ))
    }

    /// Dict of all active constraints in the instance keyed by their IDs.
    ///
    /// Each value is an {class}`~ommx.v1.AttachedConstraint`: a write-through
    /// handle whose getters read from this instance's SoA store and whose
    /// metadata setters write back through to it. Use
    /// {meth}`~ommx.v1.AttachedConstraint.detach` to materialize a
    /// {class}`~ommx.v1.Constraint` snapshot if you need an independent copy.
    #[getter]
    pub fn constraints(slf: Bound<'_, Self>) -> BTreeMap<u64, crate::AttachedConstraint> {
        let py = slf.py();
        let ids: Vec<ConstraintID> = slf.borrow().inner.constraints().keys().copied().collect();
        let py_instance: Py<Self> = slf.unbind();
        ids.into_iter()
            .map(|id| {
                (
                    id.into_inner(),
                    crate::AttachedConstraint::from_instance(py_instance.clone_ref(py), id),
                )
            })
            .collect()
    }

    /// Add a regular constraint to this instance.
    ///
    /// Picks an unused {class}`~ommx.v1.ConstraintID`, drains the wrapper's
    /// metadata snapshot into this instance's SoA store, and returns an
    /// {class}`~ommx.v1.AttachedConstraint` bound to the new id. The input
    /// {class}`~ommx.v1.Constraint` is not mutated; subsequent writes that
    /// should land in the instance must go through the returned handle.
    ///
    /// Raises {class}`ValueError` if the constraint references an undefined
    /// decision variable or one currently used as a substitution-dependency
    /// key, matching the validation performed by other constraint-insertion
    /// paths.
    pub fn add_constraint(
        slf: Bound<'_, Self>,
        constraint: Constraint,
    ) -> Result<crate::AttachedConstraint> {
        let id = {
            let mut inst = slf.borrow_mut();
            inst.inner.add_constraint(constraint.0, constraint.1)?
        };
        Ok(crate::AttachedConstraint::from_instance(slf.unbind(), id))
    }

    /// Dict of all active indicator constraints in the instance keyed by
    /// their IDs.
    ///
    /// Each value is an {class}`~ommx.v1.AttachedIndicatorConstraint`: a
    /// write-through handle whose getters read from this instance's SoA
    /// store and whose metadata setters write back through to it.
    #[getter]
    pub fn indicator_constraints(
        slf: Bound<'_, Self>,
    ) -> BTreeMap<u64, crate::AttachedIndicatorConstraint> {
        let py = slf.py();
        let ids: Vec<ommx::IndicatorConstraintID> = slf
            .borrow()
            .inner
            .indicator_constraints()
            .keys()
            .copied()
            .collect();
        let py_instance: Py<Self> = slf.unbind();
        ids.into_iter()
            .map(|id| {
                (
                    id.into_inner(),
                    crate::AttachedIndicatorConstraint::from_instance(
                        py_instance.clone_ref(py),
                        id,
                    ),
                )
            })
            .collect()
    }

    /// Add an indicator constraint to this instance.
    ///
    /// Picks an unused {class}`~ommx.v1.IndicatorConstraintID`, drains the
    /// wrapper's metadata snapshot into this instance's SoA store, and
    /// returns an {class}`~ommx.v1.AttachedIndicatorConstraint` bound to the
    /// new id.
    ///
    /// Raises {class}`ValueError` if the constraint references an undefined
    /// decision variable or one currently used as a substitution-dependency
    /// key.
    pub fn add_indicator_constraint(
        slf: Bound<'_, Self>,
        constraint: crate::IndicatorConstraint,
    ) -> Result<crate::AttachedIndicatorConstraint> {
        let id = {
            let mut inst = slf.borrow_mut();
            inst.inner
                .add_indicator_constraint(constraint.0, constraint.1)?
        };
        Ok(crate::AttachedIndicatorConstraint::from_instance(
            slf.unbind(),
            id,
        ))
    }

    /// Dict of all removed indicator constraints in the instance keyed by their IDs.
    #[getter]
    pub fn removed_indicator_constraints(
        &self,
    ) -> BTreeMap<u64, crate::RemovedIndicatorConstraint> {
        let metadata = self.inner.indicator_constraint_collection().metadata();
        self.inner
            .removed_indicator_constraints()
            .iter()
            .map(|(id, (c, r))| {
                (
                    id.into_inner(),
                    crate::RemovedIndicatorConstraint::from_parts(
                        c.clone(),
                        metadata.collect_for(*id),
                        r.clone(),
                    ),
                )
            })
            .collect()
    }

    /// Dict of all active one-hot constraints in the instance keyed by their IDs.
    ///
    /// Each value is an {class}`~ommx.v1.AttachedOneHotConstraint`: a
    /// write-through handle whose getters read from this instance's SoA
    /// store and whose metadata setters write back through to it.
    #[getter]
    pub fn one_hot_constraints(
        slf: Bound<'_, Self>,
    ) -> BTreeMap<u64, crate::AttachedOneHotConstraint> {
        let py = slf.py();
        let ids: Vec<ommx::OneHotConstraintID> = slf
            .borrow()
            .inner
            .one_hot_constraints()
            .keys()
            .copied()
            .collect();
        let py_instance: Py<Self> = slf.unbind();
        ids.into_iter()
            .map(|id| {
                (
                    id.into_inner(),
                    crate::AttachedOneHotConstraint::from_instance(py_instance.clone_ref(py), id),
                )
            })
            .collect()
    }

    /// Add a one-hot constraint to this instance.
    pub fn add_one_hot_constraint(
        slf: Bound<'_, Self>,
        constraint: crate::OneHotConstraint,
    ) -> Result<crate::AttachedOneHotConstraint> {
        let id = {
            let mut inst = slf.borrow_mut();
            inst.inner
                .add_one_hot_constraint(constraint.0, constraint.1)?
        };
        Ok(crate::AttachedOneHotConstraint::from_instance(
            slf.unbind(),
            id,
        ))
    }

    /// Dict of all removed one-hot constraints in the instance keyed by their IDs.
    #[getter]
    pub fn removed_one_hot_constraints(&self) -> BTreeMap<u64, crate::RemovedOneHotConstraint> {
        let metadata = self.inner.one_hot_constraint_metadata();
        self.inner
            .removed_one_hot_constraints()
            .iter()
            .map(|(id, (c, r))| {
                (
                    id.into_inner(),
                    crate::RemovedOneHotConstraint::from_parts(
                        c.clone(),
                        metadata.collect_for(*id),
                        r.clone(),
                    ),
                )
            })
            .collect()
    }

    /// Dict of all active SOS1 constraints in the instance keyed by their IDs.
    ///
    /// Each value is an {class}`~ommx.v1.AttachedSos1Constraint`: a
    /// write-through handle whose getters read from this instance's SoA
    /// store and whose metadata setters write back through to it.
    #[getter]
    pub fn sos1_constraints(slf: Bound<'_, Self>) -> BTreeMap<u64, crate::AttachedSos1Constraint> {
        let py = slf.py();
        let ids: Vec<ommx::Sos1ConstraintID> = slf
            .borrow()
            .inner
            .sos1_constraints()
            .keys()
            .copied()
            .collect();
        let py_instance: Py<Self> = slf.unbind();
        ids.into_iter()
            .map(|id| {
                (
                    id.into_inner(),
                    crate::AttachedSos1Constraint::from_instance(py_instance.clone_ref(py), id),
                )
            })
            .collect()
    }

    /// Add a SOS1 constraint to this instance.
    pub fn add_sos1_constraint(
        slf: Bound<'_, Self>,
        constraint: crate::Sos1Constraint,
    ) -> Result<crate::AttachedSos1Constraint> {
        let id = {
            let mut inst = slf.borrow_mut();
            inst.inner.add_sos1_constraint(constraint.0, constraint.1)?
        };
        Ok(crate::AttachedSos1Constraint::from_instance(
            slf.unbind(),
            id,
        ))
    }

    /// Dict of all removed SOS1 constraints in the instance keyed by their IDs.
    #[getter]
    pub fn removed_sos1_constraints(&self) -> BTreeMap<u64, crate::RemovedSos1Constraint> {
        let metadata = self.inner.sos1_constraint_metadata();
        self.inner
            .removed_sos1_constraints()
            .iter()
            .map(|(id, (c, r))| {
                (
                    id.into_inner(),
                    crate::RemovedSos1Constraint::from_parts(
                        c.clone(),
                        metadata.collect_for(*id),
                        r.clone(),
                    ),
                )
            })
            .collect()
    }

    /// The non-standard constraint capabilities this instance currently uses.
    ///
    /// Returns the set of :class:`AdditionalCapability` values corresponding to
    /// the active (non-removed) constraint collections the instance contains.
    /// An empty set means the instance only uses regular constraints.
    ///
    /// Callers can diff this against an adapter's
    /// ``ADDITIONAL_CAPABILITIES`` to see what would be converted, or use
    /// :meth:`reduce_capabilities` to perform the conversion.
    #[getter]
    pub fn required_capabilities(&self) -> std::collections::HashSet<crate::AdditionalCapability> {
        self.inner
            .required_capabilities()
            .into_iter()
            .map(|c| c.into())
            .collect()
    }

    /// Convert constraint types not in `supported` into regular constraints.
    ///
    /// For every capability in :attr:`required_capabilities` not in
    /// ``supported``, the corresponding bulk conversion is invoked
    /// (:meth:`convert_all_indicators_to_constraints`,
    /// :meth:`convert_all_one_hots_to_constraints`, or
    /// :meth:`convert_all_sos1_to_constraints`). The instance is mutated in
    /// place and :attr:`required_capabilities` becomes a subset of
    /// ``supported`` on success.
    ///
    /// Returns the set of :class:`AdditionalCapability` values that were
    /// actually converted. Empty when nothing needed conversion.
    ///
    /// Raises if any underlying Big-M conversion fails (e.g. a SOS1 variable
    /// with a non-finite bound).
    pub fn reduce_capabilities(
        &mut self,
        py: Python<'_>,
        supported: std::collections::HashSet<crate::AdditionalCapability>,
    ) -> anyhow::Result<std::collections::HashSet<crate::AdditionalCapability>> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let rust_supported: ommx::Capabilities = supported.into_iter().map(|c| c.into()).collect();
        let converted = self.inner.reduce_capabilities(&rust_supported)?;
        Ok(converted.into_iter().map(|c| c.into()).collect())
    }

    /// Dict of all removed constraints in the instance keyed by their IDs.
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

    /// List of all named functions in the instance sorted by their IDs.
    #[getter]
    pub fn named_functions(&self) -> Vec<NamedFunction> {
        let metadata = self.inner.named_function_metadata();
        self.inner
            .named_functions()
            .iter()
            .map(|(id, named_function)| {
                NamedFunction(named_function.clone(), metadata.collect_for(*id))
            })
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
    pub fn used_decision_variables(&self) -> Vec<DecisionVariable> {
        let metadata = self.inner.variable_metadata();
        self.inner
            .used_decision_variables()
            .iter()
            .map(|(id, &var)| DecisionVariable::from_parts(var.clone(), metadata.collect_for(*id)))
            .collect()
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let buf = self.inner.to_bytes();
        PyBytes::new(py, &buf)
    }

    /// Get the set of decision variable IDs used in the objective and remaining constraints.
    ///
    /// # Examples
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints=[],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// >>> instance.required_ids()
    /// {0, 1, 2}
    /// ```
    pub fn required_ids(&self) -> BTreeSet<u64> {
        self.inner
            .required_ids()
            .into_iter()
            .map(|id| id.into_inner())
            .collect()
    }

    pub fn as_qubo_format<'py>(&self, py: Python<'py>) -> Result<(Bound<'py, PyDict>, f64)> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let (qubo, constant) = self.inner.as_qubo_format()?;
        Ok((
            serde_pyobject::to_pyobject(py, &qubo)?
                .extract()
                .map_err(|e| anyhow::anyhow!("{}", e))?,
            constant,
        ))
    }

    pub fn as_hubo_format<'py>(&self, py: Python<'py>) -> Result<(Bound<'py, PyDict>, f64)> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let (hubo, constant) = self.inner.as_hubo_format()?;
        Ok((
            serde_pyobject::to_pyobject(py, &hubo)?
                .extract()
                .map_err(|e| anyhow::anyhow!("{}", e))?,
            constant,
        ))
    }

    /// Convert the instance to a QUBO format.
    ///
    /// This is a **Driver API** for QUBO conversion calling single-purpose methods in order:
    ///
    /// 1. Convert the instance to a minimization problem by {meth}`~ommx.v1.Instance.as_minimization_problem`.
    /// 2. Check continuous variables and raise error if exists.
    /// 3. Convert inequality constraints
    ///
    ///   * Try {meth}`~ommx.v1.Instance.convert_inequality_to_equality_with_integer_slack` first with given ``inequality_integer_slack_max_range``.
    ///   * If failed, {meth}`~ommx.v1.Instance.add_integer_slack_to_inequality`
    ///
    /// 4. Convert to QUBO with (uniform) penalty method
    ///
    ///   * If ``penalty_weights`` is given (in ``dict[constraint_id, weight]`` form), use {meth}`~ommx.v1.Instance.penalty_method` with the given weights.
    ///   * If ``uniform_penalty_weight`` is given, use {meth}`~ommx.v1.Instance.uniform_penalty_method` with the given weight.
    ///   * If both are None, defaults to ``uniform_penalty_weight = 1.0``.
    ///
    /// 5. Log-encode integer variables by {meth}`~ommx.v1.Instance.log_encode`.
    /// 6. Finally convert to QUBO format by {meth}`~ommx.v1.Instance.as_qubo_format`.
    ///
    /// Please see the document of each method for details.
    /// If you want to customize the conversion, use the methods above manually.
    ///
    /// # Examples
    ///
    /// Let's consider a maximization problem with two integer variables $x_0, x_1 \in [0, 2]$ subject to an inequality:
    ///
    /// $$\max \; x_0 + x_1 \quad \text{s.t.} \quad x_0 + 2 x_1 \leq 3$$
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [DecisionVariable.integer(i, lower=0, upper=2, name="x", subscripts=[i]) for i in range(2)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints=[(x[0] + 2*x[1] <= 3).set_id(0)],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// ```
    ///
    /// Convert into QUBO format
    ///
    /// ```python
    /// >>> qubo, offset = instance.to_qubo()
    /// >>> qubo
    /// {(3, 3): -6.0, (3, 4): 2.0, (3, 5): 4.0, (3, 6): 4.0, (3, 7): 2.0, (3, 8): 4.0, (4, 4): -6.0, (4, 5): 4.0, (4, 6): 4.0, (4, 7): 2.0, (4, 8): 4.0, (5, 5): -9.0, (5, 6): 8.0, (5, 7): 4.0, (5, 8): 8.0, (6, 6): -9.0, (6, 7): 4.0, (6, 8): 8.0, (7, 7): -5.0, (7, 8): 4.0, (8, 8): -8.0}
    /// >>> offset
    /// 9.0
    /// ```
    ///
    /// For the maximization problem, the sense is converted to minimization for generating QUBO, and then converted back to maximization.
    ///
    /// ```python
    /// >>> instance.sense == Instance.MAXIMIZE
    /// True
    /// ```
    #[pyo3(signature = (*, uniform_penalty_weight=None, penalty_weights=None, inequality_integer_slack_max_range=31))]
    pub fn to_qubo<'py>(
        &mut self,
        py: Python<'py>,
        uniform_penalty_weight: Option<f64>,
        penalty_weights: Option<HashMap<u64, f64>>,
        inequality_integer_slack_max_range: u64,
    ) -> Result<(Bound<'py, PyDict>, f64)> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let is_converted = self.as_minimization_problem();
        self.check_no_continuous_variables("QUBO")?;
        self.qubo_hubo_pipeline(
            uniform_penalty_weight,
            penalty_weights,
            inequality_integer_slack_max_range,
        )?;
        self.log_encode(py, BTreeSet::new())?;
        let result = self.as_qubo_format(py)?;
        if is_converted {
            self.as_maximization_problem();
        }
        Ok(result)
    }

    /// Convert the instance to a HUBO format.
    ///
    /// This is a **Driver API** for HUBO conversion calling single-purpose methods in order:
    ///
    /// 1. Convert the instance to a minimization problem by {meth}`~ommx.v1.Instance.as_minimization_problem`.
    /// 2. Check continuous variables and raise error if exists.
    /// 3. Convert inequality constraints
    ///
    ///   * Try {meth}`~ommx.v1.Instance.convert_inequality_to_equality_with_integer_slack` first with given ``inequality_integer_slack_max_range``.
    ///   * If failed, {meth}`~ommx.v1.Instance.add_integer_slack_to_inequality`
    ///
    /// 4. Convert to HUBO with (uniform) penalty method
    ///
    ///   * If ``penalty_weights`` is given (in ``dict[constraint_id, weight]`` form), use {meth}`~ommx.v1.Instance.penalty_method` with the given weights.
    ///   * If ``uniform_penalty_weight`` is given, use {meth}`~ommx.v1.Instance.uniform_penalty_method` with the given weight.
    ///   * If both are None, defaults to ``uniform_penalty_weight = 1.0``.
    ///
    /// 5. Log-encode integer variables by {meth}`~ommx.v1.Instance.log_encode`.
    /// 6. Finally convert to HUBO format by {meth}`~ommx.v1.Instance.as_hubo_format`.
    ///
    /// Please see the documentation for {meth}`~ommx.v1.Instance.to_qubo` for more information, or the
    /// documentation for each individual method for additional details. The
    /// difference between this and {meth}`~ommx.v1.Instance.to_qubo` is that this method isn't
    /// restricted to quadratic or linear problems. If you want to customize the
    /// conversion, use the individual methods above manually.
    #[pyo3(signature = (*, uniform_penalty_weight=None, penalty_weights=None, inequality_integer_slack_max_range=31))]
    pub fn to_hubo<'py>(
        &mut self,
        py: Python<'py>,
        uniform_penalty_weight: Option<f64>,
        penalty_weights: Option<HashMap<u64, f64>>,
        inequality_integer_slack_max_range: u64,
    ) -> Result<(Bound<'py, PyDict>, f64)> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let is_converted = self.as_minimization_problem();
        self.check_no_continuous_variables("HUBO")?;
        self.qubo_hubo_pipeline(
            uniform_penalty_weight,
            penalty_weights,
            inequality_integer_slack_max_range,
        )?;
        self.log_encode(py, BTreeSet::new())?;
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

    /// Convert to a parametric unconstrained instance by penalty method.
    ///
    /// Roughly, this converts a constrained problem:
    ///
    /// $$\min_x f(x) \quad \text{s.t.} \quad g_i(x) = 0 \; (\forall i), \quad h_j(x) \leq 0 \; (\forall j)$$
    ///
    /// to an unconstrained problem with parameters:
    ///
    /// $$\min_x f(x) + \sum_i \lambda_i g_i(x)^2 + \sum_j \rho_j h_j(x)^2$$
    ///
    /// where $\lambda_i$ and $\rho_j$ are the penalty weight parameters for each constraint.
    /// If you want to use single weight parameter, use {meth}`~ommx.v1.Instance.uniform_penalty_method` instead.
    ///
    /// The removed constraints are stored in {attr}`~ommx.v1.ParametricInstance.removed_constraints`.
    ///
    /// > Note: This method converts inequality constraints $h(x) \leq 0$ to $|h(x)|^2$ not to $\max(0, h(x))^2$.
    /// > This means the penalty is enforced even for $h(x) < 0$ cases, and $h(x) = 0$ is unfairly favored.
    /// > This feature is intended to use with {meth}`~ommx.v1.Instance.add_integer_slack_to_inequality`.
    ///
    /// # Examples
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable, Constraint
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints=[x[0] + x[1] == 1, x[1] + x[2] == 1],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// >>> instance.objective
    /// Function(x0 + x1 + x2)
    /// >>> pi = instance.penalty_method()
    /// ```
    ///
    /// The constraint is put in removed_constraints
    ///
    /// ```python
    /// >>> pi.constraints
    /// []
    /// >>> len(pi.removed_constraints)
    /// 2
    /// >>> pi.removed_constraints[0]
    /// RemovedConstraint(x0 + x1 - 1 == 0, reason=ommx.Instance.penalty_method, parameter_id=3)
    /// >>> pi.removed_constraints[1]
    /// RemovedConstraint(x1 + x2 - 1 == 0, reason=ommx.Instance.penalty_method, parameter_id=4)
    /// ```
    pub fn penalty_method(&self, py: Python<'_>) -> Result<ParametricInstance> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let parametric_instance = self.inner.clone().penalty_method()?;
        Ok(ParametricInstance {
            inner: parametric_instance,
            annotations: HashMap::new(),
        })
    }

    /// Convert to a parametric unconstrained instance by penalty method with uniform weight.
    ///
    /// Roughly, this converts a constrained problem:
    ///
    /// $$\min_x f(x) \quad \text{s.t.} \quad g_i(x) = 0 \; (\forall i), \quad h_j(x) \leq 0 \; (\forall j)$$
    ///
    /// to an unconstrained problem with a parameter:
    ///
    /// $$\min_x f(x) + \lambda \left( \sum_i g_i(x)^2 + \sum_j h_j(x)^2 \right)$$
    ///
    /// where $\lambda$ is the uniform penalty weight parameter for all constraints.
    ///
    /// The removed constraints are stored in {attr}`~ommx.v1.ParametricInstance.removed_constraints`.
    ///
    /// > Note: This method converts inequality constraints $h(x) \leq 0$ to $|h(x)|^2$ not to $\max(0, h(x))^2$.
    /// > This means the penalty is enforced even for $h(x) < 0$ cases, and $h(x) = 0$ is unfairly favored.
    /// > This feature is intended to use with {meth}`~ommx.v1.Instance.add_integer_slack_to_inequality`.
    ///
    /// # Examples
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints=[sum(x) == 3],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// >>> instance.objective
    /// Function(x0 + x1 + x2)
    /// >>> pi = instance.uniform_penalty_method()
    /// ```
    ///
    /// The constraint is put in removed_constraints
    ///
    /// ```python
    /// >>> pi.constraints
    /// []
    /// >>> len(pi.removed_constraints)
    /// 1
    /// >>> pi.removed_constraints[0]
    /// RemovedConstraint(x0 + x1 + x2 - 3 == 0, reason=ommx.Instance.uniform_penalty_method)
    /// ```
    ///
    /// There is only one parameter in the instance
    ///
    /// ```python
    /// >>> len(pi.parameters)
    /// 1
    /// >>> p = pi.parameters[0]
    /// >>> p.id
    /// 3
    /// >>> p.name
    /// 'uniform_penalty_weight'
    /// ```
    pub fn uniform_penalty_method(&self, py: Python<'_>) -> Result<ParametricInstance> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let parametric_instance = self.inner.clone().uniform_penalty_method()?;
        Ok(ParametricInstance {
            inner: parametric_instance,
            annotations: HashMap::new(),
        })
    }

    /// Evaluate the given {class}`~ommx.v1.State` into a {class}`~ommx.v1.Solution`.
    ///
    /// This method evaluates the problem instance using the provided state (a map from decision variable IDs to their values),
    /// and returns a {class}`~ommx.v1.Solution` object containing objective value, evaluated constraint values, and feasibility information.
    ///
    /// # Examples
    ///
    /// Create a simple instance with three binary variables and evaluate a solution:
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints=[(x[0] + x[1] <= 1).set_id(0)],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// ```
    ///
    /// Evaluate it with a state x0 = 1, x1 = 0, x2 = 0, and show the objective and constraints:
    ///
    /// ```python
    /// >>> solution = instance.evaluate({0: 1, 1: 0, 2: 0})
    /// >>> solution.objective
    /// 1.0
    /// ```
    ///
    /// If the value is out of the range, the solution is infeasible:
    ///
    /// ```python
    /// >>> solution = instance.evaluate({0: 1, 1: 0, 2: 2})
    /// >>> solution.feasible
    /// False
    /// ```
    ///
    /// If some of the decision variables are not set, this raises an error:
    ///
    /// ```python
    /// >>> instance.evaluate({0: 1, 1: 0})
    /// ```
    /// Traceback (most recent call last):
    ///     ...
    /// ValueError: The state does not contain some required IDs: {VariableID(2)}
    #[pyo3(signature = (state, *, atol=None))]
    pub fn evaluate(&self, py: Python<'_>, state: State, atol: Option<f64>) -> PyResult<Solution> {
        let _guard = crate::TRACING.attach_parent_context(py);
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

    /// Creates a new instance with specific decision variables fixed to given values.
    ///
    /// This method substitutes the specified decision variables with their provided values,
    /// creating a new problem instance where these variables are fixed. This is useful for
    /// scenarios such as:
    ///
    /// - Creating simplified sub-problems with some variables fixed
    /// - Incrementally solving a problem by fixing some variables and optimizing the rest
    /// - Testing specific configurations of a problem
    ///
    /// **Args:**
    /// - `state`: Maps decision variable IDs to their fixed values.
    ///   Can be a {class}`~ommx.v1.State` object or a dictionary mapping variable IDs to values.
    /// - `atol`: Absolute tolerance for floating point comparisons. If None, uses the default tolerance.
    ///
    /// **Returns:**
    /// A new instance with the specified decision variables fixed to their given values.
    ///
    /// # Examples
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = DecisionVariable.binary(1)
    /// >>> y = DecisionVariable.binary(2)
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x, y],
    /// ...     objective=x + y,
    /// ...     constraints=[x + y <= 1],
    /// ...     sense=Instance.MINIMIZE
    /// ... )
    /// >>> new_instance = instance.partial_evaluate({1: 1})
    /// >>> new_instance.objective
    /// Function(x2 + 1)
    /// ```
    ///
    /// Substituted value is stored in the decision variable:
    ///
    /// ```python
    /// >>> x = new_instance.get_decision_variable_by_id(1)
    /// >>> x.substituted_value
    /// 1.0
    /// ```
    #[pyo3(signature = (state, *, atol=None))]
    pub fn partial_evaluate(
        &self,
        py: Python<'_>,
        state: State,
        atol: Option<f64>,
    ) -> PyResult<Self> {
        let _guard = crate::TRACING.attach_parent_context(py);
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
        py: Python<'_>,
        samples: Samples,
        atol: Option<f64>,
    ) -> Result<SampleSet> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        Ok(SampleSet {
            inner: self.inner.evaluate_samples(&samples.0, atol)?,
            annotations: HashMap::new(),
        })
    }

    /// Generate a random state for this instance using the provided random number generator.
    ///
    /// This method generates random values only for variables that are actually used in the
    /// objective function or constraints, as determined by decision variable analysis.
    /// Generated values respect the bounds of each variable type.
    ///
    /// **Args:**
    /// - `rng`: Random number generator to use for generating the state.
    ///
    /// **Returns:**
    /// A randomly generated state that satisfies the variable bounds of this instance.
    /// Only contains values for variables that are used in the problem.
    ///
    /// # Examples
    ///
    /// Generate random state only for used variables
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable, Rng
    /// >>> x = [DecisionVariable.binary(i) for i in range(5)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=x[0] + x[1],
    /// ...     constraints=[],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    ///
    /// >>> rng = Rng()
    /// >>> state = instance.random_state(rng)
    /// ```
    ///
    /// Only used variables have values
    ///
    /// ```python
    /// >>> set(state.entries.keys())
    /// {0, 1}
    /// ```
    ///
    /// Values respect binary bounds
    ///
    /// ```python
    /// >>> all(state.entries[i] in [0.0, 1.0] for i in state.entries)
    /// True
    /// ```
    pub fn random_state(&self, rng: &Rng) -> Result<crate::State> {
        let strategy = self.inner.arbitrary_state();
        let mut rng_guard = rng
            .lock()
            .map_err(|_| anyhow::anyhow!("Cannot get lock for RNG"))?;
        let state = ommx::random::sample(&mut rng_guard, strategy);
        Ok(crate::State(state))
    }

    /// Generate random samples for this instance.
    ///
    /// The generated samples will contain ``num_samples`` sample entries divided into
    /// ``num_different_samples`` groups, where each group shares the same state but has
    /// different sample IDs.
    ///
    /// **Args:**
    /// - `rng`: Random number generator
    /// - `num_different_samples`: Number of different states to generate
    /// - `num_samples`: Total number of samples to generate
    /// - `max_sample_id`: Maximum sample ID (default: ``num_samples``)
    ///
    /// **Returns:**
    /// Samples object
    ///
    /// # Examples
    ///
    /// Generate samples for a simple instance:
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable, Rng
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints=[(sum(x) <= 2).set_id(0)],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    ///
    /// >>> rng = Rng()
    /// >>> samples = instance.random_samples(rng, num_different_samples=2, num_samples=5)
    /// >>> samples.num_samples()
    /// 5
    /// ```
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

    /// Remove a constraint from the instance.
    ///
    /// The removed constraint is stored in {attr}`~ommx.v1.Instance.removed_constraints`, and can be restored by {meth}`~ommx.v1.Instance.restore_constraint`.
    ///
    /// **Args:**
    /// - `constraint_id`: The ID of the constraint to remove.
    /// - `reason`: The reason why the constraint is removed.
    /// - `parameters`: Additional parameters to describe the reason.
    ///
    /// # Examples
    ///
    /// Relax constraint, and restore it.
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints=[(sum(x) == 3).set_id(1)],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// >>> instance.constraints
    /// [Constraint(x0 + x1 + x2 - 3 == 0)]
    /// ```
    ///
    /// ```python
    /// >>> instance.relax_constraint(1, "manual relaxation")
    /// >>> instance.constraints
    /// []
    /// >>> instance.removed_constraints
    /// [RemovedConstraint(x0 + x1 + x2 - 3 == 0, reason=manual relaxation)]
    /// ```
    ///
    /// ```python
    /// >>> instance.restore_constraint(1)
    /// >>> instance.constraints
    /// [Constraint(x0 + x1 + x2 - 3 == 0)]
    /// >>> instance.removed_constraints
    /// []
    /// ```
    #[pyo3(signature = (constraint_id, reason, **parameters))]
    pub fn relax_constraint(
        &mut self,
        constraint_id: u64,
        reason: String,
        #[gen_stub(override_type(type_repr = "str"))] parameters: Option<HashMap<String, String>>,
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

    /// Relax an indicator constraint by moving it from active to removed.
    #[pyo3(signature = (constraint_id, reason, **parameters))]
    pub fn relax_indicator_constraint(
        &mut self,
        constraint_id: u64,
        reason: String,
        #[gen_stub(override_type(type_repr = "str"))] parameters: Option<HashMap<String, String>>,
    ) -> Result<()> {
        self.inner.relax_indicator_constraint(
            constraint_id.into(),
            reason,
            parameters.unwrap_or_default(),
        )?;
        Ok(())
    }

    /// Restore a removed indicator constraint back to active.
    pub fn restore_indicator_constraint(&mut self, constraint_id: u64) -> Result<()> {
        self.inner
            .restore_indicator_constraint(constraint_id.into())?;
        Ok(())
    }

    /// Convert a one-hot constraint to a regular equality constraint.
    ///
    /// A one-hot constraint over ``{x_1, ..., x_n}`` is mathematically equivalent to the
    /// linear equality ``x_1 + ... + x_n - 1 == 0``. This method inserts that equality
    /// as a new regular constraint and moves the one-hot constraint into
    /// {attr}`~ommx.v1.Instance.removed_one_hot_constraints` with
    /// ``reason="ommx.Instance.convert_one_hot_to_constraint"`` and a
    /// ``constraint_id`` parameter pointing to the new regular constraint.
    ///
    /// Returns the ID of the newly created regular constraint.
    ///
    /// # Examples
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable, OneHotConstraint
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints={},
    /// ...     one_hot_constraints={1: OneHotConstraint(variables=[0, 1, 2])},
    /// ...     sense=Instance.MINIMIZE,
    /// ... )
    /// >>> new_id = instance.convert_one_hot_to_constraint(1)
    /// >>> instance.one_hot_constraints
    /// {}
    /// >>> instance.constraints
    /// {0: Constraint(x0 + x1 + x2 - 1 == 0)}
    /// >>> instance.removed_one_hot_constraints
    /// {1: RemovedOneHotConstraint(OneHotConstraint(exactly one of {x0, x1, x2} = 1), reason=ommx.Instance.convert_one_hot_to_constraint, constraint_id=0)}
    /// ```
    pub fn convert_one_hot_to_constraint(&mut self, one_hot_id: u64) -> Result<u64> {
        let new_id = self
            .inner
            .convert_one_hot_to_constraint(one_hot_id.into())?;
        Ok(new_id.into_inner())
    }

    /// Convert every active one-hot constraint to a regular equality constraint.
    ///
    /// See {meth}`~ommx.v1.Instance.convert_one_hot_to_constraint` for the conversion rule.
    /// Returns the IDs of the newly created regular constraints.
    ///
    /// # Examples
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable, OneHotConstraint
    /// >>> x = [DecisionVariable.binary(i) for i in range(4)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints={},
    /// ...     one_hot_constraints={
    /// ...         1: OneHotConstraint(variables=[0, 1]),
    /// ...         2: OneHotConstraint(variables=[2, 3]),
    /// ...     },
    /// ...     sense=Instance.MINIMIZE,
    /// ... )
    /// >>> instance.convert_all_one_hots_to_constraints()
    /// [0, 1]
    /// >>> instance.one_hot_constraints
    /// {}
    /// >>> instance.constraints
    /// {0: Constraint(x0 + x1 - 1 == 0), 1: Constraint(x2 + x3 - 1 == 0)}
    /// ```
    pub fn convert_all_one_hots_to_constraints(&mut self) -> Result<Vec<u64>> {
        let ids = self.inner.convert_all_one_hots_to_constraints()?;
        Ok(ids.into_iter().map(|id| id.into_inner()).collect())
    }

    /// Convert a SOS1 constraint to regular constraints using the Big-M method.
    ///
    /// A SOS1 constraint over $\{x_1, \ldots, x_n\}$ with each $x_i \in [l_i, u_i]$
    /// asserts that at most one $x_i$ is non-zero. Per variable, a binary indicator
    /// $y_i$ is introduced with the Big-M pair
    ///
    /// $$
    /// x_i - u_i y_i \leq 0, \qquad l_i y_i - x_i \leq 0
    /// $$
    ///
    /// (trivial sides $u_i = 0$ or $l_i = 0$ are skipped), together with the single
    /// cardinality constraint
    ///
    /// $$
    /// \sum_i y_i - 1 \leq 0.
    /// $$
    ///
    /// If $x_i$ is already binary with bound $[0, 1]$, $x_i$ itself is reused as its
    /// indicator (no new variable, no Big-M pair).
    ///
    /// Returns the list of newly created regular constraint IDs in insertion order
    /// (Big-M upper/lower pairs per non-binary variable, followed by the cardinality
    /// sum).
    ///
    /// Raises if any $x_i$ has a non-binary bound that is not finite, if its domain
    /// excludes $0$, or if its kind is semi-continuous / semi-integer (the split
    /// domain $\{0\} \cup [l, u]$ is not uniformly implemented across the codebase
    /// yet, so Big-M conversion of these kinds is not supported).
    /// The instance is not mutated on error.
    ///
    /// # Examples
    ///
    /// All-binary SOS1 reduces to ``sum(x_i) - 1 <= 0`` without extra variables:
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable, Sos1Constraint
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints={},
    /// ...     sos1_constraints={1: Sos1Constraint(variables=[0, 1, 2])},
    /// ...     sense=Instance.MINIMIZE,
    /// ... )
    /// >>> instance.convert_sos1_to_constraints(1)
    /// [0]
    /// >>> instance.sos1_constraints
    /// {}
    /// >>> instance.constraints
    /// {0: Constraint(x0 + x1 + x2 - 1 <= 0)}
    /// >>> instance.removed_sos1_constraints
    /// {1: RemovedSos1Constraint(Sos1Constraint(at most one of {x0, x1, x2} ≠ 0), reason=ommx.Instance.convert_sos1_to_constraints, constraint_ids=0)}
    /// ```
    pub fn convert_sos1_to_constraints(&mut self, sos1_id: u64) -> Result<Vec<u64>> {
        let new_ids = self.inner.convert_sos1_to_constraints(sos1_id.into())?;
        Ok(new_ids.into_iter().map(|id| id.into_inner()).collect())
    }

    /// Convert every active SOS1 constraint to regular constraints using Big-M.
    ///
    /// See {meth}`~ommx.v1.Instance.convert_sos1_to_constraints` for the conversion
    /// rule. Returns a dict mapping each original SOS1 ID to the list of regular
    /// constraint IDs it produced.
    ///
    /// Atomic: every active SOS1 is validated up front, and only if every one is
    /// convertible are the conversions applied. If any SOS1 fails validation
    /// (unsupported kind, non-finite bound, domain excludes 0, etc.), no mutation
    /// happens and the instance is left untouched.
    ///
    /// # Examples
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable, Sos1Constraint
    /// >>> x = [DecisionVariable.binary(i) for i in range(4)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints={},
    /// ...     sos1_constraints={
    /// ...         1: Sos1Constraint(variables=[0, 1]),
    /// ...         2: Sos1Constraint(variables=[2, 3]),
    /// ...     },
    /// ...     sense=Instance.MINIMIZE,
    /// ... )
    /// >>> instance.convert_all_sos1_to_constraints()
    /// {1: [0], 2: [1]}
    /// >>> instance.sos1_constraints
    /// {}
    /// >>> instance.constraints
    /// {0: Constraint(x0 + x1 - 1 <= 0), 1: Constraint(x2 + x3 - 1 <= 0)}
    /// ```
    pub fn convert_all_sos1_to_constraints(&mut self) -> Result<BTreeMap<u64, Vec<u64>>> {
        let result = self.inner.convert_all_sos1_to_constraints()?;
        Ok(result
            .into_iter()
            .map(|(id, ids)| {
                (
                    id.into_inner(),
                    ids.into_iter().map(|c| c.into_inner()).collect(),
                )
            })
            .collect())
    }

    /// Convert an indicator constraint to regular constraints using the Big-M method.
    ///
    /// An indicator constraint ``y = 1 → f(x) <= 0`` (or ``= 0``) is encoded with
    /// upper and lower Big-M sides computed from the interval bounds of $f(x)$:
    ///
    /// $$
    /// f(x) + u y - u \leq 0, \qquad -f(x) - l y + l \leq 0,
    /// $$
    ///
    /// where $u \geq \sup f(x)$ and $l \leq \inf f(x)$ are the upper and lower
    /// bounds of $f$ over the decision variables' domains.
    ///
    /// Side emission:
    ///
    /// - For ``<=`` indicators, only the upper side is considered; it is emitted
    ///   iff $u > 0$. If $u \leq 0$ the constraint is already implied by the
    ///   variable bounds and no Big-M is emitted.
    /// - For ``=`` indicators, both sides are considered independently: upper
    ///   emitted iff $u > 0$, lower emitted iff $l < 0$.
    ///
    /// When an equality side is skipped, the remaining constraints still enforce
    /// the implication correctly because the skipped inequality is already implied
    /// by the variable bounds: e.g. $u \leq 0$ together with the emitted lower side
    /// forces $f(x) = 0$ at $y = 1$ when $u = 0$, or renders $y = 1$ infeasible
    /// when $u < 0$ (correctly reflecting that $f(x) = 0$ has no solution under the
    /// given bounds). When both $u = 0$ and $l = 0$, the bound says $f(x) \equiv 0$
    /// so the equality is vacuously satisfied and nothing is emitted.
    ///
    /// Returns the list of newly created regular constraint IDs in insertion order
    /// (upper first, then lower). The list is empty when both sides are redundant.
    ///
    /// Raises if the bound needed for an emitted side is non-finite, or if $f(x)$
    /// references a semi-continuous / semi-integer variable (the split domain
    /// $\{0\} \cup [l, u]$ is not uniformly implemented, so Big-M conversion could
    /// silently drop the upper side when $0 \notin [l, u]$). The instance is not
    /// mutated on error.
    ///
    /// # Examples
    ///
    /// Convert an inequality indicator where the upper side is active:
    ///
    /// ```python
    /// >>> from ommx.v1 import (
    /// ...     Instance, DecisionVariable, IndicatorConstraint, Equality,
    /// ... )
    /// >>> x = DecisionVariable.continuous(0, lower=0.0, upper=5.0)
    /// >>> y = DecisionVariable.binary(1)
    /// >>> ic = IndicatorConstraint(
    /// ...     indicator_variable=y,
    /// ...     function=x - 2,
    /// ...     equality=Equality.LessThanOrEqualToZero,
    /// ... )
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x, y],
    /// ...     objective=x,
    /// ...     constraints={},
    /// ...     indicator_constraints={1: ic},
    /// ...     sense=Instance.MINIMIZE,
    /// ... )
    /// >>> instance.convert_indicator_to_constraint(1)
    /// [0]
    /// >>> instance.indicator_constraints
    /// {}
    /// >>> instance.constraints
    /// {0: Constraint(x0 + 3*x1 - 5 <= 0)}
    /// ```
    pub fn convert_indicator_to_constraint(&mut self, indicator_id: u64) -> Result<Vec<u64>> {
        let new_ids = self
            .inner
            .convert_indicator_to_constraint(indicator_id.into())?;
        Ok(new_ids.into_iter().map(|id| id.into_inner()).collect())
    }

    /// Convert every active indicator constraint to regular constraints using Big-M.
    ///
    /// See {meth}`~ommx.v1.Instance.convert_indicator_to_constraint` for the
    /// conversion rule. Returns a dict mapping each original indicator ID to the
    /// list of regular constraint IDs it produced.
    ///
    /// Atomic: every active indicator is validated up front, and only if every
    /// one is convertible are the conversions applied. If any indicator fails
    /// validation (non-finite bound on a required side), no mutation happens and
    /// the instance is left untouched.
    pub fn convert_all_indicators_to_constraints(&mut self) -> Result<BTreeMap<u64, Vec<u64>>> {
        let result = self.inner.convert_all_indicators_to_constraints()?;
        Ok(result
            .into_iter()
            .map(|(id, ids)| {
                (
                    id.into_inner(),
                    ids.into_iter().map(|c| c.into_inner()).collect(),
                )
            })
            .collect())
    }

    /// Log-encode the integer decision variables.
    ///
    /// Log encoding of an integer variable $x \in [l, u]$ is to represent by $m$ bits $b_i \in \{0, 1\}$ by:
    ///
    /// $$x = \sum_{i=0}^{m-2} 2^i b_i + (u - l - 2^{m-1} + 1) b_{m-1} + l$$
    ///
    /// where $m = \lceil \log_2(u - l + 1) \rceil$.
    ///
    /// **Args:**
    /// - `decision_variable_ids`: The IDs of the integer decision variables to log-encode.
    ///   If not specified (or empty), all integer variables are log-encoded.
    ///
    /// # Examples
    ///
    /// Let's consider a simple integer programming problem with three integer variables x0, x1, and x2.
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [
    /// ...     DecisionVariable.integer(i, lower=0, upper=3, name="x", subscripts=[i])
    /// ...     for i in range(3)
    /// ... ]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints=[],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// >>> instance.objective
    /// Function(x0 + x1 + x2)
    /// ```
    ///
    /// To log-encode the integer variables x0 and x2 (except x1), call log_encode:
    ///
    /// ```python
    /// >>> instance.log_encode({0, 2})
    /// ```
    ///
    /// Integer variable in range $[0, 3]$ can be represented by two binary variables:
    ///
    /// $$x_0 = b_{0,0} + 2 b_{0,1}, \quad x_2 = b_{2,0} + 2 b_{2,1}$$
    ///
    /// And these are substituted into the objective and constraint functions.
    ///
    /// ```python
    /// >>> instance.objective
    /// Function(x1 + x3 + 2*x4 + x5 + 2*x6)
    /// ```
    #[pyo3(signature = (decision_variable_ids=BTreeSet::new()))]
    pub fn log_encode(
        &mut self,
        py: Python<'_>,
        decision_variable_ids: BTreeSet<u64>,
    ) -> Result<()> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let ids: BTreeSet<u64> = if decision_variable_ids.is_empty() {
            // Auto-detect: find all used integer decision variables
            let analysis = self.inner.analyze_decision_variables();
            let integer_ids: BTreeSet<u64> = analysis
                .used_integer()
                .into_keys()
                .map(|id| id.into_inner())
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

    /// Convert an inequality constraint $f(x) \leq 0$ to an equality constraint $f(x) + s/a = 0$ with an integer slack variable $s$.
    ///
    /// - Since $a$ is determined as the minimal multiplier to make every coefficient of $a f(x)$ integer,
    ///   $a$ itself and the range of $s$ becomes impractically large. ``max_integer_range`` limits the maximal
    ///   range of $s$, and returns error if the range exceeds it.
    ///
    /// - Since this method evaluates the bound of $f(x)$, we may find that:
    ///
    ///   - The bound $[l, u]$ is strictly positive, i.e. $l > 0$:
    ///     this means the instance is infeasible because this constraint never be satisfied,
    ///     and an error is raised.
    ///
    ///   - The bound $[l, u]$ is always negative, i.e. $u \leq 0$:
    ///     this means this constraint is trivially satisfied,
    ///     the constraint is moved to {attr}`~ommx.v1.Instance.removed_constraints`,
    ///     and this method returns without introducing slack variable or raising an error.
    ///
    /// # Examples
    ///
    /// Let's consider a simple inequality constraint x0 + 2*x1 <= 5.
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [
    /// ...     DecisionVariable.integer(i, lower=0, upper=3, name="x", subscripts=[i])
    /// ...     for i in range(3)
    /// ... ]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints=[
    /// ...         (x[0] + 2*x[1] <= 5).set_id(0)
    /// ...     ],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// >>> instance.constraints[0]
    /// Constraint(x0 + 2*x1 - 5 <= 0)
    /// ```
    ///
    /// Introduce an integer slack variable
    ///
    /// ```python
    /// >>> instance.convert_inequality_to_equality_with_integer_slack(
    /// ...     constraint_id=0,
    /// ...     max_integer_range=32
    /// ... )
    /// >>> instance.constraints[0]
    /// Constraint(x0 + 2*x1 + x3 - 5 == 0)
    /// ```
    pub fn convert_inequality_to_equality_with_integer_slack(
        &mut self,
        constraint_id: u64,
        max_integer_range: u64,
    ) -> Result<()> {
        self.inner
            .convert_inequality_to_equality_with_integer_slack(
                constraint_id,
                max_integer_range,
                ommx::ATol::default(),
            )?;
        Ok(())
    }

    /// Convert inequality $f(x) \leq 0$ to **inequality** $f(x) + b s \leq 0$ with an integer slack variable $s$.
    ///
    /// - This should be used when {meth}`~ommx.v1.Instance.convert_inequality_to_equality_with_integer_slack` is not applicable.
    ///
    /// - The bound of $s$ will be $[0, \text{slack\_upper\_bound}]$, and the coefficient $b$ is determined from the lower bound of $f(x)$.
    ///
    /// - Since the slack variable is integer, the yielded inequality has residual error $\min_s f(x) + b s$ at most $b$.
    ///   And thus $b$ is returned to use scaling the penalty weight or other things.
    ///
    ///   - Larger slack_upper_bound (i.e. finer-grained slack) yields smaller $b$, and thus smaller the residual error,
    ///     but it needs more bits for the slack variable, and thus the problem size becomes larger.
    ///
    /// **Returns:**
    /// The coefficient $b$ of the slack variable. If the constraint is trivially satisfied, this returns ``None``.
    ///
    /// # Examples
    ///
    /// Let's consider a simple inequality constraint x0 + 2*x1 <= 4.
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [
    /// ...     DecisionVariable.integer(i, lower=0, upper=3, name="x", subscripts=[i])
    /// ...     for i in range(3)
    /// ... ]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints=[
    /// ...         (x[0] + 2*x[1] <= 4).set_id(0)
    /// ...     ],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// >>> instance.constraints[0]
    /// Constraint(x0 + 2*x1 - 4 <= 0)
    /// ```
    ///
    /// Introduce an integer slack variable s in [0, 2]
    ///
    /// ```python
    /// >>> b = instance.add_integer_slack_to_inequality(
    /// ...     constraint_id=0,
    /// ...     slack_upper_bound=2
    /// ... )
    /// >>> b, instance.constraints[0]
    /// (2.0, Constraint(x0 + 2*x1 + 2*x3 - 4 <= 0))
    /// ```
    pub fn add_integer_slack_to_inequality(
        &mut self,
        constraint_id: u64,
        slack_upper_bound: u64,
    ) -> Result<Option<f64>> {
        let result = self
            .inner
            .add_integer_slack_to_inequality(constraint_id, slack_upper_bound)?;
        Ok(result)
    }

    /// Analyze decision variables in the optimization problem instance.
    ///
    /// Returns a comprehensive analysis of all decision variables including:
    ///
    /// - Kind-based partitioning (binary, integer, continuous, etc.)
    /// - Usage-based partitioning (used in objective, constraints, fixed, etc.)
    /// - Variable bounds information
    ///
    /// **Returns:**
    /// Analysis object containing detailed information about decision variables
    ///
    /// # Examples
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=x[0] + x[1],
    /// ...     constraints=[(x[1] + x[2] == 1).set_id(0)],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// >>> analysis = instance.decision_variable_analysis()
    /// >>> analysis.used_decision_variable_ids()
    /// {0, 1, 2}
    /// >>> analysis.used_in_objective()
    /// {0, 1}
    /// >>> analysis.used_in_constraints()
    /// {0: {1, 2}}
    /// ```
    pub fn decision_variable_analysis(&self) -> DecisionVariableAnalysis {
        DecisionVariableAnalysis(self.inner.analyze_decision_variables())
    }

    /// Get statistics about the instance.
    ///
    /// Returns a dictionary containing counts of decision variables and constraints
    /// categorized by kind, usage, and status.
    ///
    /// **Returns:**
    /// A dictionary with the following structure:
    ///
    /// ```text
    /// {
    ///     "decision_variables": {
    ///         "total": int,
    ///         "by_kind": {
    ///             "binary": int,
    ///             "integer": int,
    ///             "continuous": int,
    ///             "semi_integer": int,
    ///             "semi_continuous": int
    ///         },
    ///         "by_usage": {
    ///             "used_in_objective": int,
    ///             "used_in_constraints": int,
    ///             "used": int,
    ///             "fixed": int,
    ///             "dependent": int,
    ///             "irrelevant": int
    ///         }
    ///     },
    ///     "constraints": {
    ///         "total": int,
    ///         "active": int,
    ///         "removed": int
    ///     }
    /// }
    /// ```
    ///
    /// # Examples
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance
    /// >>> instance = Instance.empty()
    /// >>> stats = instance.stats()
    /// >>> stats["decision_variables"]["total"]
    /// 0
    /// >>> stats["constraints"]["total"]
    /// 0
    /// ```
    pub fn stats<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyDict>> {
        let stats = self.inner.stats();
        serde_pyobject::to_pyobject(py, &stats)?
            .extract()
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// DataFrame of decision variables
    #[pyo3(signature = (include = None))]
    pub fn decision_variables_df<'py>(
        &self,
        py: Python<'py>,
        include: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let flags = crate::pandas::IncludeFlags::from_optional(include)?;
        let var_meta_store = self.inner.variable_metadata().clone();
        let var_meta_view: Vec<(ommx::DecisionVariableMetadata, &ommx::DecisionVariable)> = self
            .inner
            .decision_variables()
            .iter()
            .map(|(id, dv)| (var_meta_store.collect_for(*id), dv))
            .collect();
        entries_to_dataframe(
            py,
            var_meta_view
                .iter()
                .map(|(m, dv)| crate::pandas::WithMetadata::new(*dv, m)),
            "id",
            flags,
        )
    }

    /// DataFrame of constraints, dispatched on `kind=`.
    ///
    /// `kind` selects the constraint family — `"regular"`, `"indicator"`,
    /// `"one_hot"`, or `"sos1"`. The DataFrame is indexed by the kind-
    /// qualified id column (`{kind}_constraint_id`).
    ///
    /// `include` selects which optional column families to fold in. It
    /// accepts a sequence of `"metadata"` / `"parameters"` /
    /// `"removed_reason"`; passing `None` (the default) yields the
    /// v2-equivalent shape (`metadata` + `parameters`). `"removed_reason"`
    /// is a unit flag that gates both the `removed_reason` column and the
    /// `removed_reason.{key}` parameter columns together.
    ///
    /// `removed=False` (default) returns active constraints only.
    /// `removed=True` returns active + removed rows in the same DataFrame
    /// and auto-sets `"removed_reason"` so removed rows are
    /// distinguishable (active rows have NA in the reason columns).
    #[pyo3(signature = (kind = ConstraintKind::Regular, include = None, removed = false))]
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
                let meta = coll.metadata().clone();
                let active = coll.active();
                let removed_map = coll.removed();
                let mut entries: Vec<Bound<'py, pyo3::types::PyAny>> = Vec::new();
                if removed {
                    // Globally id-sorted merge of active and removed.
                    // Both BTreeMaps are individually sorted; ids are
                    // disjoint between the two maps.
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
                            let dict = crate::pandas::WithMetadata::new((*id, c), &m)
                                .to_pandas_entry(py)?;
                            // Active rows in a removed=True view get NA
                            // in the (always-present) removed_reason column.
                            crate::pandas::set_removed_reason_na(&dict)?;
                            crate::pandas::apply_include_filter(&dict, flags)?;
                            crate::pandas::rename_id_column(&dict, id_col)?;
                            entries.push(dict.into_any());
                        } else {
                            let (id, pair) = ri.next().unwrap();
                            let m = meta.collect_for(*id);
                            let dict = crate::pandas::WithMetadata::new((*id, pair), &m)
                                .to_pandas_entry(py)?;
                            crate::pandas::apply_include_filter(&dict, flags)?;
                            crate::pandas::rename_id_column(&dict, id_col)?;
                            entries.push(dict.into_any());
                        }
                    }
                } else {
                    for (id, c) in active.iter() {
                        let m = meta.collect_for(*id);
                        let dict =
                            crate::pandas::WithMetadata::new((*id, c), &m).to_pandas_entry(py)?;
                        // User-requested removed_reason on an active-only
                        // view: ensure the column survives even when no
                        // row carries a reason.
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
        let nf_meta_store = self.inner.named_function_metadata().clone();
        let nf_meta_view: Vec<(ommx::NamedFunctionMetadata, &ommx::NamedFunction)> = self
            .inner
            .named_functions()
            .iter()
            .map(|(id, nf)| (nf_meta_store.collect_for(*id), nf))
            .collect();
        entries_to_dataframe(
            py,
            nf_meta_view
                .iter()
                .map(|(m, nf)| crate::pandas::WithMetadata::new(*nf, m)),
            "id",
            flags,
        )
    }

    /// Constraint metadata DataFrame (id-indexed wide format).
    ///
    /// One row per constraint id (active + removed) with columns
    /// `name`, `subscripts`, `description`. Index column is
    /// `{kind}_constraint_id`. `kind` selects which constraint family
    /// to read: `"regular"`, `"indicator"`, `"one_hot"`, or `"sos1"`.
    #[pyo3(signature = (kind = ConstraintKind::Regular))]
    pub fn constraint_metadata_df<'py>(
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
                crate::pandas::constraint_metadata_dataframe(
                    py,
                    coll.metadata(),
                    coll.active().keys().chain(coll.removed().keys()).copied(),
                    id_col,
                )
            }
        )
    }

    /// Constraint parameters DataFrame (long format).
    ///
    /// One row per (constraint_id, parameter_key) pair. Columns:
    /// `{kind}_constraint_id`, `key`, `value`. Default RangeIndex.
    #[pyo3(signature = (kind = ConstraintKind::Regular))]
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
                    coll.metadata(),
                    coll.active().keys().chain(coll.removed().keys()).copied(),
                    id_col,
                )
            }
        )
    }

    /// Constraint provenance DataFrame (long format).
    ///
    /// One row per (constraint_id, step) pair. Columns:
    /// `{kind}_constraint_id`, `step`, `source_kind`, `source_id`.
    #[pyo3(signature = (kind = ConstraintKind::Regular))]
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
                    coll.metadata(),
                    coll.active().keys().chain(coll.removed().keys()).copied(),
                    id_col,
                )
            }
        )
    }

    /// Removed-constraint reasons DataFrame (long format).
    ///
    /// One row per (constraint_id, parameter_key) pair, plus one row with
    /// `key`/`value` set to NA when the reason has no parameters. Columns:
    /// `{kind}_constraint_id`, `reason`, `key`, `value`.
    #[pyo3(signature = (kind = ConstraintKind::Regular))]
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

    /// Decision-variable metadata DataFrame (id-indexed wide format).
    ///
    /// Columns: `name`, `subscripts`, `description`. Index column =
    /// `variable_id`.
    pub fn variable_metadata_df<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDataFrame>> {
        crate::pandas::variable_metadata_dataframe(
            py,
            self.inner.variable_metadata(),
            self.inner.decision_variables().keys().copied(),
            "variable_id",
        )
    }

    /// Decision-variable parameters DataFrame (long format).
    ///
    /// One row per (variable_id, parameter_key) pair. Columns:
    /// `variable_id`, `key`, `value`.
    pub fn variable_parameters_df<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        crate::pandas::variable_parameters_dataframe(
            py,
            self.inner.variable_metadata(),
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

    /// Convert the instance to a minimization problem.
    ///
    /// If the instance is already a minimization problem, this does nothing.
    ///
    /// **Returns:**
    /// ``True`` if the instance is converted, ``False`` if already a minimization problem.
    ///
    /// # Examples
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints=[sum(x) == 1],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// >>> instance.sense == Instance.MAXIMIZE
    /// True
    /// >>> instance.objective
    /// Function(x0 + x1 + x2)
    /// ```
    ///
    /// Convert to a minimization problem
    ///
    /// ```python
    /// >>> instance.as_minimization_problem()
    /// True
    /// >>> instance.sense == Instance.MINIMIZE
    /// True
    /// >>> instance.objective
    /// Function(-x0 - x1 - x2)
    /// ```
    ///
    /// If the instance is already a minimization problem, this does nothing
    ///
    /// ```python
    /// >>> instance.as_minimization_problem()
    /// False
    /// ```
    pub fn as_minimization_problem(&mut self) -> bool {
        self.inner.as_minimization_problem()
    }

    /// Convert the instance to a maximization problem.
    ///
    /// If the instance is already a maximization problem, this does nothing.
    ///
    /// **Returns:**
    /// ``True`` if the instance is converted, ``False`` if already a maximization problem.
    ///
    /// # Examples
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints=[sum(x) == 1],
    /// ...     sense=Instance.MINIMIZE,
    /// ... )
    /// >>> instance.sense == Instance.MINIMIZE
    /// True
    /// >>> instance.objective
    /// Function(x0 + x1 + x2)
    /// ```
    ///
    /// Convert to a maximization problem
    ///
    /// ```python
    /// >>> instance.as_maximization_problem()
    /// True
    /// >>> instance.sense == Instance.MAXIMIZE
    /// True
    /// >>> instance.objective
    /// Function(-x0 - x1 - x2)
    /// ```
    ///
    /// If the instance is already a maximization problem, this does nothing
    ///
    /// ```python
    /// >>> instance.as_maximization_problem()
    /// False
    /// ```
    pub fn as_maximization_problem(&mut self) -> bool {
        self.inner.as_maximization_problem()
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
            .map(|constraint| Constraint::from_parts(constraint.clone(), metadata.collect_for(cid)))
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
            .map(|removed_constraint| {
                let (c, r) = removed_constraint;
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
        let id = NamedFunctionID::from(named_function_id);
        self.inner
            .named_functions()
            .get(&id)
            .map(|named_function| {
                NamedFunction(
                    named_function.clone(),
                    self.inner.named_function_metadata().collect_for(id),
                )
            })
            .ok_or_else(|| {
                PyKeyError::new_err(format!(
                    "Named function with ID {named_function_id} not found"
                ))
            })
    }

    /// Reduce binary powers in the instance.
    ///
    /// This method replaces binary powers in the instance with their equivalent linear expressions.
    /// For binary variables, $x^n = x$ for any $n \geq 1$, so we can reduce higher powers to linear terms.
    ///
    /// **Returns:**
    /// ``True`` if any reduction was performed, ``False`` otherwise.
    ///
    /// # Examples
    ///
    /// Consider an instance with binary variables and quadratic terms:
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [DecisionVariable.binary(i) for i in range(2)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=x[0] * x[0] + x[0] * x[1],
    /// ...     constraints=[],
    /// ...     sense=Instance.MINIMIZE,
    /// ... )
    /// >>> instance.objective
    /// Function(x0*x0 + x0*x1)
    /// ```
    ///
    /// After reducing binary powers, x0^2 becomes x0:
    ///
    /// ```python
    /// >>> changed = instance.reduce_binary_power()
    /// >>> changed
    /// True
    /// >>> instance.objective
    /// Function(x0*x1 + x0)
    /// ```
    ///
    /// Running it again should not change anything:
    ///
    /// ```python
    /// >>> changed = instance.reduce_binary_power()
    /// >>> changed
    /// False
    /// ```
    pub fn reduce_binary_power(&mut self) -> bool {
        self.inner.reduce_binary_power()
    }

    #[staticmethod]
    pub fn load_mps(py: Python<'_>, path: String) -> Result<Self> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let instance = ommx::mps::load(path)?;
        Ok(Self {
            inner: instance,
            annotations: HashMap::new(),
        })
    }

    #[pyo3(signature = (path, compress = true))]
    pub fn save_mps(&self, py: Python<'_>, path: String, compress: bool) -> Result<()> {
        let _guard = crate::TRACING.attach_parent_context(py);
        ommx::mps::save(&self.inner, path, compress)?;
        Ok(())
    }

    #[staticmethod]
    pub fn load_qplib(py: Python<'_>, path: String) -> Result<Self> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let instance = ommx::qplib::load(path)?;
        Ok(Self {
            inner: instance,
            annotations: HashMap::new(),
        })
    }

    /// Generate folded stack format for memory profiling of this instance.
    ///
    /// This method generates a format compatible with flamegraph visualization tools
    /// like ``flamegraph.pl`` and ``inferno``. Each line has the format:
    /// "frame1;frame2;...;frameN bytes"
    ///
    /// The output shows the hierarchical memory structure of the instance, making it
    /// easy to identify which components are consuming the most memory.
    ///
    /// To visualize with flamegraph:
    ///
    /// 1. Save the output to a file: ``profile.txt``
    /// 2. Generate SVG: ``flamegraph.pl profile.txt > memory.svg``
    /// 3. Open memory.svg in a browser
    ///
    /// **Returns:**
    /// Folded stack format string that can be visualized with flamegraph tools
    ///
    /// # Examples
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=x[0] + x[1],
    /// ...     constraints=[],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// >>> profile = instance.logical_memory_profile()
    /// >>> isinstance(profile, str)
    /// True
    /// ```
    pub fn logical_memory_profile(&self) -> String {
        self.inner.logical_memory_profile().to_string()
    }
}

impl Instance {
    pub(crate) fn check_no_continuous_variables(&self, format_name: &str) -> Result<()> {
        let continuous_ids: Vec<u64> = self
            .inner
            .analyze_decision_variables()
            .used_continuous()
            .into_keys()
            .map(|id| id.into_inner())
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
    #[tracing::instrument(skip_all)]
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
